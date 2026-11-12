//! `open_pr`: pushes the task's branch and opens its pull request (GitHub) or merge request
//! (GitLab), then keeps the link on the task.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::context::Context;
use crate::model::{Checks, Pr, Task};
use crate::plugins::hooks;
use crate::repos::accounts::Accounts;
use crate::repos::names;
use crate::repos::publish::{origin_of, publish, PrRequest, Programs, Published};

use super::checks::{one_line, Checked};
use super::hand_back::worktree_of;
use super::{ToolDef, MAX_PROMPT_LEN};

pub const OPEN_PR: &str = "open_pr";

const MAX_TITLE_LEN: u64 = 200;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OpenPrInput {
    title: String,
    #[serde(default)]
    body: Option<String>,
}

pub fn tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: OPEN_PR,
        description: "Push this task's branch to origin and open a pull request (GitHub) or merge request (GitLab) into the task's `prBase` when it has one, else its base branch. Returns state `opened` with the url, or `noRemote` when the repo has no origin (nothing is pushed). Commit your work first.",
        input_schema: schema(),
        read_only: false,
    }]
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string", "minLength": 1, "maxLength": MAX_TITLE_LEN,
                "description": "The title of the pull or merge request."
            },
            "body": {
                "type": "string", "maxLength": MAX_PROMPT_LEN,
                "description": "Its description: what changed and why."
            }
        },
        "required": ["title"],
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

pub(super) async fn open_pr(ctx: &Context, task: &str, input: OpenPrInput) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    let task = store
        .find_task(task)
        .await
        .map_err(text)?
        .ok_or_else(|| format!("no task named `{task}`"))?;
    let title = one_line(&input.title);
    if title.is_empty() {
        return Err("`title` must not be blank".into());
    }
    let worktree = worktree_of(ctx, &task).await.ok_or_else(|| {
        format!(
            "task `{}` has no worktree: it has not started or was cleaned up",
            task.name
        )
    })?;
    let (branch, base) = (task.branch(), task.pr_target().to_string());
    let body = input.body.unwrap_or_default();
    let fields = json!({
        "task": task, "repo": task.repo, "title": title, "body": body, "base": base
    });
    let asked = hooks::ask(ctx, "pr.before_open", fields).await;
    if let Some(blocked) = asked.blocked {
        return Err(format!(
            "plugin {} blocked opening the pull request: {}",
            blocked.plugin, blocked.reason
        ));
    }
    let title = asked.title.as_deref().map_or(title, one_line);
    let body = asked.body.unwrap_or(body);
    let settings = store.settings().await.map_err(text)?;
    let repos = store.repos().await.map_err(text)?;
    let repo = repos.iter().find(|r| names::same(&r.name, &task.repo));
    let accounts = Accounts::for_repo(&settings, repo);
    let (published, account) = tokio::task::spawn_blocking(move || {
        let req = PrRequest {
            worktree: &worktree,
            branch: &branch,
            base: &base,
            title: &title,
            body: &body,
            accounts: &accounts,
        };
        let published = publish(&req, &Programs::from_env())?;
        let account = match origin_of(&worktree)? {
            Some(origin) => accounts.pick(origin.forge.kind(), &origin.host)?,
            None => None,
        };
        Ok::<_, crate::Error>((published, account))
    })
    .await
    .map_err(|e| format!("publishing failed: {e}"))?
    .map_err(text)?;
    match published {
        Published::NoRemote => Ok(json!({
            "state": "noRemote",
            "branch": task.branch(),
            "base": task.pr_target(),
            "message": format!(
                "repo `{}` has no origin remote: nothing was pushed and no pull request was opened; the work stays on branch {}",
                task.repo,
                task.branch()
            )
        })),
        Published::Opened { url, number } => {
            if let Some(number) = number {
                let pr = Pr {
                    number,
                    url: Some(url.clone()),
                    checks: Checks::Pending,
                    merge_sha: None,
                };
                let change = move |t: Task| Ok(Task { pr: Some(pr), ..t });
                store.update_task(&task.name, change).await.map_err(text)?;
            }
            hooks::fire(
                ctx,
                "pr.opened",
                json!({"task": task, "url": url, "number": number, "base": task.pr_target()}),
            );
            let account = account.map(|a| json!({"login": a.login, "host": a.host}));
            Ok(json!({
                "state": "opened", "url": url, "number": number, "base": task.pr_target(),
                "account": account
            }))
        }
    }
}
