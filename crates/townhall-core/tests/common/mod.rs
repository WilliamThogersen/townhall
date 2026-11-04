#![allow(dead_code)]

use std::path::Path;

use tempfile::TempDir;
use townhall_core::{Home, Workspace};

pub async fn git(dir: &Path, args: &[&str]) -> String {
    townhall_core::cmd::run("git", args, Some(dir))
        .await
        .unwrap()
}

pub async fn init_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-b", "main"]).await;
    git(
        dir.path(),
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
    dir
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
