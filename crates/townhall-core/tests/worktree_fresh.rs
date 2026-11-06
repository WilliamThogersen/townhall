mod common;

use std::path::{Path, PathBuf};

use common::git;
use tempfile::TempDir;
use townhall_core::{worktree, Home, Workspace};

const IDENTITY: [&str; 4] = ["-c", "user.name=t", "-c", "user.email=t@t"];

async fn commit(dir: &Path, file: &str) -> String {
    std::fs::write(dir.join(file), file).unwrap();
    git(dir, &["add", "-A"]).await;
    let mut args = IDENTITY.to_vec();
    args.extend(["commit", "-q", "-m", file]);
    git(dir, &args).await;
    git(dir, &["rev-parse", "HEAD"]).await
}

struct Remote {
    root: TempDir,
    bare: PathBuf,
    seed: PathBuf,
}

async fn remote() -> Remote {
    let root = TempDir::new().unwrap();
    let bare = root.path().join("origin.git");
    let seed = root.path().join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    let bare_arg = bare.to_string_lossy().into_owned();
    git(
        root.path(),
        &["init", "-q", "--bare", "-b", "main", &bare_arg],
    )
    .await;
    git(&seed, &["init", "-q", "-b", "main"]).await;
    commit(&seed, "one").await;
    git(&seed, &["remote", "add", "origin", &bare_arg]).await;
    git(&seed, &["push", "-q", "origin", "main"]).await;
    Remote { root, bare, seed }
}

impl Remote {
    async fn clone_to(&self, dest: &Path) {
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let (from, to) = (self.bare.to_string_lossy(), dest.to_string_lossy());
        git(self.root.path(), &["clone", "-q", &from, &to]).await;
    }

    async fn publish(&self, file: &str) -> String {
        let sha = commit(&self.seed, file).await;
        git(&self.seed, &["push", "-q", "origin", "main"]).await;
        sha
    }
}

fn workspace(home: &Path, repo: &Path) -> Workspace {
    Workspace::new(Home::at(home), repo).unwrap()
}

#[tokio::test]
async fn a_managed_clone_is_fast_forwarded_and_used_as_the_base() {
    let remote = remote().await;
    let home = TempDir::new().unwrap();
    let clone = home.path().join("repos/shop");
    remote.clone_to(&clone).await;
    let newest = remote.publish("two").await;
    let ws = workspace(home.path(), &clone);

    let base = worktree::fresh_base(&ws, "main").await;

    assert_eq!(base, "main");
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, newest);
    assert!(clone.join("two").exists(), "checked out branch moved too");
}

#[tokio::test]
async fn a_managed_clone_not_checked_out_on_the_base_still_moves() {
    let remote = remote().await;
    let home = TempDir::new().unwrap();
    let clone = home.path().join("repos/shop");
    remote.clone_to(&clone).await;
    git(&clone, &["checkout", "-q", "-b", "other"]).await;
    let newest = remote.publish("two").await;
    let ws = workspace(home.path(), &clone);

    assert_eq!(worktree::fresh_base(&ws, "main").await, "main");
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, newest);
    assert_eq!(git(&clone, &["branch", "--show-current"]).await, "other");
}

#[tokio::test]
async fn a_user_folder_is_only_fetched_never_moved() {
    let remote = remote().await;
    let parent = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let clone = parent.path().join("mine");
    remote.clone_to(&clone).await;
    let before = git(&clone, &["rev-parse", "main"]).await;
    let newest = remote.publish("two").await;
    let ws = workspace(home.path(), &clone);

    let base = worktree::fresh_base(&ws, "main").await;

    assert_eq!(base, "origin/main");
    assert_eq!(git(&clone, &["rev-parse", "origin/main"]).await, newest);
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, before);
    assert!(!clone.join("two").exists(), "working tree untouched");
}

#[tokio::test]
async fn local_commits_the_remote_lacks_keep_the_local_base() {
    let remote = remote().await;
    let parent = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let clone = parent.path().join("mine");
    remote.clone_to(&clone).await;
    let local = commit(&clone, "mine").await;
    remote.publish("two").await;
    let ws = workspace(home.path(), &clone);

    assert_eq!(worktree::fresh_base(&ws, "main").await, "main");
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, local);
}

#[tokio::test]
async fn a_diverged_managed_clone_is_left_alone() {
    let remote = remote().await;
    let home = TempDir::new().unwrap();
    let clone = home.path().join("repos/shop");
    remote.clone_to(&clone).await;
    let local = commit(&clone, "mine").await;
    remote.publish("two").await;
    let ws = workspace(home.path(), &clone);

    assert_eq!(worktree::fresh_base(&ws, "main").await, "main");
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, local);
}

#[tokio::test]
async fn an_unreachable_remote_falls_back_to_the_local_state() {
    let remote = remote().await;
    let home = TempDir::new().unwrap();
    let clone = home.path().join("repos/shop");
    remote.clone_to(&clone).await;
    let before = git(&clone, &["rev-parse", "main"]).await;
    std::fs::remove_dir_all(&remote.bare).unwrap();
    let ws = workspace(home.path(), &clone);

    assert_eq!(worktree::fresh_base(&ws, "main").await, "main");
    assert_eq!(git(&clone, &["rev-parse", "main"]).await, before);
}

#[tokio::test]
async fn a_repo_without_a_remote_keeps_its_base() {
    let fx = common::fixture().await;
    assert_eq!(worktree::fresh_base(&fx.ws, "main").await, "main");
}

#[tokio::test]
async fn a_base_that_is_not_a_local_branch_is_returned_as_given() {
    let remote = remote().await;
    let home = TempDir::new().unwrap();
    let clone = home.path().join("repos/shop");
    remote.clone_to(&clone).await;
    let ws = workspace(home.path(), &clone);

    assert_eq!(
        worktree::fresh_base(&ws, "origin/main").await,
        "origin/main"
    );
}
