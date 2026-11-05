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

/// A dead pane's state: exit 0 ended, anything else an error. A signal is reported the
/// way a shell does, 128 plus its number.
fn exited(exit: &Exit) -> AgentState {
    match exit {
        Exit::Code(0) => AgentState::Ended,
        Exit::Code(code) => AgentState::Error {
            exit_code: Some(*code),
        },
        Exit::Signal(name) => AgentState::Error {
            exit_code: SIGNALS
                .iter()
                .find(|(n, _)| name.eq_ignore_ascii_case(n))
                .map(|(_, number)| 128 + number),
        },
        Exit::Unknown => AgentState::Error { exit_code: None },
    }
}

fn waiting(question: String) -> AgentState {
    AgentState::Waiting { question }
}

async fn live(projects: Option<&Path>, ws: &Workspace, agent: &Agent, pane: &Pane) -> AgentState {
    let turn = match projects {
        Some(dir) => current_turn(dir, &agent.worktree, pane.created).await,
        None => None,
    };
    let asked = |q: Option<String>| q.map_or(AgentState::Working, waiting);
    match turn {
        Some(Turn::Asking { question }) => waiting(question),
        Some(Turn::Busy) => asked(on_screen(ws, agent, pane, false).await),
        Some(Turn::Pending { call }) => asked(
            on_screen(ws, agent, pane, false)
                .await
                .map(|q| question::shorten(&format!("{call}\n{q}"))),
        ),
        Some(Turn::Idle { text, at }) => idle(ws, agent, pane, &text, at).await,
        None => asked(on_screen(ws, agent, pane, true).await),
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
async fn current_turn(projects: &Path, worktree: &Path, since: u64) -> Option<Turn> {
    let (projects, worktree) = (projects.to_path_buf(), worktree.to_path_buf());
    let read = tokio::task::spawn_blocking(move || read_turn(&projects, &worktree, since)).await;
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

fn read_turn(projects: &Path, worktree: &Path, since: u64) -> crate::Result<Option<Turn>> {
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
    let turn = transcript::read(&path)
        .map_err(io)?
        .map(|t| ended_at(t, modified));
    memo::keep_turn(&path, stamp, turn.clone());
    Ok(turn)
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
        eprintln!("warn: {message}");
        *last = Some(message.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn projects_folder_is_the_one_steps_read() {
        assert_eq!(PROJECTS_ENV, "TOWNHALL_CLAUDE_PROJECTS");
        assert_eq!(claude_projects(), steps::claude_projects_dir().ok());
    }
}
