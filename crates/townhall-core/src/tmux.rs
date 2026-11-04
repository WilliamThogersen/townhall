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

const SPAWN_ATTEMPTS: u32 = 4;
const SPAWN_PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// tmux's server exits when its last session ends; a `new-session` that reaches it just
/// then fails with this, and the next try starts a fresh server.
fn server_was_exiting(e: &crate::Error) -> bool {
    matches!(e, crate::Error::Command { stderr, .. } if stderr.contains("server exited unexpectedly"))
}

/// Start a detached tmux session running `command` in `cwd`.
/// tmux owns the process, so it survives the townhall daemon exiting.
pub async fn spawn(key: &str, name: &str, cwd: &Path, command: &str) -> Result<()> {
    validate_name(name)?;
    let cwd = cwd.to_string_lossy().into_owned();
    let session = session_name(key, name);
    let args = ["new-session", "-d", "-s", &session, "-c", &cwd, command];
    let mut attempt = 1;
    loop {
        match run("tmux", &args, None).await {
            Ok(_) => return Ok(()),
            Err(e) if server_was_exiting(&e) && attempt < SPAWN_ATTEMPTS => {
                attempt += 1;
                tokio::time::sleep(SPAWN_PAUSE).await;
            }
            Err(e) => return Err(e),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exiting_server_race_is_retried() {
        let failed = |stderr: &str| crate::Error::Command {
            command: "tmux new-session".into(),
            code: Some(1),
            stderr: stderr.into(),
        };
        assert!(server_was_exiting(&failed("server exited unexpectedly")));
        assert!(!server_was_exiting(&failed("duplicate session: x")));
        assert!(!server_was_exiting(&crate::Error::InvalidInput("x".into())));
    }
}
