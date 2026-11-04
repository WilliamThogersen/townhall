//! What one watcher poll costs with 50 sleeping shell agents. Slow to set up, so it is
//! ignored by default: `cargo test -p townhall-core --test agent_watch_cost -- --ignored
//! --nocapture` prints the timings.

use std::time::{Duration, Instant};

use tempfile::TempDir;
use townhall_core::agent::watch::Watcher;
use townhall_core::agent::{self, state, LaunchOptions, ShellAdapter};
use townhall_core::{tmux, Context, ContextOptions, Home, Workspace};

mod common;

const AGENTS: usize = 50;

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

async fn timed(watcher: &mut Watcher) -> Duration {
    let started = Instant::now();
    watcher.poll().await;
    started.elapsed()
}

#[tokio::test]
#[ignore = "starts 50 agents; run by hand to measure"]
async fn one_poll_of_fifty_sleeping_agents() {
    let projects = TempDir::new().unwrap();
    std::env::set_var(state::PROJECTS_ENV, projects.path());
    let repo = common::init_repo().await;
    let home = TempDir::new().unwrap();
    let ws = Workspace::new(Home::at(home.path().join(".townhall")), repo.path()).unwrap();
    let ctx = Context::open(ws.clone(), ContextOptions::default()).unwrap();
    let mut sessions = Sessions(vec![]);
    let options = LaunchOptions {
        command: Some("sleep 120".into()),
        ..LaunchOptions::default()
    };
    for i in 0..AGENTS {
        let name = format!("cost-{}-{i}", std::process::id());
        sessions.0.push(tmux::session_name(ws.key(), &name));
        agent::start(&ws, &name, "main", &ShellAdapter, &options)
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut watcher = Watcher::new(ctx).await;
    let first = timed(&mut watcher).await;
    let mut steady = vec![];
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        steady.push(timed(&mut watcher).await);
    }
    let mean = steady.iter().sum::<Duration>() / steady.len() as u32;
    let worst = steady.iter().max().copied().unwrap_or_default();
    eprintln!(
        "info: {AGENTS} agents, first poll {first:?}, later polls mean {mean:?}, worst {worst:?}"
    );
    assert!(
        worst < Duration::from_millis(1500),
        "a poll fits its interval"
    );
    for i in 0..AGENTS {
        let name = format!("cost-{}-{i}", std::process::id());
        agent::stop(&ws, &name).await.unwrap();
    }
}
