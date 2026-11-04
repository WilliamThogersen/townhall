//! Agent state detection against real tmux sessions and synthetic Claude transcripts in a
//! temp projects folder (never `~/.claude`). Session names carry the temp repo's key and
//! this process id, and a guard kills them even when a test fails.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};
use tempfile::TempDir;
use townhall_core::agent::{self, state, Agent, LaunchOptions, ShellAdapter};
use townhall_core::model::AgentState;
use townhall_core::{steps, tmux, Error, Workspace};

mod common;
use common::{fixture, Fixture};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

/// Kills the sessions it names when dropped.
struct Sessions(Vec<String>);

impl Drop for Sessions {
    fn drop(&mut self) {
        for session in &self.0 {
            let target = format!("={session}");
            let _ = std::process::Command::new("tmux")
                .args(["kill-session", "-t", &target])
                .output();
        }
    }
}

struct Env {
    fx: Fixture,
    projects: TempDir,
    _sessions: Sessions,
    name: String,
}

impl Env {
    fn ws(&self) -> &Workspace {
        &self.fx.ws
    }

    async fn agent(&self) -> Agent {
        agent::list(self.ws())
            .await
            .unwrap()
            .into_iter()
            .find(|a| a.name == self.name)
            .expect("the agent is listed")
    }

    async fn state(&self) -> AgentState {
        let found = self.agent().await;
        state::detect_in(Some(self.projects.path()), self.ws(), &found).await
    }

    /// Polls until the state is `want`, failing after a few seconds with the last one.
    async fn wait_for(&self, want: &AgentState) {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut now = self.state().await;
        while &now != want && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
            now = self.state().await;
        }
        assert_eq!(&now, want);
    }

    fn transcript(&self, worktree: &Path) -> PathBuf {
        let dir = steps::project_dir_for(self.projects.path(), worktree);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("session.jsonl")
    }

    async fn write_transcript(&self, lines: &[Value]) -> PathBuf {
        let worktree = self.agent().await.worktree;
        let path = self.transcript(&worktree);
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&path, text.join("\n") + "\n").unwrap();
        path
    }
}

