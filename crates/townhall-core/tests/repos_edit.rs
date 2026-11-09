use std::path::PathBuf;

use serde_json::json;
use townhall_core::model::{PatchRepo, Repo, SettingKey};
use townhall_core::repos::RepoRegistry;
use townhall_core::Error;

mod common;
use common::{git, init_named, new_task, store_env, StoreEnv};

struct Env {
    s: StoreEnv,
    registry: RepoRegistry,
}

fn env() -> Env {
    let s = store_env();
    Env {
        registry: RepoRegistry::new(s.store.clone()),
        s,
    }
}

impl Env {
    async fn add(&self, name: &str, branch: &str) -> Repo {
        let dir = init_named(&self.s.dir.path().join("src"), name, branch).await;
        let req = townhall_core::model::AddRepo {
            path: dir.to_string_lossy().into_owned(),
            ..Default::default()
        };
        self.registry.add(&req).await.unwrap()
    }
}

fn patch() -> PatchRepo {
    PatchRepo::default()
}

#[tokio::test]
async fn a_rename_keeps_the_path_and_accounts_and_follows_into_tasks_and_conversations() {
    let env = env();
    let shop = env.add("shop", "main").await;
    let accounts = json!([{"forge": "github", "login": "work-me"}]);
    env.s
        .store
        .set_setting(SettingKey::ForgeAccounts, accounts)
        .await
        .unwrap();
    let put = townhall_core::model::PutRepoAccounts {
        github_account: Some(Some("work-me".into())),
        ..Default::default()
    };
    env.s.store.set_repo_accounts("shop", put).await.unwrap();
    let conv = env.s.store.create_conversation().await.unwrap();
    env.s
        .store
        .update_conversation(&conv.id, |c| {
            Ok(townhall_core::model::Conversation {
                linked_repo: Some("shop".into()),
                ..c
            })
        })
        .await
        .unwrap();
    env.s
        .store
        .create_task(&conv.id, new_task("fix-cart"))
        .await
        .unwrap();

    let rename = PatchRepo {
        name: Some("store".into()),
        ..patch()
    };
    let got = env.registry.edit("SHOP", &rename).await.unwrap();
    assert_eq!(got.name, "store");
    assert_eq!(got.path, shop.path);
    assert_eq!(got.github_account.as_deref(), Some("work-me"));
    assert_eq!(env.registry.list().await.unwrap(), vec![got]);
    let task = env.s.store.find_task("fix-cart").await.unwrap().unwrap();
    assert_eq!(task.repo, "store");
    let conv = env.s.store.conversation(&conv.id).await.unwrap();
    assert_eq!(conv.linked_repo.as_deref(), Some("store"));
}

#[tokio::test]
async fn a_name_taken_by_another_repo_is_a_conflict_and_changes_nothing() {
    let env = env();
    env.add("shop", "main").await;
    env.add("cart", "main").await;
    let rename = PatchRepo {
        name: Some("CART".into()),
        base_branch: Some("main".into()),
        ..patch()
    };
    let err = env.registry.edit("shop", &rename).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err:?}");
    let names: Vec<String> = env
        .registry
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, ["shop", "cart"]);
}

#[tokio::test]
async fn an_unknown_repo_is_missing() {
    let env = env();
    let rename = PatchRepo {
        name: Some("x".into()),
        ..patch()
    };
    let err = env.registry.edit("ghost", &rename).await.unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err:?}");
}

#[tokio::test]
async fn the_base_branch_changes_to_an_existing_branch_only() {
    let env = env();
    let shop = env.add("shop", "main").await;
    git(&shop.path, &["branch", "develop"]).await;
    let to = |b: &str| PatchRepo {
        base_branch: Some(b.into()),
        ..patch()
    };
    let got = env.registry.edit("shop", &to("develop")).await.unwrap();
    assert_eq!(got.base_branch, "develop");
    let err = env.registry.edit("shop", &to("nope")).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    assert_eq!(
        env.registry.get("shop").await.unwrap().base_branch,
        "develop"
    );
}

#[tokio::test]
async fn moving_the_folder_updates_the_registered_path() {
    let env = env();
    let shop = env.add("shop", "main").await;
    let target: PathBuf = env.s.dir.path().join("moved");
    let mv = PatchRepo {
        path: Some(target.to_string_lossy().into_owned()),
        move_folder: true,
        ..patch()
    };
    let got = env.registry.edit("shop", &mv).await.unwrap();
    assert_eq!(got.path, target.canonicalize().unwrap());
    assert!(!shop.path.exists());
    assert_eq!(env.registry.get("shop").await.unwrap().path, got.path);
}

#[tokio::test]
async fn a_refused_path_edit_changes_neither_the_name_nor_the_registry() {
    let env = env();
    let shop = env.add("shop", "main").await;
    let stranger = init_named(&env.s.dir.path().join("other"), "stranger", "main").await;
    let amend = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "--amend",
        "--allow-empty",
        "-q",
        "-m",
        "a different history",
    ];
    git(&stranger, &amend).await;
    let bad = PatchRepo {
        name: Some("renamed".into()),
        path: Some(stranger.to_string_lossy().into_owned()),
        ..patch()
    };
    assert!(env.registry.edit("shop", &bad).await.is_err());
    assert_eq!(env.registry.list().await.unwrap(), vec![shop]);
}
