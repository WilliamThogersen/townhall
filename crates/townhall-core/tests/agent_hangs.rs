use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};
use tempfile::TempDir;
use townhall_core::agent::{self, shell_quote, state, Agent, LaunchOptions, ShellAdapter};
use townhall_core::model::AgentState;
use townhall_core::{clock, steps, tmux, Workspace};

mod common;
use common::{fixture, Fixture};

const STALL_SECS: &str = "2";

static COUNTER: AtomicUsize = AtomicUsize::new(0);
static ENV: Once = Once::new();

struct Sessions(Vec<String>);

impl Drop for Sessions {
    fn drop(&mut self) {
        for session in &self.0 {
            let target = format!("={session}");
            let _ = common::tmux()
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

    async fn until(&self, within: Duration, done: impl Fn(&AgentState) -> bool) -> AgentState {
        let deadline = Instant::now() + within;
        loop {
            let now = self.state().await;
            if done(&now) || Instant::now() >= deadline {
                return now;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn typed(&self, word: &str) -> usize {
        tmux::capture(self.ws().key(), &self.name)
            .await
            .unwrap()
            .matches(word)
            .count()
    }

    async fn transcript_path(&self) -> PathBuf {
        let worktree = self.agent().await.worktree;
        let dir = steps::project_dir_for(self.projects.path(), &worktree);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("session.jsonl")
    }

    async fn write_transcript(&self, lines: &[Value]) -> PathBuf {
        let path = self.transcript_path().await;
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&path, text.join("\n") + "\n").unwrap();
        path
    }
}

async fn start(command: &str) -> Env {
    ENV.call_once(|| std::env::set_var(state::STALL_ENV, STALL_SECS));
    let fx = fixture().await;
    let name = format!(
        "hang-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
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

fn is_stalled(s: &AgentState) -> bool {
    matches!(s, AgentState::Waiting { question } if question.starts_with("Stalled"))
}

fn is_api_stop(s: &AgentState) -> bool {
    matches!(s, AgentState::Waiting { question } if question.starts_with("Stopped on an API"))
}

fn api_error_ending(ago_ms: i64) -> [Value; 2] {
    let at = clock::from_unix_millis(clock::unix_millis(SystemTime::now()) - ago_ms);
    [
        json!({"type": "user", "timestamp": at, "message": {"content": "go"}}),
        json!({"type": "assistant", "isApiErrorMessage": true, "timestamp": at,
               "message": {"id": "e1", "stop_reason": "stop_sequence",
                           "content": [{"type": "text", "text": "API Error: 529 overloaded"}]}}),
    ]
}

#[tokio::test]
async fn a_silent_busy_agent_is_nudged_once_then_surfaced_as_stalled() {
    let env = start("cat").await;
    assert_eq!(env.state().await, AgentState::Working);
    let started = Instant::now();
    let now = env.until(Duration::from_secs(12), is_stalled).await;
    assert!(is_stalled(&now), "{now:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        env.typed("continue").await,
        2,
        "typed once and echoed by cat"
    );
    assert!(is_stalled(&env.state().await));
    assert_eq!(env.typed("continue").await, 2, "never nudged twice");
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_stalled_agent_that_gets_input_works_again() {
    let env = start("cat").await;
    let now = env.until(Duration::from_secs(12), is_stalled).await;
    assert!(is_stalled(&now), "{now:?}");
    agent::send(env.ws(), &env.name, "more").await.unwrap();
    assert_eq!(env.state().await, AgentState::Working);
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_growing_transcript_keeps_a_quiet_pane_from_stalling() {
    let env = start("cat").await;
    let path = env
        .write_transcript(&[json!({"type": "user", "message": {"content": "go"}})])
        .await;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&format!(
            "{}\n",
            json!({"type": "user", "message": {"content": "more"}})
        ));
        std::fs::write(&path, text).unwrap();
        assert_eq!(env.state().await, AgentState::Working);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(
        env.typed("continue").await,
        0,
        "no nudge while the transcript grows"
    );
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn an_api_error_is_resumed_once_then_surfaced() {
    let env = start("cat").await;
    env.write_transcript(&api_error_ending(10_000)).await;
    let started = Instant::now();
    assert_eq!(env.state().await, AgentState::Working);
    assert_eq!(
        env.typed("continue").await,
        2,
        "resumed with one `continue`"
    );
    let now = env.until(Duration::from_secs(5), is_api_stop).await;
    assert!(is_api_stop(&now), "{now:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(env.typed("continue").await, 2, "never resumed twice");
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_fresh_api_error_waits_for_claudes_own_retry() {
    let env = start("cat").await;
    env.write_transcript(&api_error_ending(0)).await;
    assert_eq!(env.state().await, AgentState::Working);
    assert_eq!(env.typed("continue").await, 0);
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_went_to_sleep_screen_is_resumed_once_then_surfaced() {
    let script = format!(
        "printf '%s\\n' {}; cat",
        shell_quote("The session went to sleep")
    );
    let env = start(&format!("sh -c {}", shell_quote(&script))).await;
    let started = Instant::now();
    let now = env.until(Duration::from_secs(14), is_api_stop).await;
    assert!(is_api_stop(&now), "{now:?}");
    assert!(
        started.elapsed() < Duration::from_secs(14),
        "{:?}",
        started.elapsed()
    );
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_permission_prompt_on_screen_waits_within_seconds() {
    let screen = "Do you want to proceed?\n1. Yes\n2. No\n";
    let script = format!("printf '%s' {}; sleep 30", shell_quote(screen));
    let env = start(&format!("sh -c {}", shell_quote(&script))).await;
    let started = Instant::now();
    let now = env
        .until(Duration::from_secs(5), |s| {
            matches!(s, AgentState::Waiting { .. })
        })
        .await;
    assert!(matches!(now, AgentState::Waiting { .. }), "{now:?}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    agent::stop(env.ws(), &env.name).await.unwrap();
}

#[tokio::test]
async fn a_dead_process_is_an_error_within_seconds() {
    let env = start("sh -c 'sleep 1; exit 7'").await;
    let started = Instant::now();
    let now = env
        .until(Duration::from_secs(8), |s| {
            matches!(s, AgentState::Error { .. })
        })
        .await;
    assert_eq!(now, AgentState::Error { exit_code: Some(7) });
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    agent::stop(env.ws(), &env.name).await.unwrap();
}
