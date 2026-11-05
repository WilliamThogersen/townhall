//! `GET /api/stats` (unit 2.7): what shipped from `townhall/*` branches into each repo's base
//! branch, summed across repos.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use axum::extract::{Query, State};
use axum::response::Response;
use serde::Serialize;
use tokio::task::JoinSet;
use townhall_core::model::Repo;
use townhall_core::stats::{merge_days, CacheKey, ShippedCache, ShippedDay};

use super::agents::RepoQuery;
use crate::envelope::ok;
use crate::error::ApiError;
use crate::state::AppState;

/// The window the home screen shows.
const SINCE_DAYS: u32 = 730;

/// One cache per process. Its keys carry each repo's path and base branch, so a daemon the
/// desktop app restarts, or tests side by side, never read another repo's entry.
static CACHE: LazyLock<ShippedCache> = LazyLock::new(ShippedCache::default);
/// Why each skipped repo was skipped last, so polling does not log the same skip again.
static SKIPPED: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Mutex::default);

#[derive(Debug, Serialize)]
pub struct Stats {
    days: Vec<ShippedDay>,
}

/// `{ days }` for the repo named by `?repo=` (`404` when unknown), else for every repo. A repo
/// whose folder or base branch is gone is skipped with a warning.
pub async fn get(
    State(state): State<AppState>,
    Query(q): Query<RepoQuery>,
) -> Result<Response, ApiError> {
    let ctx = state.ctx();
    let repos = match q.repo.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        Some(name) => vec![ctx.repo(name).await?],
        None => ctx.repos().await?,
    };
    let days = shipped_across(repos).await;
    Ok(ok(Stats { days }))
}

/// Counts every repo at once and sums them.
async fn shipped_across(repos: Vec<Repo>) -> Vec<ShippedDay> {
    let mut counting = JoinSet::new();
    for repo in repos {
        counting.spawn(async move {
            let found = CACHE.shipped(&key(&repo)).await;
            (repo.name, found)
        });
    }
    let mut counted = vec![];
    while let Some(done) = counting.join_next().await {
        match done {
            Ok((repo, Ok(days))) => {
                skipped().remove(&repo);
                counted.push(days);
            }
            Ok((repo, Err(e))) => warn_once(&repo, &e.to_string()),
            Err(e) => tracing::warn!(
                target: "townhall::http",
                event = "stats.task_failed",
                error = %e,
                "stats for a repo stopped"
            ),
        }
    }
    merge_days(counted)
}

fn key(repo: &Repo) -> CacheKey {
    CacheKey {
        repo: repo.name.clone(),
        path: repo.path.clone(),
        base_branch: repo.base_branch.clone(),
        since_days: SINCE_DAYS,
    }
}

fn skipped() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    SKIPPED.lock().unwrap_or_else(PoisonError::into_inner)
}

fn warn_once(repo: &str, why: &str) {
    if first_time(&mut skipped(), repo, why) {
        tracing::warn!(
            target: "townhall::http",
            event = "stats.repo_skipped",
            repo,
            error = %townhall_core::redact::redact(why),
            "stats skip repo"
        );
    }
}

/// True when `repo` was not skipped for `why` last time; remembers it.
fn first_time(skipped: &mut HashMap<String, String>, repo: &str, why: &str) -> bool {
    if skipped.get(repo).map(String::as_str) == Some(why) {
        return false;
    }
    skipped.insert(repo.to_string(), why.to_string());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skip_is_logged_once_per_reason() {
        let mut skipped = HashMap::new();
        assert!(first_time(&mut skipped, "shop", "repo gone"));
        assert!(!first_time(&mut skipped, "shop", "repo gone"));
        assert!(first_time(&mut skipped, "site", "repo gone"));
        assert!(first_time(&mut skipped, "shop", "branch gone"));
        skipped.remove("shop");
        assert!(first_time(&mut skipped, "shop", "branch gone"));
    }
}
