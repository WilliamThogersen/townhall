#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use townhall_core::{Home, Workspace};

mod tmux_server;
#[allow(unused_imports)]
pub use tmux_server::{private_tmux, tmux};

pub async fn git(dir: &Path, args: &[&str]) -> String {
    townhall_core::cmd::run("git", args, Some(dir))
        .await
        .unwrap()
}

async fn commit(dir: &Path) {
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ],
    )
    .await;
}

pub async fn init_repo() -> TempDir {
    private_tmux();
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-b", "main"]).await;
    commit(dir.path()).await;
    dir
}

/// A repo with one commit on `branch`, in a folder called `name` under `parent`.
pub async fn init_named(parent: &Path, name: &str, branch: &str) -> PathBuf {
    private_tmux();
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-b", branch]).await;
    commit(&dir).await;
    dir
}

/// A clone of a repo whose default branch is `develop`, so `origin/HEAD` points at it.
/// The clone itself is checked out on `feature`.
pub async fn clone_with_origin_head(parent: &Path) -> PathBuf {
    let origin = init_named(parent, "origin-src", "develop").await;
    let clone = parent.join("cloned");
    let (from, to) = (origin.to_string_lossy(), clone.to_string_lossy());
    git(parent, &["clone", "-q", &from, &to]).await;
    git(&clone, &["checkout", "-q", "-b", "feature"]).await;
    clone
}

/// A throwaway repo plus a throwaway townhall home. Never touches the real `~/.townhall`.
pub struct Fixture {
    pub repo: TempDir,
    pub home: TempDir,
    pub ws: Workspace,
}

pub async fn fixture() -> Fixture {
    let repo = init_repo().await;
    let home = TempDir::new().unwrap();
    let ws = Workspace::new(Home::at(home.path().join(".townhall")), repo.path()).unwrap();
    Fixture { repo, home, ws }
}

/// A store in a throwaway home, with its bus.
pub struct StoreEnv {
    pub dir: TempDir,
    pub bus: townhall_core::events::EventBus,
    pub store: townhall_core::store::Store,
}

pub fn store_env() -> StoreEnv {
    private_tmux();
    let dir = TempDir::new().unwrap();
    let bus = townhall_core::events::EventBus::new();
    let store =
        townhall_core::store::Store::open(&Home::at(dir.path().join(".townhall")), bus.clone())
            .unwrap();
    StoreEnv { dir, bus, store }
}

pub fn new_task(name: &str) -> townhall_core::model::NewTask {
    townhall_core::model::NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
    }
}

pub fn you(text: &str) -> townhall_core::model::EntryBody {
    townhall_core::model::EntryBody::You {
        text: text.into(),
        attachments: vec![],
        reply_to: None,
    }
}
