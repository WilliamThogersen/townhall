use std::path::{Path, PathBuf};

use crate::cmd::{run, validate_name};
use crate::Result;

const BRANCH_PREFIX: &str = "townhall/";
const WORKTREE_DIR: &str = ".townhall/worktrees";

/// A git worktree owned by one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
}

fn describe(repo: &Path, name: &str) -> Worktree {
    Worktree {
        name: name.to_string(),
        branch: format!("{BRANCH_PREFIX}{name}"),
        path: repo.join(WORKTREE_DIR).join(name),
    }
}

/// Create `<repo>/.townhall/worktrees/<name>` on a new branch `townhall/<name>`.
pub async fn create(repo: &Path, name: &str, base: &str) -> Result<Worktree> {
    validate_name(name)?;
    let wt = describe(repo, name);
    let path = wt.path.to_string_lossy().into_owned();
    run(
        "git",
        &["worktree", "add", "-b", &wt.branch, &path, base],
        Some(repo),
    )
    .await?;
    Ok(wt)
}

/// Remove the worktree and delete its branch.
pub async fn remove(repo: &Path, name: &str) -> Result<()> {
    validate_name(name)?;
    let wt = describe(repo, name);
    let path = wt.path.to_string_lossy().into_owned();
    run("git", &["worktree", "remove", "--force", &path], Some(repo)).await?;
    run("git", &["branch", "-D", &wt.branch], Some(repo)).await?;
    Ok(())
}

/// List townhall-managed worktrees (rediscovered from git, so restarts lose nothing).
pub async fn list(repo: &Path) -> Result<Vec<Worktree>> {
    let out = run("git", &["worktree", "list", "--porcelain"], Some(repo)).await?;
    // git reports canonical paths (e.g. /private/var on macOS), so compare against one.
    let root = repo
        .canonicalize()
        .map_err(|source| crate::Error::Spawn {
            program: "canonicalize".into(),
            source,
        })?
        .join(WORKTREE_DIR);
    let found = out
        .split("\n\n")
        .filter_map(|block| {
            let path = block.lines().find_map(|l| l.strip_prefix("worktree "))?;
            let name = Path::new(path)
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some(describe(repo, &name))
        })
        .collect();
    Ok(found)
}
