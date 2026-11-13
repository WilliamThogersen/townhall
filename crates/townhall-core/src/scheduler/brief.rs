//! A task's brief: its whole prompt in `<home>/tasks/<name>/brief.md`, never in the repo or
//! the worktree. The agent starts with a one-line order to read it, so a prompt of any size
//! fits in the tmux command, and nothing in the prompt can reach claude's flags.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::fsutil::write_atomic;
use crate::model::{Handoff, Task};
use crate::plugins::declared;
use crate::{Error, Home, Result};

use super::plan;

const TASKS_DIR: &str = "tasks";
const BRIEF_FILE: &str = "brief.md";
const MCP_FILE: &str = "mcp.json";
const SETTINGS_FILE: &str = "claude-settings.json";
const PLUGINS_DIR: &str = "plugins";

/// `<home>/tasks/<name>/brief.md`, absolute.
pub fn brief_file(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(TASKS_DIR).join(name).join(BRIEF_FILE);
    std::path::absolute(&path).unwrap_or(path)
}

pub fn mcp_file(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(TASKS_DIR).join(name).join(MCP_FILE);
    std::path::absolute(&path).unwrap_or(path)
}

pub fn settings_file(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(TASKS_DIR).join(name).join(SETTINGS_FILE);
    std::path::absolute(&path).unwrap_or(path)
}

pub fn plugins_dir(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(TASKS_DIR).join(name).join(PLUGINS_DIR);
    std::path::absolute(&path).unwrap_or(path)
}

pub fn token_file(home: &Home, name: &str, plugin: &str) -> PathBuf {
    let safe: String = plugin
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let file = format!("plugin-{safe}.token");
    let path = home.root().join(TASKS_DIR).join(name).join(file);
    std::path::absolute(&path).unwrap_or(path)
}

pub(super) async fn write_token(
    home: &Home,
    name: &str,
    plugin: &str,
    token: String,
) -> Result<PathBuf> {
    write_private_file(home, token_file(home, name, plugin), token).await
}

pub(super) async fn write_mcp(home: &Home, name: &str, config: String) -> Result<PathBuf> {
    write_private_file(home, mcp_file(home, name), config).await
}

pub(super) async fn write_settings(home: &Home, name: &str, settings: String) -> Result<PathBuf> {
    write_private_file(home, settings_file(home, name), settings).await
}

async fn write_private_file(home: &Home, path: PathBuf, config: String) -> Result<PathBuf> {
    let (home, target) = (home.clone(), path.clone());
    tokio::task::spawn_blocking(move || {
        if let Some(dir) = target.parent() {
            home.create_dir(dir)?;
        }
        write_private(&target, config.as_bytes())
    })
    .await
    .map_err(|e| {
        Error::io(
            "write a task mcp config",
            std::io::Error::other(e.to_string()),
        )
    })??;
    Ok(path)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{file}.{}.tmp", std::process::id()));
    let written = (|| {
        let _ = std::fs::remove_file(&tmp);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    written.map_err(|e| Error::io(format!("write {}", path.display()), e))
}

/// The one-line prompt the agent starts with.
pub fn order(brief: &Path) -> String {
    format!(
        "read your task brief at {} and do it. it is the full task.",
        brief.display()
    )
}

pub(super) async fn plugin_sections(ctx: &Context, task: &Task) -> Result<Vec<(String, String)>> {
    let store = ctx.store()?;
    match store.agent_type(&task.agent).await? {
        Some(found) => declared::brief_sections(store, &found.def.plugins).await,
        None => Ok(Vec::new()),
    }
}

/// Writes (or rewrites) the task's brief with its hand backs and attachments' paths; returns
/// its path.
pub async fn write(
    home: &Home,
    task: &Task,
    files: &[PathBuf],
    handoffs: &[Handoff],
    sections: &[(String, String)],
) -> Result<PathBuf> {
    let path = brief_file(home, &task.name);
    let text = plan::with_plugin_sections(
        plan::brief_text(&task.title, &task.prompt, &task.done, files, handoffs),
        sections,
    );
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