async fn start(command: &str) -> Env {
    let fx = fixture().await;
    let name = unique("st");
    let sessions = Sessions(vec![tmux::session_name(fx.ws.key(), &name)]);
    let options = LaunchOptions {
        command: Some(command.to_string()),
        ..LaunchOptions::default()
    };
    agent::start(&fx.ws, &name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
    Env {
        fx,
        projects: TempDir::new().unwrap(),
        _sessions: sessions,
        name,
    }
}

fn assistant(content: Value, stop: Option<&str>) -> Value {
    json!({"type": "assistant", "isSidechain": false, "timestamp": "2026-11-04T10:00:00.000Z",
           "message": {"id": "m1", "stop_reason": stop, "content": content}})
}

fn ask(question: &str) -> Value {
    let input = json!({"questions": [{"question": question, "header": "q", "multiSelect": false,
        "options": [{"label": "yes", "description": "go"}, {"label": "no", "description": "stop"}]}]});
    assistant(
        json!([{"type": "tool_use", "id": "t1", "name": "AskUserQuestion", "input": input}]),
        Some("tool_use"),
    )
}

#[tokio::test]
async fn exit_zero_is_ended_and_the_dead_pane_is_not_live() {
    let env = start("true").await;
    env.wait_for(&AgentState::Ended).await;
    assert!(!env.agent().await.alive);
    assert!(!tmux::list(env.ws().key())
        .await
        .unwrap()
        .contains(&env.name));
    let output = agent::output(env.ws(), &env.name).await.unwrap_err();
    assert!(matches!(output, Error::NotFound(_)), "{output:?}");
    let sent = agent::send(env.ws(), &env.name, "x").await.unwrap_err();
    assert!(matches!(sent, Error::NotFound(_)), "{sent:?}");
    agent::stop(env.ws(), &env.name).await.unwrap();
    assert!(tmux::pane(env.ws().key(), &env.name)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn exit_three_is_an_error_with_its_code() {
    let env = start("sh -c 'echo failing; exit 3'").await;
    env.wait_for(&AgentState::Error { exit_code: Some(3) })
        .await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_signal_is_reported_like_a_shell_does() {
    let env = start("sh -c 'kill -TERM $$'").await;
    env.wait_for(&AgentState::Error {
        exit_code: Some(143),
    })
    .await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_shell_question_waits_until_it_is_answered() {
    let env = start("sh -c 'printf \"continue? [y/n] \"; read a; echo got-$a; sleep 30'").await;
    env.wait_for(&AgentState::Waiting {
        question: "continue? [y/n]".into(),
    })
    .await;
    agent::send(env.ws(), &env.name, "y").await.unwrap();
    env.wait_for(&AgentState::Working).await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_live_agent_without_a_question_works_and_a_gone_session_ended() {
    let env = start("sleep 30").await;
    env.wait_for(&AgentState::Working).await;
    tmux::kill(env.ws().key(), &env.name).await.unwrap();
    env.wait_for(&AgentState::Ended).await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_pending_ask_in_the_transcript_waits_with_its_options() {
    let env = start("sleep 30").await;
    env.wait_for(&AgentState::Working).await;
    let path = env
        .write_transcript(&[
            json!({"type": "user", "message": {"content": "go"}}),
            ask("Ship it?"),
        ])
        .await;
    env.wait_for(&AgentState::Waiting {
        question: "Ship it?\n1. yes\n2. no".into(),
    })
    .await;
    let answered = json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "t1", "content": "yes"}]}});
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!("{answered}\n"));
    std::fs::write(&path, text).unwrap();
    env.wait_for(&AgentState::Working).await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_turn_ending_on_a_question_waits() {
    let env = start("sleep 30").await;
    let text = json!([{"type": "text", "text": "Fixed the redirect.\n\nShould I push?"}]);
    env.write_transcript(&[assistant(text, Some("end_turn"))])
        .await;
    env.wait_for(&AgentState::Waiting {
        question: "Should I push?".into(),
    })
    .await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_transcript_older_than_the_session_is_ignored() {
    let env = start("sleep 30").await;
    let path = env.write_transcript(&[ask("Old question?")]).await;
    let old = SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();
    env.wait_for(&AgentState::Working).await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_permission_prompt_on_screen_waits_with_the_pending_call() {
    let screen = "printf \"Bash command\\n\\n  rm -rf build\\n\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n\"";
    let env = start(&format!("sh -c '{screen}; sleep 30'")).await;
    let call = json!([{"type": "tool_use", "id": "t9", "name": "Bash", "input": {"command": "rm -rf build"}}]);
    env.write_transcript(&[assistant(call, Some("tool_use"))])
        .await;
    env.wait_for(&AgentState::Waiting {
        question: "Bash: rm -rf build\nDo you want to proceed?\n1. Yes\n2. No".into(),
    })
    .await;
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_dead_session_of_the_same_name_is_replaced_on_spawn() {
    let fx = fixture().await;
    let name = unique("respawn");
    let _sessions = Sessions(vec![tmux::session_name(fx.ws.key(), &name)]);
    let dir = TempDir::new().unwrap();
    tmux::spawn(fx.ws.key(), &name, dir.path(), "true")
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !tmux::panes(fx.ws.key())
        .await
        .unwrap()
        .iter()
        .any(|p| p.dead())
    {
        assert!(Instant::now() < deadline, "the pane never died");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tmux::spawn(fx.ws.key(), &name, dir.path(), "sleep 30")
        .await
        .unwrap();
    assert_eq!(tmux::list(fx.ws.key()).await.unwrap(), vec![name.clone()]);
    let live = tmux::spawn(fx.ws.key(), &name, dir.path(), "sleep 30").await;
    assert!(matches!(live, Err(Error::Command { .. })), "{live:?}");
    tmux::kill(fx.ws.key(), &name).await.unwrap();
}

#[tokio::test]
async fn a_name_never_matches_a_longer_session() {
    let fx = fixture().await;
    let long = unique("prefix");
    let short = &long[..long.len() - 1];
    let _sessions = Sessions(vec![tmux::session_name(fx.ws.key(), &long)]);
    let dir = TempDir::new().unwrap();
    tmux::spawn(fx.ws.key(), &long, dir.path(), "sleep 30")
        .await
        .unwrap();
    assert!(tmux::capture(fx.ws.key(), short).await.is_err());
    assert!(tmux::kill(fx.ws.key(), short).await.is_err());
    assert!(tmux::pane(fx.ws.key(), short).await.unwrap().is_none());
    tmux::kill(fx.ws.key(), &long).await.unwrap();
}
