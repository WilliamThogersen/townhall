//! Agent states over HTTP and from the background watcher, with real tmux sessions.
//! Transcripts are read from a temp folder, never `~/.claude`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::agent::state::PROJECTS_ENV;
use townhall_core::model::{AgentState, ServerEvent};
use townhall_core::tmux;
use townhall_daemon::{router, start_background};

mod common;
use common::{call, unique, Env};

static PROJECTS: OnceLock<std::path::PathBuf> = OnceLock::new();

fn temp_projects() {
    PROJECTS.get_or_init(|| {
        let dir = tempfile::Builder::new()
            .prefix("townhall-daemon-projects")
            .tempdir()
            .unwrap()
            .keep();
        std::env::set_var(PROJECTS_ENV, &dir);
        dir
    });
}

/// Kills the session when dropped, also when the test fails.
struct Session(String);

impl Drop for Session {
    fn drop(&mut self) {
        let target = format!("={}", self.0);
        let _ = std::process::Command::new("tmux")
            .args(["kill-session", "-t", &target])
            .output();
    }
}

fn shell(name: &str, command: &str) -> Value {
    json!({"name": name, "adapter": "shell", "command": command})
}

#[tokio::test]
async fn get_agents_shows_the_detected_state() {
    temp_projects();
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let name = unique("http");
    let _session = Session(tmux::session_name(env.workspace(&env.startup).key(), &name));
    let (s, v) = call(
        &app,
        "POST",
        "/api/agents",
        Some(shell(&name, "sh -c 'exit 3'")),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let deadline = Instant::now() + Duration::from_secs(8);
    let found = loop {
        let (_, v) = call(&app, "GET", "/api/agents", None).await;
        let found = v["data"][0].clone();
        if found["state"]["kind"] == "error" || Instant::now() > deadline {
            break found;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert_eq!(found["state"], json!({"kind": "error", "exitCode": 3}));
    assert_eq!(found["alive"], false);
    let (s, _) = call(&app, "GET", &format!("/api/agents/{name}/output"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "a dead pane has no live output");
    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn start_background_runs_the_watcher_and_it_publishes_states() {
    temp_projects();
    let env = Env::new("alpha").await;
    let state = env.state();
    let app = router(state.clone());
    let mut rx = state.ctx().bus().subscribe();
    let background = start_background(&state).await;
    assert!(background.running() >= 1, "the agent watcher runs");
    let name = unique("bg");
    let _session = Session(tmux::session_name(env.workspace(&env.startup).key(), &name));
    let (s, v) = call(&app, "POST", "/api/agents", Some(shell(&name, "sleep 30"))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let published = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            match rx.recv().await {
                Ok(ServerEvent::AgentUpdated { agent }) if agent.name == name => break agent,
                Ok(_) => {}
                Err(e) => panic!("bus closed: {e}"),
            }
        }
    })
    .await
    .expect("agentUpdated within a few polls");
    assert_eq!(published.state, AgentState::Working);
    assert_eq!(published.repo, "alpha");
    background.stop();
    let (s, _) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert_eq!(s, StatusCode::OK);
}
