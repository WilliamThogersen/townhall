use std::collections::HashSet;

use crate::clock::{now_rfc3339, parse_rfc3339};
use crate::context::Context;
use crate::model::{AgentEvent, EntryBody, Task, TaskState, VerifyPhase};
use crate::{tmux, Result};

use super::{transition, TaskLaunch};

const GRACE_MS: i64 = 60_000;

pub(super) const GONE: &str = "the agent session is gone";

fn owned_elsewhere(task: &Task) -> bool {
    let verifying = matches!(
        task.done.verify.as_ref().map(|v| v.phase),
        Some(VerifyPhase::Verifying | VerifyPhase::Failed)
    );
    verifying || task.blocked.is_some()
}

fn settled(task: &Task, launches: &[TaskLaunch], now: i64) -> bool {
    launches
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(&task.name))
        .and_then(|l| parse_rfc3339(&l.launched_at))
        .is_some_and(|at| now - at >= GRACE_MS)
}

pub(super) async fn reconcile(ctx: &Context, tasks: &[Task]) -> Result<(Vec<String>, Vec<String>)> {
    let working: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.state == TaskState::Working && !owned_elsewhere(t))
        .collect();
    if working.is_empty() {
        return Ok((vec![], vec![]));
    }
    let store = ctx.store()?;
    let launches = store.task_launches().await?;
    let now = parse_rfc3339(&now_rfc3339()).unwrap_or_default();
    let repos: HashSet<&str> = working.iter().map(|t| t.repo.as_str()).collect();
    let (mut ended, mut warnings) = (vec![], vec![]);
    for repo in repos {
        let live = match live_sessions(ctx, repo).await {
            Ok(live) => live,
            Err(e) => {
                warnings.push(format!("sessions of repo `{repo}` unknown: {e}"));
                continue;
            }
        };
        let gone = working
            .iter()
            .filter(|t| t.repo == repo && settled(t, &launches, now))
            .filter(|t| !live.iter().any(|n| n.eq_ignore_ascii_case(&t.name)));
        for task in gone {
            if transition(store, &task.name, TaskState::Working, TaskState::Ended)
                .await?
                .is_none()
            {
                continue;
            }
            let body = EntryBody::Agent {
                agent: task.name.clone(),
                event: AgentEvent::Ended,
                text: GONE.into(),
            };
            store.append_entry(&task.conversation_id, body).await?;
            tracing::info!(
                target: "townhall::task", event = "task.reconciled", task = %task.name,
                from = "working", to = "ended", reason = "no live agent session",
                "task ended: its agent session is gone"
            );
            ended.push(task.name.clone());
        }
    }
    Ok((ended, warnings))
}

async fn live_sessions(ctx: &Context, repo: &str) -> Result<Vec<String>> {
    let repo = ctx.repo(repo).await?;
    let ws = ctx.workspace(&repo)?;
    tmux::list(ws.key()).await
}
