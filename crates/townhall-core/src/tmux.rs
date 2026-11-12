//! tmux sessions, one per agent. Sessions are created with `remain-on-exit`, so a pane
//! whose process exited stays (dead) until it is killed and tells how it exited.
//! Targets use tmux's exact form (`=name`), so `a` never matches a session `ab`.
//! Every call goes to the server `TOWNHALL_TMUX_SOCKET` names (`tmux -L <name>`), else to
//! the default one.

mod panes;

use std::ffi::OsString;
use std::path::Path;

pub use panes::{Exit, Pane};

use crate::cmd::{probe, run, validate_name};
use crate::model::AgentKey;
use crate::{Error, Result};

const SESSION_PREFIX: &str = "townhall-";

/// Names a private tmux server (`tmux -L <name>`) for every tmux call. Unset or empty
/// means the default server. Tests set it so they never share the server of real agents.
pub const SOCKET_ENV: &str = "TOWNHALL_TMUX_SOCKET";

fn socket_args(value: Option<OsString>) -> Result<Vec<String>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(vec![]);
    };
    let plain = |n: &&str| {
        n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    match value.to_str().filter(plain) {
        Some(name) => Ok(vec!["-L".into(), name.into()]),
        None => Err(Error::InvalidInput(format!(
            "{SOCKET_ENV} {value:?} may hold only letters, digits, `.`, `_` and `-`"
        ))),
    }
}

/// `-L <name>` when [`SOCKET_ENV`] names a server, else nothing.
pub fn server_args() -> Result<Vec<String>> {
    socket_args(std::env::var_os(SOCKET_ENV))
}

/// Runs `tmux args...` on townhall's server.
async fn run_tmux(args: &[&str]) -> Result<String> {
    let server = server_args()?;
    let all: Vec<&str> = server
        .iter()
        .map(String::as_str)
        .chain(args.iter().copied())
        .collect();
    run("tmux", &all, None).await
}

async fn probe_tmux(args: &[&str]) -> Result<String> {
    let server = server_args()?;
    let all: Vec<&str> = server
        .iter()
        .map(String::as_str)
        .chain(args.iter().copied())
        .collect();
    probe("tmux", &all, None).await
}

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

/// The session option that remembers which adapter started the agent.
const KIND_OPTION: &str = "@townhall_kind";

/// The pane gets its own `PWD`, so it never inherits the stale one of whoever started
/// the tmux server. The session keeps `kind` in [`KIND_OPTION`].
fn new_session(key: &str, name: &str, cwd: &str, command: &str, kind: &str) -> Vec<String> {
    let session = session_name(key, name);
    [
        "new-session",
        "-d",
        "-s",
        &session,
        "-c",
        cwd,
        "-e",
        &format!("PWD={cwd}"),
        command,
        ";",
        "set-option",
        "-w",
        "-t",
        &pane_target(key, name),
        "remain-on-exit",
        "on",
        ";",
        "set-option",
        "-t",
        &pane_target(key, name),
        KIND_OPTION,
        kind,
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
        match run_tmux(&args).await {
            Err(e) if server_was_exiting(&e) && attempt < SPAWN_ATTEMPTS => {
                attempt += 1;
                tokio::time::sleep(SPAWN_PAUSE).await;
            }
            other => return other,
        }
    }
}

/// Start a detached tmux session running `command` in `cwd`, started by the adapter `kind`.
/// tmux owns the process, so it survives the townhall daemon exiting. A dead session
/// left by an earlier agent of the same name is replaced; a live one is a conflict.
pub async fn spawn(key: &str, name: &str, cwd: &Path, command: &str, kind: &str) -> Result<()> {
    validate_name(name)?;
    let args = new_session(key, name, &cwd.to_string_lossy(), command, kind);
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
    Ok(run_tmux(&args).await?.trim() == "1")
}

/// A dead pane answers like a missing session.
fn gone(name: &str) -> Error {
    Error::NotFound(name.to_string())
}

/// `text` as one argument tmux passes on unchanged. tmux reads an argument that ends in
/// `;` as the end of its command and turns a final `\;` into `;`, so a final `;` goes
/// as `\;`.
fn literal(text: &str) -> String {
    match text.strip_suffix(';') {
        Some(head) => format!("{head}\\;"),
        None => text.to_string(),
    }
}

/// `send-keys` that types `text` as it is (`-l`, never key names like `Down` or `C-c`),
/// then a second one that presses Enter.
fn type_then_enter(target: &str, text: &str) -> Vec<String> {
    let typed = literal(text);
    let args = ["send-keys", "-t", target, "-l", "--", &typed, ";"];
    let enter = ["send-keys", "-t", target, "Enter"];
    args.into_iter().chain(enter).map(String::from).collect()
}

/// The session's pane target, once its pane is known to be live.
async fn live_target(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    if dead(key, name).await? {
        return Err(gone(name));
    }
    Ok(pane_target(key, name))
}

/// Type `text` into the session as it is, then press Enter. A dead pane counts as a
/// missing session.
pub async fn send(key: &str, name: &str, text: &str) -> Result<()> {
    let args = type_then_enter(&live_target(key, name).await?, text);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_tmux(&args).await?;
    Ok(())
}

