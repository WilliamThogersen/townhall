//! `create_task`'s input checks and defaults: the schema's limits, the repo (the given one,
//! else the conversation's `linkedRepo`, else its latest `repo` entry), the base branch and
//! a name no agent or branch has yet. Every refusal is a text the session can act on.

use std::path::PathBuf;

use crate::cmd::{run, validate_name};
use crate::context::Context;
use crate::model::{Conversation, CreateTaskInput, EntryBody, Repo, TASK_BRANCH_PREFIX};
use crate::repos::{names, verify_branch};
use crate::{fleet, Error};

use super::{MAX_LIST_LEN, MAX_NAME_LEN, MAX_PROMPT_LEN, MAX_TITLE_LEN};

pub(super) type Checked<T> = std::result::Result<T, String>;

fn too_long(field: &str, value: &str, max: u64) -> Checked<()> {
    let len = u64::try_from(value.chars().count()).unwrap_or(u64::MAX);
    match len > max {
        true => Err(format!("`{field}` is longer than {max} characters")),
        false => Ok(()),
    }
}

fn too_many(field: &str, list: &[String]) -> Checked<()> {
    let len = u64::try_from(list.len()).unwrap_or(u64::MAX);
    match len > MAX_LIST_LEN {
        true => Err(format!("`{field}` lists more than {MAX_LIST_LEN} names")),
        false => Ok(()),
    }
}

/// `text` on one line: control characters and runs of whitespace become one space.
pub(super) fn one_line(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn given(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

/// The input within the schema's limits, its title on one line and blank optionals dropped.
pub(super) fn input(raw: CreateTaskInput) -> Checked<CreateTaskInput> {
    validate_name(&raw.name).map_err(|e| e.to_string())?;
    too_long("name", &raw.name, MAX_NAME_LEN)?;
    let title = one_line(&raw.title);
    if title.is_empty() {
        return Err("`title` must not be blank".into());
    }
    too_long("title", &title, MAX_TITLE_LEN)?;
    if raw.prompt.trim().is_empty() {
        return Err("`prompt` must not be blank: it is the agent's whole brief".into());
    }
    too_long("prompt", &raw.prompt, MAX_PROMPT_LEN)?;
    too_many("after", &raw.after)?;
    too_many("attachments", &raw.attachments)?;
    Ok(CreateTaskInput {
        title,
        repo: given(raw.repo.as_deref()),
        base_branch: given(raw.base_branch.as_deref()),
        ..raw
    })
}

/// `shop, admin` (sorted), or what to do when no repo is registered.
fn repo_list(repos: &[Repo]) -> String {
    let mut listed: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
    listed.sort_by_key(|n| n.to_ascii_lowercase());
    match listed.is_empty() {
        true => "none: no repo is registered yet, ask the user to add one".into(),
        false => listed.join(", "),
    }
}

async fn latest_repo_entry(ctx: &Context, conversation_id: &str) -> Checked<Option<String>> {
    let store = ctx.store().map_err(|e| e.to_string())?;
    let entries = store
        .entries(conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().rev().find_map(|e| match e.body {
        EntryBody::Repo { repo, .. } => Some(repo),
        _ => None,
    }))
}

/// The given repo, else the conversation's linked one, else its latest `repo` entry's.
pub(super) async fn repo(
    ctx: &Context,
    conversation: &Conversation,
    wanted: Option<&str>,
) -> Checked<Repo> {
    let repos = ctx.repos().await.map_err(|e| e.to_string())?;
    let chosen = match (wanted, &conversation.linked_repo) {
        (Some(name), _) => Some(name.to_string()),
        (None, Some(linked)) => Some(linked.clone()),
        (None, None) => latest_repo_entry(ctx, &conversation.id).await?,
    };
    let Some(name) = chosen else {
        return Err(format!(
            "this conversation has no repo yet: pass `repo`, one of: {}",
            repo_list(&repos)
        ));
    };
    repos
        .iter()
        .find(|r| names::same(&r.name, &name))
        .cloned()
        .ok_or_else(|| {
            format!(
                "unknown repo `{name}`: pass `repo`, one of: {}",
                repo_list(&repos)
            )
        })
}

/// The given base branch when it resolves in the repo, else the repo's own.
pub(super) async fn base_branch(repo: &Repo, wanted: Option<&str>) -> Checked<String> {
    let Some(branch) = wanted.map(String::from) else {
        return Ok(repo.base_branch.clone());
    };
    let (path, check): (PathBuf, String) = (repo.path.clone(), branch.clone());
    tokio::task::spawn_blocking(move || verify_branch(&path, &check))
        .await
        .map_err(|e| format!("base branch check failed: {e}"))?
        .map_err(|e| format!("{e}: pass a branch of repo `{}` or leave it out", repo.name))?;
    Ok(branch)
}

/// No agent in any repo and no branch `townhall/<name>` in this one has the name yet.
pub(super) async fn name_is_free(ctx: &Context, repo: &Repo, name: &str) -> Checked<()> {
    let held = fleet::holders(ctx, name).await.map_err(|e| e.to_string())?;
    if let Some(found) = held.first() {
        return Err(format!(
            "an agent named `{name}` already exists in repo `{}`: pick another name",
            found.repo
        ));
    }
    let branch = format!("refs/heads/{TASK_BRANCH_PREFIX}{name}");
    let args = ["rev-parse", "--verify", "--quiet", branch.as_str()];
    match run("git", &args, Some(&repo.path)).await {
        Ok(_) => Err(format!(
            "branch `{TASK_BRANCH_PREFIX}{name}` already exists in repo `{}`: pick another name",
            repo.name
        )),
        Err(Error::Command { .. }) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(title: &str) -> CreateTaskInput {
        CreateTaskInput {
            name: "fix-login".into(),
            title: title.into(),
            prompt: "p".into(),
            repo: Some("  ".into()),
            base_branch: Some(" develop ".into()),
            ..CreateTaskInput::default()
        }
    }

    #[test]
    fn titles_go_on_one_line_and_blank_optionals_drop() {
        assert_eq!(one_line(" a\n\tb \u{7}c  "), "a b c");
        let checked = input(raw("  fix\nthe login ")).unwrap();
        assert_eq!(checked.title, "fix the login");
        assert_eq!(checked.repo, None);
        assert_eq!(checked.base_branch.as_deref(), Some("develop"));
        assert!(input(raw("\n \t")).is_err());
    }

    #[test]
    fn the_repo_list_is_sorted_or_says_what_to_do() {
        let repo = |name: &str| Repo {
            name: name.into(),
            path: PathBuf::from("/r"),
            base_branch: "main".into(),
        };
        assert_eq!(repo_list(&[repo("shop"), repo("Admin")]), "Admin, shop");
        assert!(repo_list(&[]).starts_with("none"));
    }
}
