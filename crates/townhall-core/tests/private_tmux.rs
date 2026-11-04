//! Tests run their agents on a tmux server of their own (`TOWNHALL_TMUX_SOCKET`), never on
//! the default server real agents use.

use std::process::Command;

use townhall_core::agent::{self, LaunchOptions, ShellAdapter};
use townhall_core::tmux;

mod common;
use common::fixture;

fn has_session(mut tmux: Command, session: &str) -> bool {
    let target = format!("={session}");
    tmux.args(["has-session", "-t", &target])
        .output()
        .is_ok_and(|out| out.status.success())
}

#[tokio::test]
async fn agents_start_on_the_test_process_own_server() {
    let fx = fixture().await;
    let socket = std::env::var(tmux::SOCKET_ENV).unwrap_or_default();
    let own = format!("townhall-test-{}-", std::process::id());
    assert!(socket.starts_with(&own), "{socket:?}");

    let name = format!("private-{}", std::process::id());
    let options = LaunchOptions {
        command: Some("sleep 60".into()),
        ..LaunchOptions::default()
    };
    let started = agent::start(&fx.ws, &name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
    let session = tmux::session_name(fx.ws.key(), &name);
    let on_own = has_session(common::tmux(), &session);
    let on_default = has_session(Command::new("tmux"), &session);
    let pwd = common::tmux()
        .args(["show-environment", "-t", &format!("={session}"), "PWD"])
        .output()
        .unwrap();
    agent::stop(&fx.ws, &name).await.unwrap();

    assert!(on_own && !on_default, "own {on_own}, default {on_default}");
    let pwd = String::from_utf8_lossy(&pwd.stdout).trim().to_string();
    assert_eq!(pwd, format!("PWD={}", started.worktree.display()));
}
