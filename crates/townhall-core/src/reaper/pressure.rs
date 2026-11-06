//! A pass on a nearly full disk: below the floor, the ended tasks go oldest first, grace
//! or not, one at a time, until free space is back above the floor.

use std::collections::HashMap;

use crate::model::{Task, TaskState};
use crate::{agent, clock, disk, Context, Result, Workspace};

use super::Reaped;

/// When `t` ended, as unix millis; a task without a recorded end counts from its creation.
fn ended_at(t: &Task, times: &HashMap<String, String>) -> i64 {
    times
        .get(&t.name)
        .and_then(|at| clock::parse_rfc3339(at))
        .or_else(|| clock::parse_rfc3339(&t.created_at))
        .unwrap_or(0)
}

/// Removes ended tasks' agents (session and worktree; the branch only when git calls it
/// merged) while free space is below the floor. Live work is never a candidate.
pub(super) async fn relieve(
    ctx: &Context,
    spaces: &[(String, Workspace)],
    tasks: &[Task],
    times: &HashMap<String, String>,
) -> Result<Reaped> {
    let mut reaped = Reaped::default();
    let floor_gb = ctx.settings().await?.min_free_disk_gb;
    if floor_gb == 0 {
        return Ok(reaped);
    }
    let mut free = match ctx.free_bytes() {
        Ok(free) => free,
        Err(e) => {
            reaped
                .warnings
                .push(format!("free disk space not checked: {e}"));
            return Ok(reaped);
        }
    };
    let mut ended: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.state == TaskState::Ended)
        .collect();
    ended.sort_by_key(|t| (ended_at(t, times), t.name.clone()));
    for task in ended {
        if !disk::below_floor(free, floor_gb) {
            break;
        }
        let Some((repo, ws)) = spaces
            .iter()
            .find(|(repo, _)| repo.eq_ignore_ascii_case(&task.repo))
        else {
            continue;
        };
        match agent::retire(ws, &task.name).await {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                reaped.warnings.push(format!(
                    "agent `{}` of repo `{repo}` not removed under disk pressure: {e}",
                    task.name
                ));
                continue;
            }
        }
        let before = free;
        reaped.removed.push(task.name.clone());
        free = match ctx.free_bytes() {
            Ok(free) => free,
            Err(e) => {
                reaped
                    .warnings
                    .push(format!("free disk space not checked: {e}"));
                return Ok(reaped);
            }
        };
        tracing::info!(
            target: "townhall::task", event = "reaper.pressure_removed", agent = %task.name,
            repo, free_before = before, free_after = free, floor_gb,
            "ended agent removed to free disk space"
        );
    }
    Ok(reaped)
}
