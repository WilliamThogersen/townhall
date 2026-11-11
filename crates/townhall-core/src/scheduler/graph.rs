//! Feeds the history graph (`store/unit_graph.rs`): once a task ended, merged or failed, the
//! files its branch changed and the log fingerprints that named it are recorded as edges. The
//! same pass backfills older tasks, a few per pass; a task whose branch is gone is recorded
//! without files. Read only: `git diff --name-only`, never a write to the repo.

use std::collections::HashMap;

use crate::clock::parse_rfc3339;
use crate::cmd::{probe, run};
use crate::context::Context;
use crate::logs::task_fingerprints;
use crate::model::Task;
use crate::{Error, Result};

use super::TaskLaunch;

const BATCH: usize = 20;
const REFS: &str = "refs/heads/";

async fn branch_exists(repo: &std::path::Path, branch: &str) -> Result<bool> {
    let pattern = format!("{REFS}{branch}");
    let out = run(
        "git",
        &["for-each-ref", "--format=%(refname)", pattern.as_str()],
        Some(repo),
    )
    .await?;
    Ok(!out.is_empty())
}

async fn changed_files(ctx: &Context, task: &Task, launch: Option<&TaskLaunch>) -> Result<Vec<String>> {
    let repo = ctx.repo(&task.repo).await?;
    let branch = task.branch();
    if !branch_exists(&repo.path, &branch).await? {
        return Ok(vec![]);
    }
    let range = match launch {
        Some(l) => format!("{}..{branch}", l.start_commit),
        None => format!("{}...{branch}", task.base_branch),
    };
    let out = probe(
        "git",
        &["diff", "--name-only", range.as_str(), "--"],
        Some(&repo.path),
    )
    .await
    .map_err(|e| Error::InvalidInput(format!("cannot diff {range}: {e}")))?;
    let mut files: Vec<String> = out.lines().map(str::to_string).collect();
    files.sort();
    files.dedup();
    Ok(files)
}

async fn fingerprints(ctx: &Context, task: &Task) -> Result<Vec<String>> {
    let dir = ctx.home().logs_dir()?;
    let since = parse_rfc3339(&task.created_at).unwrap_or(0);
    let name = task.name.clone();
    let found = tokio::task::spawn_blocking(move || task_fingerprints(&dir, since, &name)).await;
    found.map_err(|e| Error::InvalidInput(format!("cannot read the logs: {e}")))
}

pub(super) async fn record(ctx: &Context) -> Result<Vec<String>> {
    let store = ctx.store()?;
    let open = store.unrecorded_graph_tasks().await?;
    if open.is_empty() {
        return Ok(vec![]);
    }
    let launches: HashMap<String, TaskLaunch> = store
        .task_launches()
        .await?
        .into_iter()
        .map(|l| (l.name.to_lowercase(), l))
        .collect();
    let mut warnings = vec![];
    for task in open.iter().take(BATCH) {
        let launch = launches.get(&task.name.to_lowercase());
        let files = match changed_files(ctx, task, launch).await {
            Ok(files) => files,
            Err(e) => {
                warnings.push(format!("graph: files of `{}` unknown: {e}", task.name));
                continue;
            }
        };
        let prints = fingerprints(ctx, task).await?;
        store.record_task_graph(&task.name, files, prints).await?;
    }
    Ok(warnings)
}
