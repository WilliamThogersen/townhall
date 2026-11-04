//! tmux sessions, one per agent. Sessions are created with `remain-on-exit`, so a pane
//! whose process exited stays (dead) until it is killed and tells how it exited.
//! Targets use tmux's exact form (`=name`), so `a` never matches a session `ab`.

mod panes;

use std::path::Path;

pub use panes::{Exit, Pane};

use crate::cmd::{run, validate_name};
use crate::{Error, Result};

const SESSION_PREFIX: &str = "townhall-";

/// Prefix shared by every session of one repo: `townhall-<repo-key>-`.
fn repo_prefix(key: &str) -> String {
    format!("{SESSION_PREFIX}{key}-")
}

/// Full session name: `townhall-<repo-key>-<name>`.
pub fn session_name(key: &str, name: &str) -> String {
    format!("{}{name}", repo_prefix(key))
}

/// The exact target of the session itself (`kill-session`).
fn session_target(key: &str, name: &str) -> String {
    format!("={}", session_name(key, name))
}

/// The exact target of the session's current pane (`send-keys`, `capture-pane`).
fn pane_target(key: &str, name: &str) -> String {
    format!("={}:", session_name(key, name))
}

fn new_session(key: &str, name: &str, cwd: &str, command: &str) -> Vec<String> {
    let session = session_name(key, name);
    [
        "new-session",
        "-d",
        "-s",
        &session,
        "-c",
        cwd,
        command,
        ";",
        "set-option",
        "-w",
        "-t",
        &pane_target(key, name),
        "remain-on-exit",
        "on",
    ]
    .map(String::from)
    .to_vec()
}

fn duplicate(e: &Error) -> bool {
    matches!(e, Error::Command { stderr, .. } if stderr.contains("duplicate session"))
}

const SPAWN_ATTEMPTS: u32 = 4;
const SPAWN_PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// tmux's server exits when its last session ends; a `new-session` that reaches it just
/// then fails with this, and the next try starts a fresh server.
fn server_was_exiting(e: &crate::Error) -> bool {
    matches!(e, crate::Error::Command { stderr, .. } if stderr.contains("server exited unexpectedly"))
}

/// Runs `new-session`, trying again while the tmux server was just exiting.
async fn run_new_session(args: &[String]) -> Result<String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut attempt = 1;
    loop {
        match run("tmux", &args, None).await {
            Err(e) if server_was_exiting(&e) && attempt < SPAWN_ATTEMPTS => {
                attempt += 1;
                tokio::time::sleep(SPAWN_PAUSE).await;
            }
            other => return other,
        }
    }
}

/// Start a detached tmux session running `command` in `cwd`.
/// tmux owns the process, so it survives the townhall daemon exiting. A dead session
/// left by an earlier agent of the same name is replaced; a live one is a conflict.
pub async fn spawn(key: &str, name: &str, cwd: &Path, command: &str) -> Result<()> {
    validate_name(name)?;
    let args = new_session(key, name, &cwd.to_string_lossy(), command);
    let first = run_new_session(&args).await;
    let result = match first {
        Err(e) if duplicate(&e) && dead_leftover(key, name).await? => {
            kill(key, name).await?;
            run_new_session(&args).await
        }
        other => other,
    };
    panes::invalidate();
    result.map(|_| ())
}

/// A session of that name exists and its process has exited.
async fn dead_leftover(key: &str, name: &str) -> Result<bool> {
    let session = session_name(key, name);
    Ok(panes::fetch()
        .await?
        .iter()
        .any(|p| p.session == session && p.dead()))
}

/// Every session of the repo `key` with its first pane, the prefix kept in `session`.
pub async fn panes(key: &str) -> Result<Vec<Pane>> {
    let prefix = repo_prefix(key);
    Ok(panes::fetch()
        .await?
        .iter()
        .filter(|p| p.session.starts_with(&prefix))
        .cloned()
        .collect())
}

/// Names of live sessions of the repo `key`, without the prefix. A session whose process
/// exited is not live.
pub async fn list(key: &str) -> Result<Vec<String>> {
    let prefix = repo_prefix(key);
    Ok(panes(key)
        .await?
        .into_iter()
        .filter(|p| !p.dead())
        .filter_map(|p| p.session.strip_prefix(&prefix).map(str::to_string))
        .collect())
}

/// The session's pane from a snapshot at most a second old, asking tmux again when the
/// snapshot does not have it. `None` when there is no such session.
pub async fn pane(key: &str, name: &str) -> Result<Option<Pane>> {
    let session = session_name(key, name);
    let find = |all: &[Pane]| all.iter().find(|p| p.session == session).cloned();
    if let Some(found) = find(&panes::cached().await?) {
        return Ok(Some(found));
    }
    Ok(find(&panes::fetch().await?))
}

/// Whether the session's pane is dead. A missing session is not dead: the command that
/// follows reports it.
async fn dead(key: &str, name: &str) -> Result<bool> {
    let args = [
        "display-message",
        "-p",
        "-t",
        &pane_target(key, name),
        "#{pane_dead}",
    ];
    Ok(run("tmux", &args, None).await?.trim() == "1")
}

/// A dead pane answers like a missing session.
fn gone(name: &str) -> Error {
    Error::NotFound(name.to_string())
}

/// Type `text` into the session and press Enter. A dead pane counts as a missing session.
pub async fn send(key: &str, name: &str, text: &str) -> Result<()> {
    validate_name(name)?;
    if dead(key, name).await? {
        return Err(gone(name));
    }
    run(
        "tmux",
        &["send-keys", "-t", &pane_target(key, name), text, "Enter"],
        None,
    )
    .await?;
    Ok(())
}

/// Snapshot of the visible pane plus scrollback, dead or alive.
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
            &pane_target(key, name),
        ],
        None,
    )
    .await
}

/// Like `capture`, but a dead pane counts as a missing session. One tmux call.
pub async fn capture_live(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    let target = pane_target(key, name);
    let out = run(
        "tmux",
        &[
            "display-message",
            "-p",
            "-t",
            &target,
            "#{pane_dead}",
            ";",
            "capture-pane",
            "-p",
            "-S",
            "-",
            "-t",
            &target,
        ],
        None,
    )
    .await?;
    match out.split_once('\n') {
        Some(("1", _)) => Err(gone(name)),
        Some((_, pane)) => Ok(pane.trim().to_string()),
        None if out == "1" => Err(gone(name)),
        None => Ok(String::new()),
    }
}

/// The visible screen only, wrapped lines joined: what an agent shows right now.
pub async fn screen(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    run(
        "tmux",
        &["capture-pane", "-p", "-J", "-t", &pane_target(key, name)],
        None,
    )
    .await
}

pub async fn kill(key: &str, name: &str) -> Result<()> {
    validate_name(name)?;
    let killed = run(
        "tmux",
        &["kill-session", "-t", &session_target(key, name)],
        None,
    )
    .await;
    panes::invalidate();
    killed.map(|_| ())
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
