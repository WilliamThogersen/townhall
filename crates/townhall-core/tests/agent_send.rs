//! What `/send` puts into an agent's pane: text as it is (never read as tmux key names)
//! followed by Enter, or one key on its own. The agent records every byte it gets.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use townhall_core::agent::{self, shell_quote, LaunchOptions, ShellAdapter};
use townhall_core::model::AgentKey;
use townhall_core::Error;

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

/// An agent that writes every byte it reads to a file (no line editing, no echo).
async fn recorder(fx: &Fixture, name: &str) -> PathBuf {
    let file = fx.home.path().join(format!("{name}.bytes"));
    let script = format!(
        "stty -icanon -echo; echo ready > {0}.ready; exec cat > {0}",
        shell_quote(&file.to_string_lossy())
    );
    let options = LaunchOptions {
        command: Some(format!("sh -c {}", shell_quote(&script))),
        ..LaunchOptions::default()
    };
    agent::start(&fx.ws, name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
    let ready = PathBuf::from(format!("{}.ready", file.display()));
    received(&ready, "ready\n").await;
    file
}

/// What the agent recorded, once it is `want` or a few seconds passed.
async fn received(file: &Path, want: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let got = std::fs::read_to_string(file).unwrap_or_default();
        if got == want || Instant::now() > deadline {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn text_is_typed_as_it_is_then_enter() {
    let fx = fixture().await;
    let name = unique("typed");
    let file = recorder(&fx, &name).await;
    let texts = [
        "Down",
        "C-c",
        "-x Escape",
        "fix it;",
        ";",
        "a\\;",
        "#{pane_id}",
    ];
    for text in texts {
        agent::send(&fx.ws, &name, text).await.unwrap();
    }
    let want = texts.map(|t| format!("{t}\n")).concat();
    assert_eq!(received(&file, &want).await, want);
    agent::stop(&fx.ws, &name).await.unwrap();
}

#[tokio::test]
async fn a_key_is_pressed_alone() {
    let fx = fixture().await;
    let name = unique("keys");
    let file = recorder(&fx, &name).await;
    let keys = [
        AgentKey::Up,
        AgentKey::Down,
        AgentKey::Tab,
        AgentKey::Escape,
        AgentKey::Enter,
    ];
    for key in keys {
        agent::send_key(&fx.ws, &name, key).await.unwrap();
    }
    let want = "\u{1b}[A\u{1b}[B\t\u{1b}\n";
    assert_eq!(received(&file, want).await, want);
    agent::stop(&fx.ws, &name).await.unwrap();
}

#[tokio::test]
async fn a_key_for_a_missing_or_dead_agent_is_not_found() {
    let fx = fixture().await;
    let missing = agent::send_key(&fx.ws, &unique("ghost"), AgentKey::Enter).await;
    assert!(matches!(missing, Err(Error::NotFound(_))), "{missing:?}");
    let name = unique("gone");
    let options = LaunchOptions {
        command: Some("true".into()),
        ..LaunchOptions::default()
    };
    agent::start(&fx.ws, &name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while agent::list(&fx.ws).await.unwrap()[0].alive && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let dead = agent::send_key(&fx.ws, &name, AgentKey::Down).await;
    assert!(matches!(dead, Err(Error::NotFound(_))), "{dead:?}");
    agent::stop(&fx.ws, &name).await.unwrap();
}
