//! Agent steps for the control center: the tool calls in Claude's transcript.

mod locate;
mod parse;
mod summary;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use locate::{latest_transcript, project_dir_for};
pub use parse::parse_transcript;

use crate::{Error, Result};

/// Where a step stands: no result yet, finished, or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepResult {
    Running,
    Ok,
    Error,
}

/// One tool call by the agent, paired with its result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// The `tool_use` id Claude gave the call.
    pub id: String,
    pub tool: String,
    pub summary: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub result: StepResult,
}

/// Steps from the newest transcript Claude wrote in `worktree`; empty when there is none.
pub async fn steps_for_worktree(claude_projects: &Path, worktree: &Path) -> Result<Vec<Step>> {
    let projects = claude_projects.to_path_buf();
    let cwd = worktree.to_path_buf();
    tokio::task::spawn_blocking(move || read_steps(&projects, &cwd))
        .await
        .map_err(|e| Error::Io {
            context: format!("read steps for {}", worktree.display()),
            source: std::io::Error::other(e),
        })?
}

fn read_steps(projects: &Path, cwd: &Path) -> Result<Vec<Step>> {
    let Some(path) = latest_transcript(projects, cwd)? else {
        return Ok(vec![]);
    };
    match std::fs::read(&path) {
        Ok(bytes) => Ok(parse_transcript(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(source) => Err(Error::Io {
            context: format!("read transcript {}", path.display()),
            source,
        }),
    }
}
