//! `hand_back`: the one tool a task's agent gets, on its own endpoint
//! (`/api/tasks/{name}/mcp`). It ends the agent's run and queues the same task, in the same
//! worktree and branch, for another agent type with the reason as part of its brief.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::cmd::run;
use crate::context::Context;
use crate::model::{Task, MAX_HANDOFFS};
use crate::worktree;

use super::checks::{self, one_line, Checked};
use super::{ToolDef, MAX_NAME_LEN, MAX_PROMPT_LEN, NAME_PATTERN};

pub const HAND_BACK: &str = "hand_back";

pub const INSTRUCTIONS: &str = "townhall tools: hand_back sends your task to another agent type with the reason, then ends your run. Use it when the work needs another type, for example a failing check that the type that wrote the code must fix. add_artifact attaches proof (a screenshot, log, note or report) to your own task outside the repo, and list_artifacts reads the proof of any task of this conversation; proof can be added but never changed or deleted. open_pr pushes your branch and opens its pull or merge request. open_pr publishes as the GitHub account in townhall's githubAccount setting: it switches gh to that account, sets the worktree's git author and committer to it and fails with a clear message when that is not possible, so never ask the user which GitHub account to use. When your task is landing the work: call open_pr, watch CI with `gh pr checks --watch`, fix a failing check and push again (or hand_back to the type that can), rebase onto the base branch when it moved, and run `gh pr merge` only once every check is green. Never push to the base branch.";

const MAX_REASON_LEN: u64 = 2_000;
const MAX_COMMITS: usize = 30;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HandBackInput {
    to: String,
    reason: String,
    #[serde(default)]
    summary: Option<String>,
}

pub fn tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: HAND_BACK,
        description: "Hand this task back to another agent type: your run ends and the task is queued again for that type, in this same worktree and branch, with your reason and a summary of what you did as its brief. Commit your work first. Stop right after calling it.",
        input_schema: schema(),
        read_only: true,
    }]
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "to": {
                "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN,
                "description": "The agent type that takes the task."
            },
            "reason": {
                "type": "string", "minLength": 1, "maxLength": MAX_REASON_LEN,
                "description": "Why it goes back, with what must change: for example the failing check and its error."
            },
            "summary": {
                "type": "string", "maxLength": MAX_PROMPT_LEN,
                "description": "What you did so far. Your commits are added for you."
            }
        },
        "required": ["to", "reason"],
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

fn listed(names: &[String]) -> String {
    match names.is_empty() {
        true => "none".to_string(),
        false => names.join(", "),
    }
}

pub(super) async fn hand_back(ctx: &Context, task: &str, input: HandBackInput) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    let task = store
        .find_task(task)
        .await
        .map_err(text)?
        .ok_or_else(|| format!("no task named `{task}`"))?;
    let reason = one_line(&input.reason);
    if reason.is_empty() {
        return Err("`reason` must not be blank: say what the next type must do".into());
    }
    let own = store.agent_type(&task.agent).await.map_err(text)?;
    let names: Vec<String> = store
        .agent_types()
        .await
        .map_err(text)?
        .into_iter()
        .map(|t| t.def.name)
        .collect();
    let to = input.to.trim();
    if !names.iter().any(|n| n.eq_ignore_ascii_case(to)) {
        return Err(format!(
            "unknown agent type `{to}`: you can hand back to: {}",
            listed(&allowed(own.as_ref(), &task, &names))
        ));
    }
    let targets = allowed(own.as_ref(), &task, &names);
    if !targets.iter().any(|n| n.eq_ignore_ascii_case(to)) {
        return Err(format!(
            "`{}` may not hand back to `{to}`: you can hand back to: {}",
            task.agent,
            listed(&targets)
        ));
    }
    let remedy = "a handed-back task keeps its own `after` chain, so hand back to a type that does not need them, or ask the planner to create a task of that type after those tasks";
    checks::requirements(ctx, to, &task.after, remedy).await?;
    let summary = summary_of(ctx, &task, input.summary.as_deref()).await;
    let handoff = store
        .hand_back(&task.name, to, &reason, &summary)
        .await
        .map_err(text)?;
    Ok(json!({
        "handedBackTo": handoff.to,
        "maxHandoffs": MAX_HANDOFFS,
        "next": "your run ends: stop now"
    }))
}

fn allowed(own: Option<&crate::model::AgentType>, task: &Task, names: &[String]) -> Vec<String> {
    match own {
        Some(own) => own.def.hand_back_targets(&task.agent, names),
        None => names
            .iter()
            .filter(|n| !n.eq_ignore_ascii_case(&task.agent))
            .cloned()
            .collect(),
    }
}

pub(crate) async fn summary_of(ctx: &Context, task: &Task, told: Option<&str>) -> String {
    let told = told.map(str::trim).filter(|s| !s.is_empty());
    let commits = commits(ctx, task).await.unwrap_or_default();
    let log = match commits.is_empty() {
        true => None,
        false => Some(format!(
            "Commits on the branch so far:\n{}",
            commits
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    };
    [told.map(str::to_string), log]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) async fn worktree_of(ctx: &Context, task: &Task) -> Option<PathBuf> {
    let repo = ctx.repo(&task.repo).await.ok()?;
    let ws = ctx.workspace(&repo).ok()?;
    let tree = worktree::list(&ws)
        .await
        .ok()?
        .into_iter()
        .find(|w| w.name == task.name)?;
    Some(tree.path)
}

async fn commits(ctx: &Context, task: &Task) -> Option<Vec<String>> {
    let store = ctx.store().ok()?;
    let start = store.task_launch(&task.name).await.ok()??.start_commit;
    let path = worktree_of(ctx, task).await?;
    let range = format!("{start}..HEAD");
    let max = format!("--max-count={MAX_COMMITS}");
    let args = ["log", "--format=%s", max.as_str(), range.as_str()];
    let out = run("git", &args, Some(&path)).await.ok()?;
    Some(out.lines().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::INSTRUCTIONS;

    #[test]
    fn the_instructions_settle_the_github_account_for_landing() {
        for needle in [
            "githubAccount",
            "never ask the user which GitHub account",
            "gh pr checks",
            "gh pr merge",
            "rebase",
        ] {
            assert!(INSTRUCTIONS.contains(needle), "{needle}");
        }
        assert!(!INSTRUCTIONS.contains('\u{2014}') && !INSTRUCTIONS.contains('\u{2013}'));
    }
}
