use std::path::Path;

use tempfile::TempDir;
use townhall_core::events::EventBus;
use townhall_core::model::AddRepo;
use townhall_core::repos::RepoRegistry;
use townhall_core::store::Store;
use townhall_core::{worktree, Error, Home, Workspace};

mod common;
use common::{clone_with_origin_head, git, init_named};

struct Env {
    dir: TempDir,
    registry: RepoRegistry,
}

fn env() -> Env {
    static NO_GIT_CONFIG: std::sync::Once = std::sync::Once::new();
    NO_GIT_CONFIG.call_once(|| {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    });
    let dir = TempDir::new().unwrap();
    let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
    Env {
        registry: RepoRegistry::new(store),
        dir,
    }
}

fn add(path: &Path) -> AddRepo {
    AddRepo {
        path: path.to_string_lossy().into_owned(),
        ..AddRepo::default()
    }
}

#[tokio::test]
async fn add_defaults_name_and_current_branch() {
    let env = env();
    let repo = init_named(env.dir.path(), "my app", "trunk").await;
    let got = env.registry.add(&add(&repo)).await.unwrap();
    assert_eq!(got.name, "my-app");
    assert_eq!(got.base_branch, "trunk");
    assert_eq!(got.path, repo.canonicalize().unwrap());
    assert_eq!(env.registry.list().await.unwrap(), vec![got.clone()]);
    assert_eq!(env.registry.get("MY-APP").await.unwrap(), got);
}

#[tokio::test]
async fn add_prefers_the_origin_head_branch() {
    let env = env();
    let clone = clone_with_origin_head(env.dir.path()).await;
    assert_eq!(
        env.registry.add(&add(&clone)).await.unwrap().base_branch,
        "develop"
    );
}

#[tokio::test]
async fn origin_head_without_a_local_branch_stays_remote() {
    let env = env();
    let clone = clone_with_origin_head(env.dir.path()).await;
    git(&clone, &["branch", "-q", "-D", "develop"]).await;
    let got = env.registry.add(&add(&clone)).await.unwrap();
    assert_eq!(got.base_branch, "origin/develop");
}

#[tokio::test]
async fn add_checks_a_given_name_and_base_branch() {
    let env = env();
    let repo = init_named(env.dir.path(), "shop", "main").await;
    let named = AddRepo {
        name: Some(" store ".into()),
        base_branch: Some("main".into()),
        ..add(&repo)
    };
    assert_eq!(env.registry.add(&named).await.unwrap().name, "store");
    let other = init_named(env.dir.path(), "other", "main").await;
    let cases = [
        (Some("bad name"), None),
        (Some("-x"), None),
        (Some(""), None),
        (Some("   "), None),
        (Some("ok"), Some("nope")),
        (Some("ok"), Some("-b")),
    ];
    for (name, base) in cases {
        let req = AddRepo {
            name: name.map(String::from),
            base_branch: base.map(String::from),
            ..add(&other)
        };
        let err = env.registry.add(&req).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{name:?}: {err:?}");
    }
}

#[tokio::test]
async fn add_refuses_what_is_not_a_repo() {
    let env = env();
    let file = env.dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    for path in [
        file.to_string_lossy().into_owned(),
        "/definitely/not/here".into(),
        "relative/path".into(),
        "  ".into(),
    ] {
        let req = AddRepo {
            path: path.clone(),
            ..AddRepo::default()
        };
        let err = env.registry.add(&req).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{path}: {err:?}");
    }
    assert!(env.registry.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn names_differing_only_by_case_and_paths_are_unique() {
    let env = env();
    let a = init_named(&env.dir.path().join("x"), "app", "main").await;
    let b = init_named(&env.dir.path().join("y"), "app", "main").await;
    env.registry.add(&add(&a)).await.unwrap();
    assert!(matches!(
        env.registry.add(&add(&b)).await,
        Err(Error::Conflict(_))
    ));
    let upper = AddRepo {
        name: Some("APP".into()),
        ..add(&b)
    };
    let err = env.registry.add(&upper).await.unwrap_err();
    assert_eq!(err.to_string(), "a repo named `APP` already exists");
    let again = AddRepo {
        name: Some("again".into()),
        ..add(&a)
    };
    assert!(matches!(
        env.registry.add(&again).await,
        Err(Error::Conflict(_))
    ));
}

#[tokio::test]
async fn ensure_registers_once_and_avoids_taken_names() {
    let env = env();
    let a = init_named(&env.dir.path().join("x"), "app", "main").await;
    let b = init_named(&env.dir.path().join("y"), "App", "main").await;
    let first = env.registry.ensure(&a).await.unwrap();
    let second = env.registry.ensure(&b).await.unwrap();
    assert_eq!(
        (first.name.as_str(), second.name.as_str()),
        ("app", "App-2")
    );
    assert_eq!(env.registry.ensure(&a).await.unwrap(), first);
    assert_eq!(env.registry.list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn remove_only_unregisters() {
    let env = env();
    let repo = init_named(env.dir.path(), "shop", "main").await;
    git(&repo, &["branch", "townhall/keep"]).await;
    env.registry.add(&add(&repo)).await.unwrap();
    assert_eq!(env.registry.remove("SHOP").await.unwrap().name, "shop");
    assert!(env.registry.list().await.unwrap().is_empty());
    assert!(repo.join(".git").exists());
    assert!(!git(&repo, &["branch", "--list", "townhall/keep"])
        .await
        .is_empty());
    let err = env.registry.remove("shop").await.unwrap_err();
    assert!(
        matches!(err, Error::Missing { what: "repo", .. }),
        "{err:?}"
    );
    assert!(matches!(
        env.registry.get("shop").await,
        Err(Error::Missing { .. })
    ));
}

const NOT_TOP: &str = "is not the top folder of a git repo";

async fn refused(env: &Env, path: &Path) -> String {
    match env.registry.add(&add(path)).await {
        Err(Error::InvalidInput(msg)) => msg,
        other => panic!("{} was not refused: {other:?}", path.display()),
    }
}

#[tokio::test]
async fn only_the_top_folder_of_a_repo_is_a_repo() {
    let env = env();
    let repo = init_named(env.dir.path(), "shop", "main").await;
    let sub = repo.join("src/app");
    std::fs::create_dir_all(&sub).unwrap();
    assert!(refused(&env, &sub).await.contains(NOT_TOP));
    assert!(refused(&env, &repo.join("src")).await.contains(NOT_TOP));
    let plain = env.dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    assert!(
        env.registry.ensure(&plain).await.is_err(),
        "ensure never inits"
    );
    assert!(!plain.join(".git").exists());
    assert!(env.registry.ensure(&sub).await.is_err());
    assert_eq!(env.registry.add(&add(&repo)).await.unwrap().name, "shop");
}

#[tokio::test]
async fn townhall_worktrees_are_never_repos() {
    let env = env();
    let repo = init_named(env.dir.path(), "shop", "main").await;
    let ws = Workspace::new(Home::at(env.dir.path().join(".townhall")), &repo).unwrap();
    let agent = worktree::create(&ws, "fix-login", "main").await.unwrap();
    assert!(refused(&env, &agent.path).await.contains(NOT_TOP));
    assert!(env.registry.ensure(&agent.path).await.is_err());

    let outside = env.dir.path().join("elsewhere");
    let target = outside.to_string_lossy().into_owned();
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "townhall/by-hand",
            &target,
            "main",
        ],
    )
    .await;
    assert!(refused(&env, &outside).await.contains(NOT_TOP));

    let feature = env.dir.path().join("feature");
    let target = feature.to_string_lossy().into_owned();
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "feature", &target, "main"],
    )
    .await;
    let ok = env.registry.add(&add(&feature)).await.unwrap();
    assert_eq!(
        ok.base_branch, "feature",
        "a worktree of your own is a repo"
    );
}

