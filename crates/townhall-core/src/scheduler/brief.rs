//! A task's brief: its whole prompt in `<home>/tasks/<name>/brief.md`, never in the repo or
//! the worktree. The agent starts with a one-line order to read it, so a prompt of any size
//! fits in the tmux command, and nothing in the prompt can reach claude's flags.

use std::path::{Path, PathBuf};

use crate::fsutil::write_atomic;
use crate::model::{Handoff, Task};
use crate::{Error, Home, Result};

use super::plan;

const TASKS_DIR: &str = "tasks";
const BRIEF_FILE: &str = "brief.md";

/// `<home>/tasks/<name>/brief.md`, absolute.
pub fn brief_file(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(TASKS_DIR).join(name).join(BRIEF_FILE);
    std::path::absolute(&path).unwrap_or(path)
}

/// The one-line prompt the agent starts with.
pub fn order(brief: &Path) -> String {
    format!(
        "read your task brief at {} and do it. it is the full task.",
        brief.display()
    )
}

/// Writes (or rewrites) the task's brief with its hand backs and attachments' paths; returns
/// its path.
pub async fn write(
    home: &Home,
    task: &Task,
    files: &[PathBuf],
    handoffs: &[Handoff],
) -> Result<PathBuf> {
    let path = brief_file(home, &task.name);
    let text = plan::brief_text(&task.title, &task.prompt, files, handoffs);
    let (home, target) = (home.clone(), path.clone());
    tokio::task::spawn_blocking(move || {
        if let Some(dir) = target.parent() {
            home.create_dir(dir)?;
        }
        write_atomic(&target, text.as_bytes(), true)
    })
    .await
    .map_err(|e| Error::io("write a task brief", std::io::Error::other(e.to_string())))??;
    Ok(path)
}
