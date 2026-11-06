use std::path::Path;

use crate::cmd::{run_blocking, run_blocking_env};
use crate::home::REPOS_DIR;
use crate::repos::no_prompt_env;
use crate::{Result, Workspace};

const REMOTE: &str = "origin";

/// The ref a new task worktree should start from. Fetches `origin` without ever prompting;
/// when the fetch fails (offline) it logs a warning and returns `base` untouched.
///
/// A local branch `base` that is behind `origin/<base>` gives `origin/<base>`, and in a clone
/// townhall made itself (`<home>/repos`) the local branch is fast-forwarded as well. A folder
/// the user owns is never changed beyond the fetch, and a branch with commits the remote
/// lacks keeps being the base.
pub async fn fresh_base(ws: &Workspace, base: &str) -> String {
    let (ws, base) = (ws.clone(), base.to_string());
    tokio::task::spawn_blocking(move || refresh(&ws, &base))
        .await
        .expect("the base refresh task panicked")
}

fn refresh(ws: &Workspace, base: &str) -> String {
    let repo = ws.repo();
    if run_blocking("git", &["remote", "get-url", REMOTE], Some(repo)).is_err() {
        return base.to_string();
    }
    if let Err(e) = fetch(repo) {
        tracing::warn!(
            target: "townhall::git", event = "worktree.fetch_failed", repo = %repo.display(),
            error = %e, "fetch failed, using the local state"
        );
        return base.to_string();
    }
    let remote_ref = format!("{REMOTE}/{base}");
    let local_ref = format!("refs/heads/{base}");
    let remote_full = format!("refs/remotes/{remote_ref}");
    if !exists(repo, &local_ref) || !exists(repo, &remote_full) {
        return base.to_string();
    }
    let behind = is_ancestor(repo, &local_ref, &remote_full);
    if !behind || is_ancestor(repo, &remote_full, &local_ref) {
        return base.to_string();
    }
    if !is_managed(ws) {
        return remote_ref;
    }
    match fast_forward(repo, base, &remote_ref) {
        Ok(()) => base.to_string(),
        Err(e) => {
            tracing::warn!(
                target: "townhall::git", event = "worktree.fast_forward_failed",
                repo = %repo.display(), branch = %base, error = %e,
                "could not fast-forward the clone, branching from the remote"
            );
            remote_ref
        }
    }
}

fn fetch(repo: &Path) -> Result<String> {
    let envs = no_prompt_env();
    run_blocking_env("git", &["fetch", "--quiet", REMOTE], Some(repo), &envs)
}

fn exists(repo: &Path, full_ref: &str) -> bool {
    run_blocking(
        "git",
        &["rev-parse", "--verify", "--quiet", full_ref],
        Some(repo),
    )
    .is_ok()
}

fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> bool {
    run_blocking(
        "git",
        &["merge-base", "--is-ancestor", ancestor, descendant],
        Some(repo),
    )
    .is_ok()
}

fn is_managed(ws: &Workspace) -> bool {
    let repos = ws.home().root().join(REPOS_DIR);
    repos
        .canonicalize()
        .is_ok_and(|repos| ws.repo().starts_with(repos))
}

fn fast_forward(repo: &Path, branch: &str, remote_ref: &str) -> Result<()> {
    let current = run_blocking("git", &["branch", "--show-current"], Some(repo))?;
    let args: Vec<&str> = if current == branch {
        vec!["merge", "--ff-only", "--quiet", remote_ref]
    } else {
        vec!["branch", "--force", branch, remote_ref]
    };
    run_blocking("git", &args, Some(repo)).map(|_| ())
}
