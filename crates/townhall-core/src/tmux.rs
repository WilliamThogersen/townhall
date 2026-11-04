use std::path::Path;

use crate::cmd::{run, validate_name};
use crate::Result;

const SESSION_PREFIX: &str = "townhall-";

/// Prefix shared by every session of one repo: `townhall-<repo-key>-`.
fn repo_prefix(key: &str) -> String {
    format!("{SESSION_PREFIX}{key}-")
}

/// Full session name: `townhall-<repo-key>-<name>`.
pub fn session_name(key: &str, name: &str) -> String {
    format!("{}{name}", repo_prefix(key))
}

/// Start a detached tmux session running `command` in `cwd`.
/// tmux owns the process, so it survives the townhall daemon exiting.
pub async fn spawn(key: &str, name: &str, cwd: &Path, command: &str) -> Result<()> {
    validate_name(name)?;
    let cwd = cwd.to_string_lossy().into_owned();
    run(
        "tmux",
        &[
            "new-session",
            "-d",
            "-s",
            &session_name(key, name),
            "-c",
            &cwd,
            command,
        ],
        None,
    )
    .await?;
    Ok(())
}

/// Names of live sessions of the repo `key`, without the prefix.
pub async fn list(key: &str) -> Result<Vec<String>> {
    let prefix = repo_prefix(key);
    match run("tmux", &["list-sessions", "-F", "#{session_name}"], None).await {
        Ok(out) => Ok(out
            .lines()
            .filter_map(|l| l.strip_prefix(prefix.as_str()))
            .map(str::to_string)
            .collect()),
        // No tmux server running means no sessions.
        Err(crate::Error::Command { stderr, .. })
            if stderr.contains("no server running") || stderr.contains("error connecting to") =>
        {
            Ok(vec![])
        }
        Err(e) => Err(e),
    }
}

/// Type `text` into the session and press Enter.
pub async fn send(key: &str, name: &str, text: &str) -> Result<()> {
    validate_name(name)?;
    run(
        "tmux",
        &["send-keys", "-t", &session_name(key, name), text, "Enter"],
        None,
    )
    .await?;
    Ok(())
}

/// Snapshot of the visible pane plus scrollback.
pub async fn capture(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    run(
        "tmux",
        &[
            "capture-pane",
            "-p",
            "-S",
            "-",
            "-t",
            &session_name(key, name),
        ],
        None,
    )
    .await
}

pub async fn kill(key: &str, name: &str) -> Result<()> {
    validate_name(name)?;
    run(
        "tmux",
        &["kill-session", "-t", &session_name(key, name)],
        None,
    )
    .await?;
    Ok(())
}
