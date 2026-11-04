//! The git questions the registry asks about a folder. Blocking: call from
//! `spawn_blocking` (as `RepoRegistry` does) or from code that is not async.

use std::path::{Path, PathBuf};

use crate::cmd::run_blocking;
use crate::{Error, Result};

const ORIGIN_PREFIX: &str = "origin/";
const DETACHED: &str = "HEAD";
const TOWNHALL_BRANCH: &str = crate::model::TASK_BRANCH_PREFIX;

fn not_top(path: &Path) -> Error {
    Error::InvalidInput(format!(
        "{} is not the top folder of a git repo",
        path.display()
    ))
}

fn git_path(path: &Path, flag: &str) -> Option<PathBuf> {
    let out = run_blocking("git", &["rev-parse", flag], Some(path)).ok()?;
    path.join(out).canonicalize().ok()
}

/// Ok when the canonical `path` is the top folder of a git work tree, is not inside
/// townhall's own `worktrees` folder, and is not a linked worktree on a `townhall/*` branch.
/// A missing `git` is its own error.
pub fn check_top_folder(path: &Path, worktrees: &Path) -> Result<()> {
    let top = match run_blocking("git", &["rev-parse", "--show-toplevel"], Some(path)) {
        Ok(top) => top,
        Err(Error::Command { .. }) => return Err(not_top(path)),
        Err(e) => return Err(e),
    };
    let top = Path::new(&top).canonicalize().map_err(|_| not_top(path))?;
    let owned = worktrees
        .canonicalize()
        .unwrap_or_else(|_| worktrees.to_path_buf());
    if top != path || path.starts_with(&owned) {
        return Err(not_top(path));
    }
    let linked = git_path(path, "--git-dir") != git_path(path, "--git-common-dir");
    let branch = symbolic_ref(path, "HEAD").unwrap_or_default();
    if linked && branch.starts_with(TOWNHALL_BRANCH) {
        return Err(not_top(path));
    }
    Ok(())
}

fn symbolic_ref(path: &Path, name: &str) -> Option<String> {
    run_blocking(
        "git",
        &["symbolic-ref", "--quiet", "--short", name],
        Some(path),
    )
    .ok()
    .filter(|s| !s.is_empty())
}

fn resolves(path: &Path, rev: &str) -> bool {
    let spec = format!("{rev}^{{commit}}");
    run_blocking(
        "git",
        &["rev-parse", "--verify", "--quiet", &spec],
        Some(path),
    )
    .is_ok()
}

/// origin's HEAD branch (as the local branch when one exists), else the current branch,
/// else `HEAD` for a detached checkout.
pub fn detect_base_branch(path: &Path) -> String {
    if let Some(remote) = symbolic_ref(path, "refs/remotes/origin/HEAD") {
        let local = remote.strip_prefix(ORIGIN_PREFIX).unwrap_or(&remote);
        if resolves(path, &format!("refs/heads/{local}")) {
            return local.to_string();
        }
        return remote;
    }
    symbolic_ref(path, "HEAD").unwrap_or_else(|| DETACHED.to_string())
}

/// A base branch someone typed: no option-like or blank names, and it must resolve to a commit.
pub fn verify_branch(path: &Path, branch: &str) -> Result<()> {
    let bad_shape = branch.is_empty()
        || branch.starts_with('-')
        || branch.chars().any(|c| c.is_whitespace() || c.is_control());
    if bad_shape {
        return Err(Error::InvalidInput(format!(
            "invalid base branch `{branch}`"
        )));
    }
    if resolves(path, branch) {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "base branch `{branch}` not found in {}",
            path.display()
        )))
    }
}
