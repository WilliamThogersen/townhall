//! Agent state detection, from tmux and Claude's transcript:
//! - no session: `ended`; a dead pane (sessions keep it with `remain-on-exit`): `ended`
//!   when the process exited 0, else `error` with the exit code,
//! - a live pane is `waiting` when the agent asks something. It asks when Claude's
//!   transcript (written since the session started) has a pending `AskUserQuestion` or a
//!   turn that ended on a question, when the screen shows a selection prompt (a permission
//!   request only ever shows there, even while the transcript reads busy: Claude writes a
//!   message with parallel tool calls only once every call has a result), or, for a program
//!   without a transcript, when its last line is a question. Claude's folder trust dialog
//!   in a new worktree asks too, as [`question::TRUST_QUESTION`]; the watcher answers it
//!   once for townhall's own worktrees (`watch/trust.rs`),
//! - a live Claude whose turn ended without asking, [`IDLE_SETTLE_MS`] ago or longer, is
//!   `ended`: interactive Claude never exits on its own, and a short pause between tool
//!   calls must not count. Typing into it starts a new turn, which is `working` again,
//! - anything else live is `working`.

mod memo;
mod question;
mod screen;
mod stamp;
mod transcript;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::clock::unix_millis;

pub use question::choices;
pub(crate) use question::TRUST_QUESTION;

use super::Agent;
use crate::model::AgentState;
use crate::tmux::{self, Exit, Pane};
use crate::{steps, Workspace};
use transcript::Turn;

/// How long a turn must have ended, without a question, before the agent counts as ended.
pub const IDLE_SETTLE_MS: i64 = 10_000;

pub const QUIET_SECS: u64 = 300;

/// Overrides where Claude's transcripts are read from (tests point it at a temp folder);
/// the same variable 2.7's steps read.
pub const PROJECTS_ENV: &str = steps::CLAUDE_PROJECTS_ENV;

/// Claude's projects folder, found as steps find it (`$TOWNHALL_CLAUDE_PROJECTS`, else
/// `~/.claude/projects`). `None` when neither is set: then only the screen is read.
pub fn claude_projects() -> Option<PathBuf> {
    steps::claude_projects_dir().ok()
}

/// The agent's state, reading transcripts from [`claude_projects`].
pub async fn detect(ws: &Workspace, agent: &Agent) -> AgentState {
    detect_in(claude_projects().as_deref(), ws, agent).await
}

/// The agent's state, reading transcripts from `projects` (none: screen only). When tmux
/// cannot be asked, the baseline answers: alive is working, dead is ended.
pub async fn detect_in(projects: Option<&Path>, ws: &Workspace, agent: &Agent) -> AgentState {
    match tmux::pane(ws.key(), &agent.name).await {
        Ok(None) => AgentState::Ended,
        Ok(Some(pane)) => match &pane.exit {
            Some(exit) => exited(exit),
            None => live(projects, ws, agent, &pane).await,
        },
        Err(e) => {
            warn_once(&format!("agent states fall back to alive or dead: {e}"));
            if agent.alive {
                AgentState::Working
            } else {
                AgentState::Ended
            }
        }
    }
}

/// tmux's signal names, numbered as on both Linux and macOS.
const SIGNALS: [(&str, i32); 12] = [
    ("hup", 1),
    ("int", 2),
    ("quit", 3),
    ("ill", 4),
    ("trap", 5),
    ("abrt", 6),
    ("fpe", 8),
    ("kill", 9),
    ("segv", 11),
    ("pipe", 13),
    ("alrm", 14),
    ("term", 15),
];

/// A signal as tmux prints it: a name in any case, with or without `SIG`, or a bare number
/// (what tmux prints on Linux).
fn signal_number(text: &str) -> Option<i32> {
    let text = text.trim();
    if let Ok(number) = text.parse::<i32>() {
        return (number > 0).then_some(number);
    }
    let bare = match text.get(..3) {
        Some(prefix) if prefix.eq_ignore_ascii_case("sig") => &text[3..],
        _ => text,
    };
    SIGNALS
        .iter()
        .find(|(n, _)| bare.eq_ignore_ascii_case(n))
        .map(|(_, number)| *number)
}

/// A dead pane's state: exit 0 ended, anything else an error. A signal is reported the
/// way a shell does, 128 plus its number.
fn exited(exit: &Exit) -> AgentState {
    match exit {
        Exit::Code(0) => AgentState::Ended,
        Exit::Code(code) => AgentState::Error {
            exit_code: Some(*code),
        },
        Exit::Signal(name) => AgentState::Error {
            exit_code: signal_number(name).map(|number| 128 + number),
        },
        Exit::Unknown => AgentState::Error { exit_code: None },
    }
}

fn waiting(question: String) -> AgentState {
    AgentState::Waiting { question }
}