fn plain_folder(env: &Env, name: &str) -> std::path::PathBuf {
    let dir = env.dir.path().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn a_plain_folder_with_files_is_initialised_and_committed() {
    let env = env();
    let dir = plain_folder(&env, "notes");
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/b.txt"), "b").unwrap();
    let got = env.registry.add(&add(&dir)).await.unwrap();
    assert_eq!(
        (got.name.as_str(), got.base_branch.as_str()),
        ("notes", "main")
    );
    let files = git(&dir, &["ls-tree", "-r", "--name-only", "main"]).await;
    assert_eq!(files, "a.txt\nsub/b.txt");
    assert!(git(&dir, &["status", "--porcelain"]).await.is_empty());
    let author = git(&dir, &["log", "-1", "--format=%an <%ae>"]).await;
    assert_eq!(
        author, "townhall <townhall@localhost>",
        "no identity configured"
    );
    let ws = Workspace::new(Home::at(env.dir.path().join(".townhall")), &dir).unwrap();
    worktree::create(&ws, "first-task", "main").await.unwrap();
}

#[tokio::test]
async fn an_empty_folder_gets_an_empty_initial_commit() {
    let env = env();
    let dir = plain_folder(&env, "blank");
    let got = env.registry.add(&add(&dir)).await.unwrap();
    assert_eq!(got.base_branch, "main");
    assert_eq!(git(&dir, &["rev-list", "--count", "main"]).await, "1");
    assert!(git(&dir, &["ls-tree", "-r", "main"]).await.is_empty());
}

#[tokio::test]
async fn a_plain_folder_takes_the_base_branch_it_was_given() {
    let env = env();
    let dir = plain_folder(&env, "trunky");
    let req = AddRepo {
        base_branch: Some(" trunk ".into()),
        ..add(&dir)
    };
    assert_eq!(env.registry.add(&req).await.unwrap().base_branch, "trunk");
    assert_eq!(
        git(&dir, &["symbolic-ref", "--short", "HEAD"]).await,
        "trunk"
    );
}

#[tokio::test]
async fn a_refused_plain_folder_is_left_untouched() {
    let env = env();
    let taken = init_named(&env.dir.path().join("x"), "dup", "main").await;
    env.registry.add(&add(&taken)).await.unwrap();
    let dir = plain_folder(&env, "dup");
    let bad_branch = AddRepo {
        name: Some("free".into()),
        base_branch: Some("-b".into()),
        ..add(&dir)
    };
    let same_name = AddRepo {
        name: Some("DUP".into()),
        ..add(&dir)
    };
    assert!(matches!(
        env.registry.add(&bad_branch).await,
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        env.registry.add(&same_name).await,
        Err(Error::Conflict(_))
    ));
    assert!(!dir.join(".git").exists());
}

#[tokio::test]
async fn a_bare_repo_folder_is_never_initialised() {
    let env = env();
    let bare = plain_folder(&env, "bare.git");
    git(&bare, &["init", "-q", "--bare"]).await;
    assert!(refused(&env, &bare).await.contains(NOT_TOP));
    assert!(!bare.join(".git").exists());
}

#[tokio::test]
async fn the_filesystem_root_is_never_initialised() {
    let env = env();
    let got = env.registry.add(&add(Path::new("/"))).await;
    assert!(matches!(got, Err(Error::InvalidInput(_))), "{got:?}");
}
