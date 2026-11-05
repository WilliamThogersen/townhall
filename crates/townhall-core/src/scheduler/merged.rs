//! Telling `merged`: an `ended` task whose branch `townhall/<name>` has commits of its own
//! (its tip is not the commit it started from) and is reachable from its base branch.
//! Read only: one `git for-each-ref --merged=<base>` per repo and base, never a merge or a
//! delete.

use std::collections::HashMap;
use std::path::Path;

use crate::cmd::run;
use crate::context::Context;
use crate::model::{AgentEvent, EntryBody, Task, TaskState, TASK_BRANCH_PREFIX};
use crate::store::Store;
use crate::{Error, Result};

use super::{transition, TaskLaunch};

const BRANCH_REFS: &str = "refs/heads/";

/// The commit `base` names, or `None` once it is gone (a stacked task's base branch is often
/// deleted after its merge). `--ignore-missing` keeps that a clean exit instead of a failed
/// command logged on every tick.
async fn base_commit(path: &Path, base: &str) -> Result<Option<String>> {
    let args = ["rev-list", "--ignore-missing", "--max-count=1", base, "--"];
    let commit = run("git", &args, Some(path)).await?;
    Ok(Some(commit).filter(|c| !c.is_empty()))
}

/// The tip of every `townhall/*` branch of `repo` that is merged into `base`, by task name.
/// Empty when `base` no longer exists.
async fn merged_tips(ctx: &Context, repo: &str, base: &str) -> Result<HashMap<String, String>> {
    if base.trim().is_empty() || base.starts_with('-') {
        return Err(Error::InvalidInput(format!("invalid base branch `{base}`")));
    }
    let repo = ctx.repo(repo).await?;
    let Some(commit) = base_commit(&repo.path, base).await? else {
        return Ok(HashMap::new());
    };
    let merged = format!("--merged={commit}");
    let pattern = format!("{BRANCH_REFS}{TASK_BRANCH_PREFIX}");
    let args = [
        "for-each-ref",
        merged.as_str(),
        "--format=%(refname) %(objectname)",
        pattern.as_str(),
    ];
    let out = run("git", &args, Some(&repo.path)).await?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let (refname, tip) = line.split_once(' ')?;
            let name = refname.strip_prefix(&pattern)?;
            Some((name.to_string(), tip.trim().to_string()))
        })
        .collect())
}

fn has_own_commits(task: &Task, tips: &HashMap<String, String>, launches: &[TaskLaunch]) -> bool {
    let Some(tip) = tips.get(&task.name) else {
        return false;
    };
    launches
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(&task.name))
        .is_some_and(|l| &l.start_commit != tip)
}

/// `ended` to `merged`, with `✓ <name> merged into <base>`. False when it moved on meanwhile.
async fn mark(store: &Store, task: &Task) -> Result<bool> {
    let moved = transition(store, &task.name, TaskState::Ended, TaskState::Merged).await?;
    if moved.is_none() {
        return Ok(false);
    }
    let body = EntryBody::Agent {
        agent: task.name.clone(),
        event: AgentEvent::Passed,
        text: format!("merged into {}", task.base_branch),
    };
    store.append_entry(&task.conversation_id, body).await?;
    tracing::info!(
        target: "townhall::task", event = "task.merged", task = %task.name,
        base = %task.base_branch, "task merged"
    );
    Ok(true)
}

/// Every distinct (repo, base branch) of the ended tasks, in task order.
fn groups(ended: &[&Task]) -> Vec<(String, String)> {
    ended.iter().fold(vec![], |acc, t| {
        let key = (t.repo.clone(), t.base_branch.clone());
        if acc.contains(&key) {
            acc
        } else {
            [acc, vec![key]].concat()
        }
    })
}

/// Marks the ended tasks whose branch was merged. Returns their names and, per repo and
/// base that could not be read, a warning.
pub(super) async fn detect(ctx: &Context, tasks: &[Task]) -> Result<(Vec<String>, Vec<String>)> {
    let ended: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.state == TaskState::Ended)
        .collect();
    if ended.is_empty() {
        return Ok((vec![], vec![]));
    }
    let store = ctx.store()?;
    let launches = store.task_launches().await?;
    let (mut merged, mut warnings) = (vec![], vec![]);
    for (repo, base) in groups(&ended) {
        let tips = match merged_tips(ctx, &repo, &base).await {
            Ok(tips) => tips,
            Err(e) => {
                warnings.push(format!(
                    "merges into `{base}` of repo `{repo}` unknown: {e}"
                ));
                continue;
            }
        };
        let of_group = ended
            .iter()
            .filter(|t| t.repo == repo && t.base_branch == base)
            .filter(|t| has_own_commits(t, &tips, &launches));
        for task in of_group {
            if mark(store, task).await? {
                merged.push(task.name.clone());
            }
        }
    }
    Ok((merged, warnings))
}