async fn live(projects: Option<&Path>, ws: &Workspace, agent: &Agent, pane: &Pane) -> AgentState {
    let turn = match projects {
        Some(dir) => current_turn(dir, &agent.worktree, &agent.name, pane.created).await,
        None => None,
    };
    let busy = busy_state(unix_secs(SystemTime::now()), pane.activity);
    let asked = |q: Option<String>| q.map_or(AgentState::Working, waiting);
    let stalled = |q: Option<String>| q.map_or(busy.clone(), waiting);
    match turn {
        Some(Turn::Asking { question }) => waiting(question),
        Some(Turn::Busy) => stalled(on_screen(ws, agent, pane, false).await),
        Some(Turn::Pending { call }) => stalled(
            on_screen(ws, agent, pane, false)
                .await
                .map(|q| question::shorten(&format!("{call}\n{q}"))),
        ),
        Some(Turn::Idle { text, at }) => idle(ws, agent, pane, &text, at).await,
        None => asked(on_screen(ws, agent, pane, true).await),
    }
}

fn busy_state(now: u64, activity: u64) -> AgentState {
    match now.saturating_sub(activity) >= QUIET_SECS {
        true => AgentState::Ended,
        false => AgentState::Working,
    }
}

/// A finished turn: waiting when it asked something (in its text or a dialog on screen),
/// ended once it has been quiet for [`IDLE_SETTLE_MS`], working until then.
async fn idle(
    ws: &Workspace,
    agent: &Agent,
    pane: &Pane,
    text: &str,
    at: Option<i64>,
) -> AgentState {
    if let Some(question) = question::closing(text) {
        return waiting(question);
    }
    if let Some(question) = on_screen(ws, agent, pane, false).await {
        return waiting(question);
    }
    let now = unix_millis(SystemTime::now());
    match at {
        Some(at) if now - at >= IDLE_SETTLE_MS => AgentState::Ended,
        _ => AgentState::Working,
    }
}

