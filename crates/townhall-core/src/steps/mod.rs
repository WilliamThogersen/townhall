//! Agent steps for the control center: the tool calls in Claude's transcript.

mod agent;
mod locate;
mod parse;
mod summary;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

pub use agent::{claude_projects_dir, worktree_made, CLAUDE_PROJECTS_ENV};
pub use locate::{latest_transcript, latest_transcript_since, project_dir_for};
pub use parse::{parse_transcript, transcript_tokens};

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
    blocking(claude_projects, worktree, |projects, cwd| {
        read_steps(&projects, &cwd, None)
    })
    .await
}

/// Steps of the agent working in `worktree`: like [`steps_for_worktree`], but a transcript
/// last written before the worktree was made ([`worktree_made`]) is an earlier agent's with
/// the same name, so it does not count. Empty when the worktree folder is gone.
pub async fn steps_for_agent(claude_projects: &Path, worktree: &Path) -> Result<Vec<Step>> {
    blocking(
        claude_projects,
        worktree,
        |projects, cwd| match worktree_made(&cwd)? {
            Some(made) => read_steps(&projects, &cwd, Some(made)),
            None => Ok(vec![]),
        },
    )
    .await
}

pub async fn tokens_for_agent(claude_projects: &Path, worktree: &Path) -> Result<u64> {
    let (projects, cwd) = (claude_projects.to_path_buf(), worktree.to_path_buf());
    tokio::task::spawn_blocking(move || read_tokens(&projects, &cwd))
        .await
        .map_err(|e| Error::Io {
            context: format!("read tokens for {}", worktree.display()),
            source: std::io::Error::other(e),
        })?
}

fn read_tokens(projects: &Path, cwd: &Path) -> Result<u64> {
    let Some(made) = worktree_made(cwd)? else {
        return Ok(0);
    };
    let Some(path) = latest_transcript_since(projects, cwd, made)? else {
        return Ok(0);
    };
    match std::fs::read(&path) {
        Ok(bytes) => Ok(transcript_tokens(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(Error::Io {
            context: format!("read transcript {}", path.display()),
            source,
        }),
    }
}

async fn blocking<F>(claude_projects: &Path, worktree: &Path, read: F) -> Result<Vec<Step>>
where
    F: FnOnce(PathBuf, PathBuf) -> Result<Vec<Step>> + Send + 'static,
{
    let (projects, cwd) = (claude_projects.to_path_buf(), worktree.to_path_buf());
    tokio::task::spawn_blocking(move || read(projects, cwd))
        .await
        .map_err(|e| Error::Io {
            context: format!("read steps for {}", worktree.display()),
            source: std::io::Error::other(e),
        })?
}

fn read_steps(projects: &Path, cwd: &Path, since: Option<SystemTime>) -> Result<Vec<Step>> {
    let newest = match since {
        Some(cutoff) => latest_transcript_since(projects, cwd, cutoff)?,
        None => latest_transcript(projects, cwd)?,
    };
    let Some(path) = newest else {
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
