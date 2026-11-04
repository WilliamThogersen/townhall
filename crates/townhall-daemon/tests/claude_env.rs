//! Claude, in townhall's own session and in an agent's tmux session, never sees the
//! variables of the Claude Code session that started townhall, while the user's other
//! `CLAUDE_*` and `ANTHROPIC_*` variables reach it. The only test in its process: it sets
//! them in its own environment before anything runs, so the tmux server it starts holds
//! them too.

use axum::http::StatusCode;
use serde_json::json;

mod common;
mod runner_support;
use common::{call, unique, Env};
use runner_support::{app_with, create, idle, Fake};

const OUTER: [(&str, &str); 5] = [
    ("CLAUDECODE", "1"),
    (
        "CLAUDE_CODE_SESSION_ID",
        "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b",
    ),
    ("CLAUDE_CODE_ENTRYPOINT", "cli"),
    ("CLAUDE_CODE_SSE_PORT", "4242"),
    ("CLAUDE_EFFORT", "xhigh"),
];
const KEPT: [(&str, &str); 2] = [
    ("CLAUDE_CONFIG_DIR", "/kept/claude-config"),
    ("ANTHROPIC_BASE_URL", "http://kept.invalid"),
];

fn assert_clean(env: &str, who: &str) {
    for (name, _) in OUTER {
        let set = env.lines().any(|l| l.starts_with(&format!("{name}=")));
        assert!(!set, "{who} inherited {name}:\n{env}");
    }
    for (name, value) in KEPT {
        let line = format!("{name}={value}");
        assert!(env.lines().any(|l| l == line), "{who} lost {name}:\n{env}");
    }
}

#[tokio::test]
async fn claude_starts_without_the_outer_sessions_variables() {
    for (name, value) in OUTER.iter().chain(&KEPT) {
        std::env::set_var(name, value);
    }
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "hello").await;
    idle(&app, created["conversation"]["id"].as_str().unwrap()).await;
    assert_clean(&fake.env(1).await, "townhall's session");

    let name = unique("env");
    let body = json!({"name": name, "adapter": "claude"});
    let (s, v) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let agent = fake.env(2).await;
    call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert!(
        agent.contains("TMUX_PANE="),
        "the fake ran in tmux:\n{agent}"
    );
    assert_clean(&agent, "the agent");
}
