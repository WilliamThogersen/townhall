//! Verification: a task whose agent ended under a `check` (the watcher marks it `verifying`
//! and leaves it running) has the check run in its worktree.
//!
//! - Exit 0: the task is `ended`, `verify` `passed`, with an `agent` `ended` entry.
//! - Anything else, a check that cannot start or that outlives `TIMEOUT` included, is a
//!   failure: `Store::check_failed` queues the task again (the next pass resumes it in the
//!   same worktree with the output in its brief, `retry n/max`) or, after `maxRetries`, marks
//!   it `needsYou` with the last output.
//!
//! A check runs on its own task so a slow one holds up neither starts nor merges. `RUNNING`
//! keeps the slow tick from starting the same check twice; after a restart a `verifying` task
//! simply runs its check again.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tokio::process::Command;

use crate::context::Context;
use crate::model::{AgentEvent, Done, EntryBody, Task, TaskState, Verify, VerifyPhase};
use crate::{Error, Result};

use super::plan;

/// A check that runs longer than this failed.
pub const TIMEOUT: Duration = Duration::from_secs(600);

/// The last failing output a task keeps, in characters: its tail says why it failed.
pub const MAX_OUTPUT: usize = 4_000;

static RUNNING: Mutex<Option<HashSet<(PathBuf, String)>>> = Mutex::new(None);

fn claim(key: &(PathBuf, String)) -> bool {
    let mut guard = RUNNING.lock().unwrap_or_else(|p| p.into_inner());
    guard.get_or_insert_with(HashSet::new).insert(key.clone())
}

fn release(key: &(PathBuf, String)) {
    let mut guard = RUNNING.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(running) = guard.as_mut() {
        running.remove(key);
    }
}

/// Tasks whose check is due: running, with a check, `verifying`.
fn due(tasks: &[Task]) -> Vec<&Task> {
    tasks
        .iter()
        .filter(|t| matches!(t.state, TaskState::Working | TaskState::NeedsYou))
        .filter(|t| t.done.check.is_some())
        .filter(|t| {
            let phase = t.done.verify.as_ref().map(|v| v.phase);
            phase == Some(VerifyPhase::Verifying)
        })
        .collect()
}

/// Starts the check of every task that is due and has none running. Returns their names.
pub(super) fn start(ctx: &Context, tasks: &[Task]) -> Vec<String> {
    let mut started = vec![];
    for task in due(tasks) {
        let key = (ctx.home().root().to_path_buf(), task.name.clone());
        if !claim(&key) {
            continue;
        }
        started.push(task.name.clone());
        let (ctx, task) = (ctx.clone(), task.clone());
        tokio::spawn(async move {
            if let Err(e) = verify(&ctx, &task).await {
                tracing::warn!(
                    target: "townhall::task", event = "task.verify_failed", task = %task.name,
                    error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                    "a task's check could not be judged"
                );
            }
            release(&key);
        });
    }
    started
}

async fn verify(ctx: &Context, task: &Task) -> Result<()> {
    let command = task.done.check.clone().unwrap_or_default();
    let dir = {
        let repo = ctx.repo(&task.repo).await?;
        ctx.workspace(&repo)?.worktrees_dir().join(&task.name)
    };
    let store = ctx.store()?;
    match run_check(&command, &dir).await {
        Ok(()) => passed(ctx, task).await,
        Err(output) => store.check_failed(&task.name, &output).await.map(|_| ()),
    }
}

/// `Err` is the failure as the next brief and the user read it: the output's tail, then why.
async fn run_check(command: &str, dir: &std::path::Path) -> std::result::Result<(), String> {
    let mut sh = Command::new("sh");
    sh.args(["-c", command]).current_dir(dir).kill_on_drop(true);
    let output = match tokio::time::timeout(TIMEOUT, sh.output()).await {
        Err(_) => {
            return Err(format!(
                "the check ran longer than {}s and was stopped",
                TIMEOUT.as_secs()
            ))
        }
        Ok(Err(e)) => {
            return Err(format!(
                "the check could not start in {}: {e}",
                dir.display()
            ))
        }
        Ok(Ok(output)) => output,
    };
    if output.status.success() {
        return Ok(());
    }
    let text = [&output.stdout, &output.stderr]
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let why = match output.status.code() {
        Some(code) => format!("exit code {code}"),
        None => "killed by a signal".to_string(),
    };
    Err(format!("{}\n({why})", tail(&text, MAX_OUTPUT))
        .trim_start()
        .to_string())
}

/// The last `max` characters of `text`, starting with `...` when it was cut.
fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let kept: String = text.chars().skip(count - max.saturating_sub(4)).collect();
    format!("...\n{kept}")
}

async fn passed(ctx: &Context, task: &Task) -> Result<()> {
    let store = ctx.store()?;
    let moved = "the task moved on";
    let change = |t: Task| {
        let verifying = t.done.verify.as_ref().map(|v| v.phase) == Some(VerifyPhase::Verifying);
        match verifying && matches!(t.state, TaskState::Working | TaskState::NeedsYou) {
            true => Ok(Task {
                state: TaskState::Ended,
                done: Done {
                    verify: Some(Verify {
                        phase: VerifyPhase::Passed,
                        retries: t.done.verify.as_ref().map_or(0, |v| v.retries),
                        output: None,
                    }),
                    ..t.done.clone()
                },
                ..t
            }),
            false => Err(Error::Conflict(moved.into())),
        }
    };
    match store.update_task(&task.name, change).await {
        Ok(_) => {
            let text = plan::capped(
                &format!(
                    "the check passed: {}",
                    task.done.check.as_deref().unwrap_or("")
                ),
                300,
            );
            let body = EntryBody::Agent {
                agent: task.name.clone(),
                event: AgentEvent::Ended,
                text,
            };
            store.append_entry(&task.conversation_id, body).await?;
            Ok(())
        }
        Err(Error::Conflict(m)) if m == moved => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_output_keeps_its_tail() {
        assert_eq!(tail("short", 100), "short");
        let long = format!("{}END", "x".repeat(200));
        let cut = tail(&long, 50);
        assert!(cut.starts_with("...\n") && cut.ends_with("END"), "{cut}");
        assert_eq!(cut.chars().count(), 50);
    }

    #[tokio::test]
    async fn a_failure_says_the_output_and_the_exit_code() {
        let dir = std::env::temp_dir();
        let err = run_check("echo out; echo err >&2; exit 4", &dir)
            .await
            .unwrap_err();
        assert_eq!(err, "out\nerr\n(exit code 4)");
        assert_eq!(run_check("true", &dir).await, Ok(()));
    }

    #[tokio::test]
    async fn a_missing_folder_is_a_failure_that_names_it() {
        let dir = std::env::temp_dir().join("th-verify-no-such-folder");
        let err = run_check("true", &dir).await.unwrap_err();
        assert!(err.contains("th-verify-no-such-folder"), "{err}");
    }
}
