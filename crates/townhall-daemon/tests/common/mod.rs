#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;
use townhall_core::{ContextOptions, Home, Workspace};
use townhall_daemon::{router, AppState};

#[path = "../../../townhall-core/tests/common/tmux_server.rs"]
mod tmux_server;
#[allow(unused_imports)]
pub use tmux_server::{private_tmux, tmux};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

pub async fn git(dir: &Path, args: &[&str]) -> String {
    townhall_core::cmd::run("git", args, Some(dir))
        .await
        .unwrap()
}

/// A repo with one commit on `main`, in a folder called `name` under `parent`.
pub async fn repo_at(parent: &Path, name: &str) -> PathBuf {
    private_tmux();
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]).await;
    git(
        &dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
    )
    .await;
    dir.canonicalize().unwrap()
}

/// Temp folders for repos and a throwaway townhall home. Never the real `~/.townhall`.
pub struct Env {
    pub root: TempDir,
    pub home: Home,
    pub startup: PathBuf,
}

impl Env {
    pub async fn new(startup_name: &str) -> Self {
        private_tmux();
        let root = TempDir::new().unwrap();
        let startup = repo_at(root.path(), startup_name).await;
        let home = Home::at(root.path().join(".townhall"));
        Self {
            root,
            home,
            startup,
        }
    }

    pub async fn repo(&self, name: &str) -> PathBuf {
        repo_at(self.root.path(), name).await
    }

    pub fn workspace(&self, repo: &Path) -> Workspace {
        Workspace::new(self.home.clone(), repo).unwrap()
    }

    /// Opens the store: once per `Env`, a second open is refused while the first is held.
    pub fn state(&self) -> AppState {
        AppState::new(self.workspace(&self.startup))
    }

    pub fn state_with(&self, options: ContextOptions) -> AppState {
        AppState::with_options(self.workspace(&self.startup), options)
    }

    pub fn start(&self) -> (AppState, Router) {
        let state = self.state();
        let app = router(state.clone());
        (state, app)
    }

    /// Writes a phase 1 `config.json` before the store is opened.
    pub fn write_config(&self, value: &Value) {
        self.home.ensure().unwrap();
        std::fs::write(self.home.config_file(), value.to_string()).unwrap();
    }
}

pub async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

pub async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    send(app, req).await
}

pub async fn settle() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}
