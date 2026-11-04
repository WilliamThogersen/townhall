//! The git questions the registry asks about a folder. Blocking: call from
//! `spawn_blocking` (as `RepoRegistry` does) or from code that is not async.

use std::path::Path;

use crate::cmd::run_blocking;
use crate::{Error, Result};

const ORIGIN_PREFIX: &str = "origin/";
const DETACHED: &str = "HEAD";

/// Ok when `path` is inside a git work tree. A missing `git` is its own error.
pub fn check_work_tree(path: &Path) -> Result<()> {
    match run_blocking("git", &["rev-parse", "--is-inside-work-tree"], Some(path)) {
        Ok(out) if out == "true" => Ok(()),
        Ok(_) | Err(Error::Command { .. }) => Err(Error::InvalidInput(format!(
            "{} is not a git repository",
            path.display()
        ))),
        Err(e) => Err(e),
    }
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
