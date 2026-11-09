use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;
use townhall_core::events::EventBus;
use townhall_core::model::{AddRepo, SettingKey};
use townhall_core::repos::RepoRegistry;
use townhall_core::store::Store;
use townhall_core::{Error, Home};

mod common;
use common::{git, init_named};

struct Env {
    dir: TempDir,
    store: Store,
    registry: RepoRegistry,
}

fn env() -> Env {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
    Env {
        registry: RepoRegistry::new(store.clone()),
        store,
        dir,
    }
}

impl Env {
    fn repos_dir(&self) -> PathBuf {
        self.dir.path().join(".townhall/repos")
    }
}

fn add(url: &str) -> AddRepo {
    AddRepo {
        path: url.to_string(),
        ..AddRepo::default()
    }
}

/// A bare repo called `<name>.git` under `parent` whose default branch is `branch`;
/// returns its `file://` url.
async fn origin(parent: &Path, name: &str, branch: &str) -> String {
    let src = init_named(parent, &format!("{name}-src"), branch).await;
    let bare = parent.join(format!("{name}.git"));
    let (from, to) = (src.to_string_lossy(), bare.to_string_lossy());
    git(parent, &["clone", "-q", "--bare", &from, &to]).await;
    format!("file://{}", bare.display())
}

#[tokio::test]
async fn a_remote_is_cloned_into_the_managed_folder_and_registered() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "develop").await;
    let got = env.registry.add(&add(&url)).await.unwrap();
    assert_eq!(got.name, "shop");
    assert_eq!(got.base_branch, "develop");
    let expected = env.repos_dir().join("shop").canonicalize().unwrap();
    assert_eq!(got.path, expected);
    assert!(expected.join(".git").exists());
    assert_eq!(env.registry.list().await.unwrap(), vec![got]);
}

#[tokio::test]
async fn trailing_slashes_and_a_given_name_and_base_branch_work() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "develop").await;
    let req = AddRepo {
        path: format!("  {url}/ "),
        name: Some("store".into()),
        base_branch: Some("develop".into()),
        ..AddRepo::default()
    };
    let got = env.registry.add(&req).await.unwrap();
    assert_eq!(got.name, "store");
    assert!(got.path.ends_with("repos/shop"), "{:?}", got.path);
}

#[tokio::test]
async fn a_clone_dir_in_the_request_picks_the_base_folder() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let base = env.dir.path().join("dev");
    let req = AddRepo {
        clone_dir: Some(base.to_string_lossy().into_owned()),
        ..add(&url)
    };
    let got = env.registry.add(&req).await.unwrap();
    assert_eq!(got.path, base.join("shop").canonicalize().unwrap());
    assert!(!env.repos_dir().join("shop").exists());
}

#[tokio::test]
async fn the_clones_dir_setting_is_the_default_base_and_the_request_overrides_it() {
    let env = env();
    let (dev, other) = (env.dir.path().join("dev"), env.dir.path().join("other"));
    let value = json!(dev.to_string_lossy());
    env.store.set_setting(SettingKey::ClonesDir, value).await.unwrap();
    let a = origin(&env.dir.path().join("a"), "shop", "main").await;
    let b = origin(&env.dir.path().join("b"), "cart", "main").await;
    let got = env.registry.add(&add(&a)).await.unwrap();
    assert_eq!(got.path, dev.join("shop").canonicalize().unwrap());
    let req = AddRepo {
        clone_dir: Some(other.to_string_lossy().into_owned()),
        ..add(&b)
    };
    let got = env.registry.add(&req).await.unwrap();
    assert_eq!(got.path, other.join("cart").canonicalize().unwrap());
}

#[tokio::test]
async fn a_chosen_base_folder_never_overwrites_or_renames_around_an_existing_folder() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let base = env.dir.path().join("dev");
    std::fs::create_dir_all(base.join("shop")).unwrap();
    std::fs::write(base.join("shop/keep.txt"), "mine").unwrap();
    let req = AddRepo {
        clone_dir: Some(base.to_string_lossy().into_owned()),
        ..add(&url)
    };
    let err = env.registry.add(&req).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
    assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
    assert!(base.join("shop/keep.txt").exists());
}

#[tokio::test]
async fn a_relative_clone_dir_is_refused() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let req = AddRepo {
        clone_dir: Some("dev/projects".into()),
        ..add(&url)
    };
    let err = env.registry.add(&req).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[tokio::test]
