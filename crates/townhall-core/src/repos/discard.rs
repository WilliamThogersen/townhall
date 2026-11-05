//! Deleting a clone townhall made. Blocking (git, disk): call from `spawn_blocking`.

use std::path::Path;

use crate::cmd::run_blocking;
use crate::model::Repo;
use crate::{Error, Home, Result};

/// Ok when `repo` is a folder directly inside `<home>/repos` and holds nothing that would
/// be lost: no uncommitted changes, no commit that is on no remote, no linked worktree.
pub fn check_deletable(home: &Home, repo: &Repo) -> Result<()> {
    check_managed(home, &repo.path)?;
    let name = &repo.name;
    let dirty = run_blocking("git", &["status", "--porcelain"], Some(&repo.path))?;
    if !dirty.is_empty() {
        return Err(Error::Conflict(format!(
            "repo `{name}` has uncommitted changes: commit or discard them first"
        )));
    }
    let unpushed = run_blocking(
        "git",
        &["log", "--branches", "--not", "--remotes", "--oneline", "-n", "1"],
        Some(&repo.path),
    )?;
    if !unpushed.is_empty() {
        return Err(Error::Conflict(format!(
            "repo `{name}` has commits that are not pushed anywhere: push or merge them first"
        )));
    }
    let worktrees = run_blocking("git", &["worktree", "list", "--porcelain"], Some(&repo.path))?;
    if worktrees.lines().filter(|l| l.starts_with("worktree ")).count() > 1 {
        return Err(Error::Conflict(format!(
            "repo `{name}` still has worktrees: remove them first"
        )));
    }
    Ok(())
}

fn check_managed(home: &Home, path: &Path) -> Result<()> {
    let root = home.repos_dir()?.canonicalize().map_err(|e| Error::io("repos folder", e))?;
    let inside = path.canonicalize().ok().filter(|p| p.parent() == Some(root.as_path()));
    match inside {
        Some(_) => Ok(()),
        None => Err(Error::InvalidInput(format!(
            "{} is not a clone townhall made in {}: it is never deleted",
            path.display(),
            root.display()
        ))),
    }
}

/// Deletes the folder of a repo that passed `check_deletable`; checks the location again.
pub fn delete_clone(home: &Home, repo: &Repo) -> Result<()> {
    check_managed(home, &repo.path)?;
    std::fs::remove_dir_all(&repo.path)
        .map_err(|e| Error::io(format!("delete clone {}", repo.path.display()), e))
}