/// A prompt on screen (any agent) or, when `plain`, a last line asking something.
/// Claude's folder trust dialog is asked as [`question::TRUST_QUESTION`].
async fn on_screen(ws: &Workspace, agent: &Agent, pane: &Pane, plain: bool) -> Option<String> {
    let text = screen_text(ws, agent, pane).await?;
    if screen::trust_dialog(&text) {
        return Some(question::TRUST_QUESTION.to_string());
    }
    let plain_question = || plain.then(|| screen::trailing_question(&text)).flatten();
    screen::prompt(&text).or_else(plain_question)
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The visible screen, captured again only when the pane printed since the last capture.
async fn screen_text(ws: &Workspace, agent: &Agent, pane: &Pane) -> Option<String> {
    if let Some(text) = memo::screen(&pane.id, pane.activity) {
        return Some(text);
    }
    let started = unix_secs(SystemTime::now());
    match tmux::screen(ws.key(), &agent.name).await {
        Ok(text) => {
            memo::keep_screen(&pane.id, pane.activity, started, &text);
            Some(text)
        }
        Err(e) => {
            warn_once(&format!("cannot read the screen of `{}`: {e}", agent.name));
            None
        }
    }
}

/// The turn in the newest transcript for `worktree` written since `since` (unix seconds,
/// the session's start), so an earlier session's transcript is never read as this one's.
async fn current_turn(projects: &Path, worktree: &Path, agent: &str, since: u64) -> Option<Turn> {
    let (projects, worktree) = (projects.to_path_buf(), worktree.to_path_buf());
    let agent = agent.to_string();
    let read =
        tokio::task::spawn_blocking(move || read_turn(&projects, &worktree, &agent, since)).await;
    match read {
        Ok(Ok(turn)) => turn,
        Ok(Err(e)) => {
            warn_once(&format!("cannot read a transcript: {e}"));
            None
        }
        Err(e) => {
            warn_once(&format!("transcript read failed: {e}"));
            None
        }
    }
}

fn read_turn(
    projects: &Path,
    worktree: &Path,
    agent: &str,
    since: u64,
) -> crate::Result<Option<Turn>> {
    let Some(path) = steps::latest_transcript(projects, worktree)? else {
        return Ok(None);
    };
    let io = |source| crate::Error::Io {
        context: format!("read transcript {}", path.display()),
        source,
    };
    let meta = std::fs::metadata(&path).map_err(io)?;
    let modified = meta.modified().map_err(io)?;
    if unix_secs(modified) < since {
        return Ok(None);
    }
    let stamp = (meta.len(), modified);
    if let Some(turn) = memo::turn(&path, stamp) {
        return Ok(turn);
    }
    let turn = transcript::read_as(&path, agent)
        .map_err(io)?
        .map(|t| ended_at(t, modified));
    memo::keep_turn(&path, stamp, turn.clone());
    Ok(turn)
}

/// What the agent in `worktree` said when its last turn ended, from its newest transcript
/// written since the worktree was made (any, once the folder is gone), read like its state.
/// `None` without a transcript, or when its last turn did not end with text.
pub async fn last_message(projects: &Path, worktree: &Path) -> crate::Result<Option<String>> {
    let (projects, worktree) = (projects.to_path_buf(), worktree.to_path_buf());
    tokio::task::spawn_blocking(move || read_last_message(&projects, &worktree))
        .await
        .map_err(|e| crate::Error::Io {
            context: "read an agent's last message".into(),
            source: std::io::Error::other(e),
        })?
}

/// What the agent said when its turn ended in the transcript of Claude session `session_id`
/// for `worktree`, and only that one: another transcript in the folder, however new, is never
/// read. The outer `None` is no such transcript; the inner one a last turn without text.
pub async fn last_message_of_session(
    projects: &Path,
    worktree: &Path,
    session_id: &str,
) -> crate::Result<Option<Option<String>>> {
    let (projects, worktree) = (projects.to_path_buf(), worktree.to_path_buf());
    let session_id = session_id.to_string();
    tokio::task::spawn_blocking(move || {
        match steps::session_transcript(&projects, &worktree, &session_id)? {
            Some(path) => read_message(&path).map(Some),
            None => Ok(None),
        }
    })
    .await
    .map_err(|e| crate::Error::Io {
        context: "read an agent's last message".into(),
        source: std::io::Error::other(e),
    })?
}

fn read_last_message(projects: &Path, worktree: &Path) -> crate::Result<Option<String>> {
    let newest = match steps::worktree_made(worktree)? {
        Some(made) => steps::latest_transcript_since(projects, worktree, made)?,
        None => steps::latest_transcript(projects, worktree)?,
    };
    match newest {
        Some(path) => read_message(&path),
        None => Ok(None),
    }
}

fn read_message(path: &Path) -> crate::Result<Option<String>> {
    let turn = transcript::read(path).map_err(|source| crate::Error::Io {
        context: format!("read transcript {}", path.display()),
        source,
    })?;
    Ok(match turn {
        Some(Turn::Idle { text, .. }) if !text.trim().is_empty() => Some(text),
        _ => None,
    })
}

/// A finished turn without its own time ended when the file was last written.
fn ended_at(turn: Turn, modified: SystemTime) -> Turn {
    match turn {
        Turn::Idle { text, at: None } => Turn::Idle {
            text,
            at: Some(unix_millis(modified)),
        },
        other => other,
    }
}

static LAST_WARNING: Mutex<Option<String>> = Mutex::new(None);

/// Logs `message` unless it is the one logged last, so a poll does not repeat it.
fn warn_once(message: &str) {
    let mut last = LAST_WARNING.lock().unwrap_or_else(|p| p.into_inner());
    if last.as_deref() != Some(message) {
        tracing::warn!(target: "townhall::agent", event = "agent.state_warning",
            error = %crate::redact::redact(message), "agent state detection");
        *last = Some(message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_transcript_in_a_quiet_pane_is_ended() {
        let now = 1_000;
        assert_eq!(busy_state(now, now - QUIET_SECS + 1), AgentState::Working);
        assert_eq!(busy_state(now, now - QUIET_SECS), AgentState::Ended);
        assert_eq!(busy_state(now, now + 5), AgentState::Working);
    }

    #[test]
    fn exits_map_to_ended_or_error() {
        assert_eq!(exited(&Exit::Code(0)), AgentState::Ended);
        assert_eq!(
            exited(&Exit::Code(3)),
            AgentState::Error { exit_code: Some(3) }
        );
        assert_eq!(
            exited(&Exit::Signal("term".into())),
            AgentState::Error {
                exit_code: Some(143)
            }
        );
        assert_eq!(
            exited(&Exit::Signal("usr1".into())),
            AgentState::Error { exit_code: None }
        );
        assert_eq!(
            exited(&Exit::Unknown),
            AgentState::Error { exit_code: None }
        );
    }

    #[test]
    fn a_signal_is_read_as_a_name_in_any_case_or_a_number() {
        for form in ["TERM", "term", "SIGTERM", "sigterm", "15"] {
            assert_eq!(signal_number(form), Some(15), "{form}");
            assert_eq!(
                exited(&Exit::Signal(form.into())),
                AgentState::Error {
                    exit_code: Some(143)
                },
                "{form}"
            );
        }
        assert_eq!(signal_number("9"), Some(9));
        assert_eq!(signal_number("SIGKILL"), Some(9));
        for form in ["", "0", "-3", "usr1", "SIG", "sig"] {
            assert_eq!(signal_number(form), None, "{form}");
        }
    }

    #[test]
    fn projects_folder_is_the_one_steps_read() {
        assert_eq!(PROJECTS_ENV, "TOWNHALL_CLAUDE_PROJECTS");
        assert_eq!(claude_projects(), steps::claude_projects_dir().ok());
    }
}
