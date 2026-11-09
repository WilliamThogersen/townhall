use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde_json::json;
use tempfile::TempDir;
use townhall_core::events::EventBus;
use townhall_core::model::{AddRepo, SettingKey};
use townhall_core::repos::publish::Programs;
use townhall_core::repos::RepoRegistry;
use townhall_core::store::Store;
use townhall_core::Home;

mod common;
use common::{git, init_named};

struct World {
    root: TempDir,
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let root = TempDir::new().unwrap();
        let glab = root.path().join("glab-config");
        std::fs::create_dir_all(&glab).unwrap();
        std::fs::write(
            glab.join("config.yml"),
            "hosts:\n  code.test:\n    user: dev\n",
        )
        .unwrap();
        let gh = root.path().join("gh-config");
        std::fs::create_dir_all(&gh).unwrap();
        let served = root.path().join("served");
        std::env::set_var("GLAB_CONFIG_DIR", &glab);
        std::env::set_var("GH_CONFIG_DIR", &gh);
        let prefixes = [
            "https://gitlab.test/",
            "git@gitlab.test:",
            "https://code.test/",
        ];
        std::env::set_var("GIT_CONFIG_COUNT", prefixes.len().to_string());
        for (n, prefix) in prefixes.into_iter().enumerate() {
            std::env::set_var(
                format!("GIT_CONFIG_KEY_{n}"),
                format!("url.file://{}/.insteadOf", served.display()),
            );
            std::env::set_var(format!("GIT_CONFIG_VALUE_{n}"), prefix);
        }
        World { root }
    })
}

fn served_dir() -> PathBuf {
    world().root.path().join("served")
}

async fn serve(path: &str) {
    let parent = world().root.path().join("src");
    let name = path.replace('/', "-");
    let src = init_named(&parent, &name, "main").await;
    let bare = served_dir().join(format!("{path}.git"));
    std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
    let (from, to) = (src.to_string_lossy(), bare.to_string_lossy());
    git(&parent, &["clone", "-q", "--bare", &from, &to]).await;
}

struct Env {
    dir: TempDir,
    store: Store,
}

fn env() -> Env {
    world();
    let dir = TempDir::new().unwrap();
    let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
    Env { dir, store }
}

impl Env {
    fn stub(&self, name: &str, script: &str) -> String {
        let path = self.dir.path().join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn registry(&self, glab_sees: bool) -> RepoRegistry {
        let api = if glab_sees {
            "echo '{}'; exit 0"
        } else {
            "echo '404 Not Found' >&2; exit 1"
        };
        let glab = format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n\"config get\") echo \"glpat-$5\"; exit 0;;\n\"api projects/\"*) {api};;\nesac\nexit 0\n"
        );
        let gh = "#!/bin/sh\nexit 1\n";
        let programs = Programs {
            gh: self.stub("gh", gh),
            glab: self.stub("glab", &glab),
        };
        RepoRegistry::new(self.store.clone()).with_programs(programs)
    }

    async fn register(&self, accounts: serde_json::Value) {
        self.store
            .set_setting(SettingKey::ForgeAccounts, accounts)
            .await
            .unwrap();
    }
}

fn add(url: &str) -> AddRepo {
    AddRepo {
        path: url.to_string(),
        ..AddRepo::default()
    }
}

fn base(env: &Env) -> Option<String> {
    Some(env.dir.path().join("dev").to_string_lossy().into_owned())
}

async fn accounts_of(env: &Env) -> Vec<String> {
    let settings = env.store.settings().await.unwrap();
    settings
        .forge_accounts
        .iter()
        .map(|a| format!("{}:{}@{}", a.forge.as_str(), a.login, a.host))
        .collect()
}

#[tokio::test]
async fn a_registered_account_that_can_see_the_repo_is_stored_on_the_repo() {
    let env = env();
    serve("team/sub/app").await;
    env.register(json!([
        {"forge": "gitlab", "login": "alice", "host": "gitlab.test"}
    ]))
    .await;
    let req = AddRepo {
        clone_dir: base(&env),
        ..add("git@gitlab.test:team/sub/app.git")
    };
    let repo = env.registry(true).add(&req).await.unwrap();
    assert_eq!(repo.gitlab_account.as_deref(), Some("alice"));
    assert_eq!(repo.github_account, None);
}

#[tokio::test]
async fn a_detected_login_is_registered_then_stored_on_the_repo() {
    let env = env();
    serve("g/app").await;
    let req = AddRepo {
        clone_dir: base(&env),
        ..add("https://code.test/g/app")
    };
    let repo = env.registry(true).add(&req).await.unwrap();
    assert_eq!(repo.gitlab_account.as_deref(), Some("dev"));
    assert_eq!(accounts_of(&env).await, vec!["gitlab:dev@code.test"]);
}

#[tokio::test]
async fn when_no_account_can_see_the_repo_it_is_added_unpicked() {
    let env = env();
    serve("team/open").await;
    env.register(json!([
        {"forge": "gitlab", "login": "alice", "host": "gitlab.test"}
    ]))
    .await;
    let req = AddRepo {
        clone_dir: base(&env),
        ..add("https://gitlab.test/team/open")
    };
    let repo = env.registry(false).add(&req).await.unwrap();
    assert_eq!(repo.gitlab_account, None);
    assert_eq!(accounts_of(&env).await, vec!["gitlab:alice@gitlab.test"]);
}

#[tokio::test]
async fn a_failed_clone_names_the_accounts_that_were_tried() {
    let env = env();
    env.register(json!([
        {"forge": "gitlab", "login": "alice", "host": "gitlab.test"},
        {"forge": "gitlab", "login": "other", "host": "gitlab.test"}
    ]))
    .await;
    let req = AddRepo {
        clone_dir: base(&env),
        ..add("https://gitlab.test/team/missing")
    };
    let err = env.registry(false).add(&req).await.unwrap_err().to_string();
    assert!(err.contains("could not clone"), "{err}");
    assert!(err.contains("no account could see it"), "{err}");
    assert!(err.contains("alice") && err.contains("other"), "{err}");
    assert!(!err.contains("glpat"), "{err}");
}

#[tokio::test]
async fn an_unknown_forge_is_cloned_as_before_and_left_unpicked() {
    let env = env();
    let req = AddRepo {
        clone_dir: base(&env),
        ..add("https://elsewhere.invalid/g/plain")
    };
    let err = env.registry(true).add(&req).await.unwrap_err().to_string();
    assert!(err.contains("could not clone"), "{err}");
    assert!(!err.contains("no account"), "{err}");
}

#[tokio::test]
async fn a_file_url_never_asks_a_forge_cli() {
    let env = env();
    serve("g/local").await;
    let url = format!("file://{}/g/local.git", served_dir().display());
    let req = AddRepo {
        clone_dir: base(&env),
        ..add(&url)
    };
    let repo = env.registry(true).add(&req).await.unwrap();
    assert_eq!((repo.github_account, repo.gitlab_account), (None, None));
}
