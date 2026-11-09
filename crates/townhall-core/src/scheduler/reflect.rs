//! The reflect step: with `reflectAfterTasks` on, an `ended` task is handed on once to the
//! agent type `reflector`, which runs in the task's own worktree and branch, reads what the
//! task changed and records short, deduplicated lessons in the repo's `CLAUDE.md` (or
//! `docs/lessons.md`, linked from it). It is the same hand back an agent makes (`Store::hand_on`),
//! so the scheduler resumes it like any handed-back task and its thread shows it. With the
//! setting off (the default) nothing here reads a task or writes a type.

use crate::context::Context;
use crate::mcp;
use crate::model::{Task, TaskState, MAX_HANDOFFS};
use crate::store::Store;
use crate::{Error, Result};

/// The agent type the step hands a finished task to.
pub const REFLECTOR: &str = "reflector";

/// The type townhall saves when the step needs it and none is saved. A saved `reflector` is
/// never replaced: edit it like any agent type.
pub const REFLECTOR_SOURCE: &str = r##"# records what a finished task taught, in the repo's memory files
name = "reflector"
description = "records lessons from a finished task in CLAUDE.md or docs/lessons.md"
adapter = "claude"
model = "haiku"
permission_mode = "default"
can_hand_back_to = []
tools = [
  "Read",
  "Grep",
  "Glob",
  "Edit(/CLAUDE.md)",
  "Edit(/docs/lessons.md)",
  "Write(/CLAUDE.md)",
  "Write(/docs/lessons.md)",
  "Bash(git diff:*)",
  "Bash(git log:*)",
  "Bash(git show:*)",
  "Bash(git status:*)",
  "Bash(git add CLAUDE.md)",
  "Bash(git add docs/lessons.md)",
  "Bash(git commit -m *)",
]
prompt = """
You record what a finished task taught, so the next agents start smarter. Your brief says what the task was and how to see what it changed.
- Read the brief, then look at the task's commits with git log and git diff. Do not run tests and do not change code.
- Write down only durable, repo-specific lessons: how to build, test or run, conventions, traps, files that matter, and decisions with their reason. Skip anything that only concerns this one task and anything obvious from the code.
- Be concise: one short bullet per lesson, at most five new bullets.
- Put them under a "Lessons" heading in CLAUDE.md. When CLAUDE.md is missing or over about 200 lines, use docs/lessons.md instead and make sure CLAUDE.md has one line linking to it.
- Read the file first and never add a duplicate: when a lesson is already there in any wording, leave it or sharpen that bullet.
- Edit only CLAUDE.md and docs/lessons.md, nothing else.
- Commit with git add on the files you changed, then git commit -m "docs: record lessons from <task>". When nothing is worth recording, change nothing and say so.
- Never push.
"""
"##;

const REASON: &str = "the task ended: record its lessons in CLAUDE.md or docs/lessons.md as your type says, then stop";

/// Hands every `ended` task that was not reflected on yet to `reflector`. Returns the names
/// it queued and, per task it could not hand on, a warning. Does nothing while the setting is
/// off.
pub(super) async fn queue(ctx: &Context, tasks: &[Task]) -> Result<(Vec<String>, Vec<String>)> {
    if !ctx.settings().await?.reflect_after_tasks {
        return Ok((vec![], vec![]));
    }
    let store = ctx.store()?;
    let (mut queued, mut warnings) = (vec![], vec![]);
    let mut reflector: Option<String> = None;
    for task in tasks.iter().filter(|t| due(t)) {
        if !wants_reflection(ctx, task).await? {
            continue;
        }
        let to = match &reflector {
            Some(name) => name.clone(),
            None => {
                let name = reflector_name(store).await?;
                reflector = Some(name.clone());
                name
            }
        };
        let summary = summary(ctx, task).await?;
        match store.hand_on(&task.name, &to, REASON, &summary).await {
            Ok(_) => queued.push(task.name.clone()),
            Err(Error::Conflict(why)) => {
                warnings.push(format!("`{}` not reflected on: {why}", task.name))
            }
            Err(e) => return Err(e),
        }
    }
    Ok((queued, warnings))
}

fn due(task: &Task) -> bool {
    task.state == TaskState::Ended && !task.agent.eq_ignore_ascii_case(REFLECTOR)
}

/// Once per task, and only while its worktree is still there: a task whose agent was already
/// retired has nothing to read, and a start would make a fresh worktree off the base branch.
async fn wants_reflection(ctx: &Context, task: &Task) -> Result<bool> {
    let handoffs = ctx.store()?.handoffs(&task.name).await?;
    let done = handoffs
        .iter()
        .any(|h| h.to.eq_ignore_ascii_case(REFLECTOR));
    Ok(!done && handoffs.len() < MAX_HANDOFFS && mcp::worktree_of(ctx, task).await.is_some())
}

/// The stored name of the `reflector` type, saved from [`REFLECTOR_SOURCE`] when none exists.
async fn reflector_name(store: &Store) -> Result<String> {
    if let Some(found) = store.agent_type(REFLECTOR).await? {
        return Ok(found.def.name);
    }
    let (saved, _) = store.save_agent_type(REFLECTOR_SOURCE).await?;
    Ok(saved.def.name)
}

async fn summary(ctx: &Context, task: &Task) -> Result<String> {
    let start = ctx.store()?.task_launch(&task.name).await?;
    let told = match start {
        Some(launch) => format!(
            "Task: {}\n\nSee what it changed with `git diff {}..HEAD`.",
            task.title, launch.start_commit
        ),
        None => format!("Task: {}", task.title),
    };
    Ok(mcp::summary_of(ctx, task, Some(&told)).await)
}
