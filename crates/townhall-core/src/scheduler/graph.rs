use std::collections::HashMap;

use crate::clock::parse_rfc3339;
use crate::cmd::{probe, run};
use crate::context::Context;
use crate::logs::task_fingerprints;
use crate::model::Task;
use crate::store::Landed;
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

async fn changed_files(
    ctx: &Context,
    task: &Task,
    launch: Option<&TaskLaunch>,
) -> Result<Vec<String>> {
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

const MERGE_LOG: &str = "5000";

fn task_names(subject: &str) -> Vec<String> {
    let source = subject.split(" into ").next().unwrap_or(subject);
    source
        .split("townhall/")
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn pr_number(subject: &str) -> Option<u64> {
    let (_, rest) = subject.split_once("pull request #")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

async fn base_ref(repo: &std::path::Path, base: &str) -> Result<String> {
    let remote = format!("origin/{base}");
    let found = probe(
        "git",
        &["rev-parse", "--verify", "-q", remote.as_str()],
        Some(repo),
    )
    .await;
    Ok(match found {
        Ok(_) => remote,
        Err(_) => base.to_string(),
    })
}

async fn land_repo(ctx: &Context, name: &str, tasks: &[Task]) -> Result<()> {
    let store = ctx.store()?;
    let repo = ctx.repo(name).await?;
    let mut bases: Vec<&str> = tasks
        .iter()
        .filter(|t| t.repo == name)
        .map(|t| t.base_branch.as_str())
        .collect();
    bases.sort_unstable();
    bases.dedup();
    let mut refs = vec![];
    for base in bases {
        refs.push(base_ref(&repo.path, base).await?);
    }
    let mut args = vec!["rev-parse"];
    args.extend(refs.iter().map(String::as_str));
    let head = run("git", &args, Some(&repo.path))
        .await?
        .replace('\n', ",");
    if !store.landing_scan_due(name, &head).await? {
        return Ok(());
    }
    let mut args = vec!["log", "--merges", "-n", MERGE_LOG, "--format=%H%x1f%s"];
    args.extend(refs.iter().map(String::as_str));
    let log = run("git", &args, Some(&repo.path)).await?;
    let mut landed = Landed {
        head,
        ..Landed::default()
    };
    let mut merge_of: HashMap<String, String> = HashMap::new();
    for line in log.lines() {
        let Some((sha, subject)) = line.split_once('\x1f') else {
            continue;
        };
        let number = pr_number(subject);
        landed.prs.extend(number);
        let named = task_names(subject);
        let by_pr = tasks
            .iter()
            .filter(|t| {
                t.repo == name && number.is_some() && t.pr.as_ref().map(|p| p.number) == number
            })
            .map(|t| t.name.clone());
        for task in named.into_iter().chain(by_pr) {
            landed.tasks.push(task.clone());
            merge_of
                .entry(task.to_lowercase())
                .or_insert_with(|| sha.to_string());
        }
    }
    let wanted: Vec<String> = merge_of
        .keys()
        .filter_map(|k| {
            tasks
                .iter()
                .find(|t| t.repo == name && t.name.eq_ignore_ascii_case(k))
        })
        .map(|t| t.name.clone())
        .collect();
    for task in store.tasks_missing_files(wanted).await? {
        let sha = &merge_of[&task.to_lowercase()];
        let parent = format!("{sha}^1");
        let out = probe(
            "git",
            &["diff", "--name-only", parent.as_str(), sha.as_str(), "--"],
            Some(&repo.path),
        )
        .await
        .map_err(|e| Error::InvalidInput(format!("cannot diff {sha}: {e}")))?;
        let mut files: Vec<String> = out.lines().map(str::to_string).collect();
        files.sort();
        files.dedup();
        landed.files.insert(task, files);
    }
    store.apply_landed(name, landed).await
}

async fn land(ctx: &Context) -> Result<Vec<String>> {
    let tasks = ctx.store()?.all_tasks().await?;
    let mut repos: Vec<&str> = tasks.iter().map(|t| t.repo.as_str()).collect();
    repos.sort_unstable();
    repos.dedup();
    let mut warnings = vec![];
    for name in repos {
        if let Err(e) = land_repo(ctx, name, &tasks).await {
            warnings.push(format!("graph: landing of `{name}` unknown: {e}"));
        }
    }
    Ok(warnings)
}

pub(super) async fn record(ctx: &Context) -> Result<Vec<String>> {
    let mut warnings = record_open(ctx).await?;
    warnings.extend(land(ctx).await?);
    Ok(warnings)
}

async fn record_open(ctx: &Context) -> Result<Vec<String>> {
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
