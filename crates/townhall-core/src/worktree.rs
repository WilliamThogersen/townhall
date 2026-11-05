use std::path::{Path, PathBuf};

use crate::cmd::{run, validate_name};
use crate::{Error, Result, Workspace};

const BRANCH_PREFIX: &str = "townhall/";

/// A git worktree owned by one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
}

fn describe(ws: &Workspace, name: &str) -> Worktree {
    Worktree {
        name: name.to_string(),
        branch: format!("{BRANCH_PREFIX}{name}"),
        path: ws.worktrees_dir().join(name),
    }
}

/// Create `~/.townhall/worktrees/<repo-key>/<name>` on a new branch `townhall/<name>`.
pub async fn create(ws: &Workspace, name: &str, base: &str) -> Result<Worktree> {
    validate_name(name)?;
    let wt = describe(ws, name);
    ws.home().create_dir(&ws.worktrees_dir())?;
    let path = wt.path.to_string_lossy().into_owned();
    run(
        "git",
        &["worktree", "add", "-b", &wt.branch, &path, base],
        Some(ws.repo()),
    )
    .await?;
    tracing::info!(
        target: "townhall::git", event = "worktree.create", agent = %name, base = %base,
        path = %path, "worktree created"
    );
    Ok(wt)
}

/// Remove the worktree and delete its branch.
pub async fn remove(ws: &Workspace, name: &str) -> Result<()> {
    validate_name(name)?;
    let wt = describe(ws, name);
    let path = wt.path.to_string_lossy().into_owned();
    run(
        "git",
        &["worktree", "remove", "--force", &path],
        Some(ws.repo()),
    )
    .await?;
    run("git", &["branch", "-D", &wt.branch], Some(ws.repo())).await?;
    tracing::info!(
        target: "townhall::git", event = "worktree.remove", agent = %name, path = %path,
        "worktree removed"
    );
    Ok(())
}

/// List townhall-managed worktrees (rediscovered from git, so restarts lose nothing).
pub async fn list(ws: &Workspace) -> Result<Vec<Worktree>> {
    // git reports canonical paths (e.g. /private/var on macOS), so compare against one.
    let root = match ws.worktrees_dir().canonicalize() {
        Ok(root) => root,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(source) => {
            return Err(Error::Io {
                context: format!("worktrees dir {}", ws.worktrees_dir().display()),
                source,
            })
        }
    };
    let out = run("git", &["worktree", "list", "--porcelain"], Some(ws.repo())).await?;
    let found = out
        .split("\n\n")
        .filter_map(|block| {
            let path = block.lines().find_map(|l| l.strip_prefix("worktree "))?;
            let name = Path::new(path)
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            Some(describe(ws, &name))
        })
        .collect();
    Ok(found)
}