/// tmux's name for a key townhall presses.
fn key_name(pressed: AgentKey) -> &'static str {
    match pressed {
        AgentKey::Up => "Up",
        AgentKey::Down => "Down",
        AgentKey::Enter => "Enter",
        AgentKey::Escape => "Escape",
        AgentKey::Tab => "Tab",
    }
}

/// Press one key in the session, nothing else. A dead pane counts as a missing session.
pub async fn send_key(key: &str, name: &str, pressed: AgentKey) -> Result<()> {
    let target = live_target(key, name).await?;
    run_tmux(&["send-keys", "-t", &target, key_name(pressed)]).await?;
    Ok(())
}

/// The viewer options of an attachable session: the attached client drives the window
/// size (`window-size latest`), the status bar is off (the pane has its own header) and
/// the mouse is on, so the wheel scrolls tmux's history (tmux uses the alternate screen,
/// so the browser terminal has no scrollback of its own).
fn viewer_options(target: &str) -> Vec<String> {
    let settings: [&[&str]; 3] = [
        &["-t", target, "status", "off"],
        &["-t", target, "mouse", "on"],
        &["-w", "-t", target, "window-size", "latest"],
    ];
    let mut args = Vec::new();
    for setting in settings {
        if !args.is_empty() {
            args.push(";".to_string());
        }
        args.push("set-option".to_string());
        args.extend(setting.iter().map(|a| a.to_string()));
    }
    args
}

/// The arguments of the `tmux` process that attaches a client to the session: the
/// server flags, then `attach-session` on the session's exact target.
fn attach_args(key: &str, name: &str) -> Result<Vec<String>> {
    let mut args = server_args()?;
    args.extend([
        "attach-session".into(),
        "-t".into(),
        session_target(key, name),
    ]);
    Ok(args)
}

/// Readies a live session for a viewer (see [`viewer_options`]) and returns the
/// arguments of the `tmux` process that attaches to it. A dead or missing session is an
/// error, so a caller can refuse before it opens a terminal.
pub async fn prepare_attach(key: &str, name: &str) -> Result<Vec<String>> {
    let target = live_target(key, name).await?;
    let options = viewer_options(&target);
    let options: Vec<&str> = options.iter().map(String::as_str).collect();
    run_tmux(&options).await?;
    attach_args(key, name)
}

/// Snapshot of the visible pane plus scrollback, dead or alive.
pub async fn capture(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    run_tmux(&[
        "capture-pane",
        "-p",
        "-S",
        "-",
        "-t",
        &pane_target(key, name),
    ])
    .await
}

/// Like `capture`, but a dead pane counts as a missing session. One tmux call.
pub async fn capture_live(key: &str, name: &str) -> Result<String> {
    validate_name(name)?;
    let target = pane_target(key, name);
    let out = run_tmux(&[
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
    ])
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
    run_tmux(&["capture-pane", "-p", "-J", "-t", &pane_target(key, name)]).await
}

pub async fn kill(key: &str, name: &str) -> Result<()> {
    validate_name(name)?;
    let killed = probe_tmux(&["kill-session", "-t", &session_target(key, name)]).await;
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

    #[test]
    fn attach_targets_the_exact_session_on_the_private_server() {
        let args = attach_args("k", "a").unwrap();
        assert_eq!(
            &args[args.len() - 3..],
            ["attach-session", "-t", "=townhall-k-a"]
        );
    }

    #[test]
    fn viewer_options_turn_the_status_off_and_follow_the_latest_client() {
        let args = viewer_options("=s:").join(" ");
        assert!(args.contains("status off"), "{args}");
        assert!(args.contains("mouse on"), "{args}");
        assert!(args.contains("-w -t =s: window-size latest"), "{args}");
    }

    #[test]
    fn a_socket_name_picks_a_private_server() {
        let args = |v: Option<&str>| socket_args(v.map(OsString::from));
        assert!(args(None).unwrap().is_empty());
        assert!(args(Some("")).unwrap().is_empty());
        assert_eq!(
            args(Some("townhall-test-1.a_b")).unwrap(),
            ["-L", "townhall-test-1.a_b"]
        );
        for bad in ["a/b", "a b", "a\nb", "../x"] {
            let error = args(Some(bad)).unwrap_err();
            assert!(error.to_string().contains(SOCKET_ENV), "{error}");
        }
    }

    #[test]
    fn text_is_typed_literally_and_a_final_semicolon_survives_tmux() {
        assert_eq!(
            type_then_enter("=s:", "Down"),
            [
                "send-keys",
                "-t",
                "=s:",
                "-l",
                "--",
                "Down",
                ";",
                "send-keys",
                "-t",
                "=s:",
                "Enter"
            ]
        );
        assert_eq!(literal("fix it;"), "fix it\\;");
        assert_eq!(literal(";"), "\\;");
        assert_eq!(literal("a\\;"), "a\\\\;");
        assert_eq!(literal("semi; colon"), "semi; colon");
        assert_eq!(literal(""), "");
    }

    #[test]
    fn a_new_session_gets_its_own_pwd() {
        let args = new_session("k", "a", "/w/a", "run it", "shell");
        let at = args.iter().position(|a| a == "-e").unwrap();
        assert_eq!(args[at + 1], "PWD=/w/a");
        let command = args.iter().position(|a| a == "run it").unwrap();
        assert!(at < command, "{args:?}");
    }
}
