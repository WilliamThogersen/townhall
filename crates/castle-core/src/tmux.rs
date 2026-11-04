use std::path::Path;

use crate::cmd::{run, validate_name};
use crate::Result;

const SESSION_PREFIX: &str = "castle-";

fn session_name(name: &str) -> String {
    format!("{SESSION_PREFIX}{name}")
}

/// Start a detached tmux session running `command` in `cwd`.
/// tmux owns the process, so it survives the castle daemon exiting.
pub async fn spawn(name: &str, cwd: &Path, command: &str) -> Result<()> {
    validate_name(name)?;
    let cwd = cwd.to_string_lossy().into_owned();
    run(
        "tmux",
        &["new-session", "-d", "-s", &session_name(name), "-c", &cwd, command],
        None,
    )
    .await?;
    Ok(())
}

/// Names of live castle sessions, without the prefix.
pub async fn list() -> Result<Vec<String>> {
    match run("tmux", &["list-sessions", "-F", "#{session_name}"], None).await {
        Ok(out) => Ok(out
            .lines()
            .filter_map(|l| l.strip_prefix(SESSION_PREFIX))
            .map(str::to_string)
            .collect()),
        // No tmux server running means no sessions.
        Err(crate::Error::Command { stderr, .. }) if stderr.contains("no server running")
            || stderr.contains("error connecting to") => {
            Ok(vec![])
        }
        Err(e) => Err(e),
    }
}

/// Type `text` into the session and press Enter.
pub async fn send(name: &str, text: &str) -> Result<()> {
    validate_name(name)?;
    run("tmux", &["send-keys", "-t", &session_name(name), text, "Enter"], None).await?;
    Ok(())
}

/// Snapshot of the visible pane plus scrollback.
pub async fn capture(name: &str) -> Result<String> {
    validate_name(name)?;
    run("tmux", &["capture-pane", "-p", "-S", "-", "-t", &session_name(name)], None).await
}

pub async fn kill(name: &str) -> Result<()> {
    validate_name(name)?;
    run("tmux", &["kill-session", "-t", &session_name(name)], None).await?;
    Ok(())
}