async fn an_existing_clone_is_reused_not_cloned_again() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let first = env.registry.add(&add(&url)).await.unwrap();
    std::fs::write(first.path.join("local-only.txt"), "mine").unwrap();
    let again = env.registry.add(&add(&url)).await.unwrap_err();
    assert!(matches!(again, Error::Conflict(_)), "{again:?}");
    env.registry.remove("shop").await.unwrap();
    let second = env.registry.add(&add(&url)).await.unwrap();
    assert_eq!(second.path, first.path);
    assert!(second.path.join("local-only.txt").exists());
    assert_eq!(std::fs::read_dir(env.repos_dir()).unwrap().count(), 1);
}

#[tokio::test]
async fn two_remotes_with_one_name_get_two_folders_and_two_names() {
    let env = env();
    let a = origin(&env.dir.path().join("a"), "shop", "main").await;
    let b = origin(&env.dir.path().join("b"), "shop", "main").await;
    let first = env.registry.add(&add(&a)).await.unwrap();
    let second = env.registry.add(&add(&b)).await.unwrap();
    assert_eq!(
        (first.name.as_str(), second.name.as_str()),
        ("shop", "shop-2")
    );
    assert_ne!(first.path, second.path);
    assert!(second.path.ends_with("repos/shop-2"), "{:?}", second.path);
}

#[tokio::test]
async fn a_given_name_that_is_taken_is_refused_before_cloning() {
    let env = env();
    let a = origin(&env.dir.path().join("a"), "shop", "main").await;
    let b = origin(&env.dir.path().join("b"), "other", "main").await;
    env.registry.add(&add(&a)).await.unwrap();
    let req = AddRepo {
        name: Some("SHOP".into()),
        ..add(&b)
    };
    let err = env.registry.add(&req).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
    assert!(!env.repos_dir().join("other").exists());
}

#[tokio::test]
async fn a_failed_clone_says_why_and_leaves_nothing_behind() {
    let env = env();
    let missing = format!("file://{}/nope.git", env.dir.path().display());
    let err = env.registry.add(&add(&missing)).await.unwrap_err();
    let Error::InvalidInput(msg) = &err else {
        panic!("{err:?}")
    };
    assert!(msg.starts_with("could not clone "), "{msg}");
    assert!(msg.contains("repository not found"), "{msg}");
    assert!(!env.repos_dir().join("nope").exists());
    assert!(env.registry.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn an_unreachable_https_remote_fails_fast_instead_of_asking_for_credentials() {
    let env = env();
    let started = std::time::Instant::now();
    let err = env
        .registry
        .add(&add("https://127.0.0.1:9/acme/shop.git"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert!(started.elapsed().as_secs() < 30);
}

#[tokio::test]
async fn a_clean_managed_clone_can_be_deleted_with_its_registration() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let repo = env.registry.add(&add(&url)).await.unwrap();
    let gone = env.registry.remove_with_clone("shop").await.unwrap();
    assert_eq!(gone, repo);
    assert!(!repo.path.exists());
    assert!(env.registry.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_user_folder_is_never_deleted() {
    let env = env();
    let mine = init_named(&env.dir.path().join("mine"), "shop", "main").await;
    env.registry
        .add(&add(&mine.to_string_lossy()))
        .await
        .unwrap();
    let err = env.registry.remove_with_clone("shop").await.unwrap_err();
    let Error::InvalidInput(msg) = &err else {
        panic!("{err:?}")
    };
    assert!(msg.contains("not a clone townhall made"), "{msg}");
    assert!(mine.exists());
    assert_eq!(env.registry.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_clone_with_changes_or_unpushed_commits_is_refused_and_kept() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let repo = env.registry.add(&add(&url)).await.unwrap();
    std::fs::write(repo.path.join("wip.txt"), "x").unwrap();
    let err = env.registry.remove_with_clone("shop").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
    assert!(err.to_string().contains("uncommitted"), "{err}");
    git(&repo.path, &["add", "-A"]).await;
    git(
        &repo.path,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "wip",
        ],
    )
    .await;
    let err = env.registry.remove_with_clone("shop").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
    assert!(err.to_string().contains("not pushed"), "{err}");
    assert!(repo.path.exists());
    assert_eq!(env.registry.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_bad_base_branch_is_refused_but_the_clone_stays_for_next_time() {
    let env = env();
    let url = origin(&env.dir.path().join("a"), "shop", "main").await;
    let req = AddRepo {
        base_branch: Some("nope".into()),
        ..add(&url)
    };
    let err = env.registry.add(&req).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert!(env.registry.list().await.unwrap().is_empty());
    assert!(env.registry.add(&add(&url)).await.is_ok());
}
