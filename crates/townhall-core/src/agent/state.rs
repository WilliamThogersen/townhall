//! Agent state detection, from tmux and Claude's transcript:
//! - no session: `ended`; a dead pane (sessions keep it with `remain-on-exit`): `ended`
//!   when the process exited 0, else `error` with the exit code,
//! - a live pane is `waiting` when the agent asks something, else `working`. It asks when
//!   Claude's transcript (written since the session started) has a pending
//!   `AskUserQuestion` or a turn that ended on a question, when the screen shows a
//!   selection prompt (a permission request only ever shows there), or, for a program
//!   without a transcript, when its last line is a question.

mod memo;
mod question;
mod screen;
mod transcript;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub use question::choices;

use super::Agent;
use crate::model::AgentState;
use crate::tmux::{self, Exit, Pane};
use crate::{steps, Workspace};
use transcript::Turn;

/// Overrides where Claude's transcripts are read from (tests point it at a temp folder).
pub const PROJECTS_ENV: &str = "TOWNHALL_CLAUDE_PROJECTS";

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Claude's projects folder: `$TOWNHALL_CLAUDE_PROJECTS`, else `$CLAUDE_CONFIG_DIR/projects`
/// (where Claude itself writes then), else `~/.claude/projects`.
pub fn claude_projects() -> Option<PathBuf> {
    env_dir(PROJECTS_ENV)
        .or_else(|| env_dir("CLAUDE_CONFIG_DIR").map(|d| d.join("projects")))
        .or_else(|| env_dir("HOME").map(|h| h.join(".claude").join("projects")))
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
    match turn {
        Some(Turn::Asking { question }) => waiting(question),
        Some(Turn::Busy) => AgentState::Working,
        Some(Turn::Idle { text }) => match question::closing(&text) {
            Some(question) => waiting(question),
            None => on_screen(ws, agent, pane, None, false).await,
        },
        Some(Turn::Pending { call }) => on_screen(ws, agent, pane, Some(call), false).await,
        None => on_screen(ws, agent, pane, None, true).await,
    }
}

/// A prompt on screen (any agent) or, when `plain`, a last line asking something. `call`
/// is the pending tool call the prompt is about.
async fn on_screen(
    ws: &Workspace,
    agent: &Agent,
    pane: &Pane,
    call: Option<String>,
    plain: bool,
) -> AgentState {
    let Some(text) = screen_text(ws, agent, pane).await else {
        return AgentState::Working;
    };
    let plain_question = || plain.then(|| screen::trailing_question(&text)).flatten();
    match screen::prompt(&text).or_else(plain_question) {
        Some(q) => waiting(match call {
            Some(call) => question::shorten(&format!("{call}\n{q}")),
            None => q,
        }),
        None => AgentState::Working,
    }
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
    let turn = transcript::read(&path).map_err(io)?;
    memo::keep_turn(&path, stamp, turn.clone());
    Ok(turn)
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
    fn projects_folder_is_found() {
        assert!(claude_projects().is_some());
        assert_eq!(env_dir("TOWNHALL_TEST_SURELY_UNSET_VAR"), None);
    }
}
