use serde_json::json;
use townhall_core::plugins::install::{PatchPlugin, PluginRegistry};
use townhall_core::plugins::{storage, tokens};
use townhall_core::store::{Marketplace, Plugin, Store};
use townhall_core::Error;

mod common;
use common::store_env;

const ALL: [&str; 6] = [
    "api:tasks:read",
    "api:tasks:create",
    "api:conversations:read",
    "api:settings:read",
    "api:storage",
    "hook:task.ended",
];

async fn add(store: &Store, name: &str, enabled: bool, approved: &[&str]) -> Plugin {
    if store.marketplace("acme").await.is_err() {
        store
            .add_marketplace(Marketplace {
                name: "acme".into(),
                source: json!({"source": "github", "repo": "acme/market"}),
                account: None,
                enabled: true,
                official: false,
                head_sha: None,
                fetched_at: None,
                error: None,
            })
            .await
            .unwrap();
    }
    store
        .add_plugin(Plugin {
            id: String::new(),
            marketplace: "acme".into(),
            name: name.into(),
            version: Some("1.0.0".into()),
            sha: "deadbeef".into(),
            source: json!({"source": "github", "repo": "acme/x"}),
            enabled,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved: json!(approved),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        })
        .await
        .unwrap()
}

fn patch(enabled: bool) -> PatchPlugin {
    PatchPlugin {
        enabled: Some(enabled),
        pinned: None,
    }
}

#[tokio::test]
async fn a_token_is_32_random_bytes_in_hex_and_never_repeats() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let one = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    let two = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    assert_eq!(one.len(), 64, "{one}");
    assert!(one.bytes().all(|b| b.is_ascii_hexdigit()), "{one}");
    assert_ne!(one, two);
}

#[tokio::test]
async fn only_the_hash_of_a_token_is_stored() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    let rows = env.store.plugin_tokens("lint@acme").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].token_hash, tokens::hash(&clear));
    assert_ne!(rows[0].token_hash, clear);
    assert_eq!(rows[0].task, None);
    assert_eq!(rows[0].revoked_at, None);
    let home = env.store.home().root().to_path_buf();
    for entry in std::fs::read_dir(home).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with("townhall.db") {
            let bytes = std::fs::read(&path).unwrap();
            let found = bytes
                .windows(clear.len())
                .any(|window| window == clear.as_bytes());
            assert!(!found, "the clear token is in {name}");
        }
    }
}

#[tokio::test]
async fn a_known_token_names_its_plugin_and_the_scopes_it_was_approved() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    let caller = tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(caller.plugin, "lint@acme");
    assert_eq!(caller.task, None);
    assert_eq!(
        caller.scopes,
        [
            "tasks:read",
            "tasks:create",
            "conversations:read",
            "settings:read",
            "storage"
        ]
    );
}

#[tokio::test]
async fn an_unknown_token_is_nobody() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    assert!(tokens::authenticate(&env.store, "nope")
        .await
        .unwrap()
        .is_none());
    let made_up = "ab".repeat(32);
    assert!(tokens::authenticate(&env.store, &made_up)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_revoked_token_is_nobody() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    assert!(tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .is_some());
    tokens::revoke_plugin(&env.store, "lint@acme")
        .await
        .unwrap();
    assert!(tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .is_none());
    let rows = env.store.plugin_tokens("lint@acme").await.unwrap();
    assert!(rows[0].revoked_at.is_some());
}

#[tokio::test]
async fn the_token_of_a_disabled_plugin_is_nobody() {
    let env = store_env();
    add(&env.store, "lint", false, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    assert!(tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn a_task_token_is_bound_to_its_task_and_has_the_read_scopes_and_storage_at_most() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", Some("one"))
        .await
        .unwrap();
    let caller = tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(caller.task.as_deref(), Some("one"));
    assert_eq!(
        caller.scopes,
        ["tasks:read", "conversations:read", "storage"]
    );
    let rows = env.store.plugin_tokens("lint@acme").await.unwrap();
    assert_eq!(rows[0].task.as_deref(), Some("one"));
}

#[tokio::test]
async fn revoking_a_task_revokes_only_its_task_tokens() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let one = tokens::issue(&env.store, "lint@acme", Some("one"))
        .await
        .unwrap();
    let two = tokens::issue(&env.store, "lint@acme", Some("two"))
        .await
        .unwrap();
    let own = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    tokens::revoke_task(&env.store, "ONE").await.unwrap();
    assert!(tokens::authenticate(&env.store, &one)
        .await
        .unwrap()
        .is_none());
    assert!(tokens::authenticate(&env.store, &two)
        .await
        .unwrap()
        .is_some());
    assert!(tokens::authenticate(&env.store, &own)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn enabling_creates_the_plugin_token_and_disabling_revokes_every_token() {
    let env = store_env();
    add(&env.store, "lint", false, &ALL).await;
    let registry = PluginRegistry::new(env.store.clone());
    assert!(env
        .store
        .plugin_tokens("lint@acme")
        .await
        .unwrap()
        .is_empty());

    registry.patch("lint@acme", &patch(true)).await.unwrap();
    let rows = env.store.plugin_tokens("lint@acme").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].task, None);
    assert_eq!(rows[0].revoked_at, None);

    registry.patch("lint@acme", &patch(true)).await.unwrap();
    assert_eq!(env.store.plugin_tokens("lint@acme").await.unwrap().len(), 1);

    let task = tokens::issue(&env.store, "lint@acme", Some("one"))
        .await
        .unwrap();
    registry.patch("lint@acme", &patch(false)).await.unwrap();
    let rows = env.store.plugin_tokens("lint@acme").await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.revoked_at.is_some()));
    assert!(tokens::authenticate(&env.store, &task)
        .await
        .unwrap()
        .is_none());

    registry.patch("lint@acme", &patch(true)).await.unwrap();
    let live: Vec<_> = env
        .store
        .plugin_tokens("lint@acme")
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.revoked_at.is_none())
        .collect();
    assert_eq!(live.len(), 1);
}

#[tokio::test]
async fn uninstalling_revokes_the_tokens_and_purging_clears_the_storage() {
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    add(&env.store, "fmt", true, &ALL).await;
    let clear = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    storage::put(&env.store, "lint@acme", "k", "v")
        .await
        .unwrap();
    storage::put(&env.store, "fmt@acme", "k", "w")
        .await
        .unwrap();
    let registry = PluginRegistry::new(env.store.clone());

    registry.uninstall("lint@acme", false).await.unwrap();
    assert!(tokens::authenticate(&env.store, &clear)
        .await
        .unwrap()
        .is_none());
    assert!(env
        .store
        .plugin_tokens("lint@acme")
        .await
        .unwrap()
        .iter()
        .all(|r| r.revoked_at.is_some()));
    assert_eq!(
        storage::get(&env.store, "lint@acme", "k").await.unwrap(),
        "v"
    );

    add(&env.store, "lint", true, &ALL).await;
    registry.uninstall("lint@acme", true).await.unwrap();
    let err = storage::get(&env.store, "lint@acme", "k")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err}");
    assert_eq!(
        storage::get(&env.store, "fmt@acme", "k").await.unwrap(),
        "w"
    );
}

async fn task_with_token(env: &common::StoreEnv, name: &str) -> String {
    use townhall_core::model::NewTask;
    let conversation = env.store.create_conversation().await.unwrap().id;
    env.store
        .create_task(
            &conversation,
            NewTask {
                name: name.into(),
                title: name.into(),
                prompt: "p".into(),
                repo: "shop".into(),
                base_branch: "main".into(),
                after: vec![],
                attachments: vec![],
                agent: "builder".into(),
                pr_base: None,
                done: Default::default(),
            },
        )
        .await
        .unwrap();
    tokens::issue(&env.store, "lint@acme", Some(name))
        .await
        .unwrap()
}

async fn move_to(env: &common::StoreEnv, name: &str, state: townhall_core::model::TaskState) {
    env.store
        .update_task(name, move |t| {
            Ok(townhall_core::model::Task { state, ..t })
        })
        .await
        .unwrap();
}

async fn live(env: &common::StoreEnv, token: &str) -> bool {
    tokens::authenticate(&env.store, token)
        .await
        .unwrap()
        .is_some()
}

#[tokio::test]
async fn a_task_token_dies_when_the_task_ends_is_merged_or_fails_and_not_before() {
    use townhall_core::model::TaskState;
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    for (name, last) in [
        ("a-ended", TaskState::Ended),
        ("a-merged", TaskState::Merged),
        ("a-error", TaskState::Error),
    ] {
        let token = task_with_token(&env, name).await;
        for middle in [TaskState::Working, TaskState::NeedsYou] {
            move_to(&env, name, middle).await;
            assert!(live(&env, &token).await, "{name} {middle:?}");
        }
        move_to(&env, name, last).await;
        assert!(!live(&env, &token).await, "{name} {last:?}");
    }
}

#[tokio::test]
async fn a_final_state_leaves_other_tasks_and_the_plugin_token_alone() {
    use townhall_core::model::TaskState;
    let env = store_env();
    add(&env.store, "lint", true, &ALL).await;
    let one = task_with_token(&env, "one").await;
    let two = task_with_token(&env, "two").await;
    let own = tokens::issue(&env.store, "lint@acme", None).await.unwrap();
    move_to(&env, "one", TaskState::Ended).await;
    assert!(!live(&env, &one).await);
    assert!(live(&env, &two).await);
    assert!(live(&env, &own).await);
}

#[tokio::test]
async fn storage_round_trips_per_plugin() {
    let env = store_env();
    storage::put(&env.store, "lint@acme", "k", "one")
        .await
        .unwrap();
    storage::put(&env.store, "fmt@acme", "k", "other")
        .await
        .unwrap();
    assert_eq!(
        storage::get(&env.store, "lint@acme", "k").await.unwrap(),
        "one"
    );
    storage::put(&env.store, "lint@acme", "k", "two")
        .await
        .unwrap();
    assert_eq!(
        storage::get(&env.store, "LINT@ACME", "k").await.unwrap(),
        "two"
    );
    assert_eq!(
        storage::get(&env.store, "fmt@acme", "k").await.unwrap(),
        "other"
    );
    storage::delete(&env.store, "lint@acme", "k").await.unwrap();
    let err = storage::get(&env.store, "lint@acme", "k")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err}");
    let err = storage::delete(&env.store, "lint@acme", "k")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Missing { .. }), "{err}");
}

#[tokio::test]
async fn a_value_may_be_64_kb_and_not_a_byte_more() {
    let env = store_env();
    let fits = "a".repeat(64 * 1024);
    storage::put(&env.store, "lint@acme", "big", &fits)
        .await
        .unwrap();
    let over = "a".repeat(64 * 1024 + 1);
    let err = storage::put(&env.store, "lint@acme", "big", &over)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");
    assert_eq!(
        storage::get(&env.store, "lint@acme", "big").await.unwrap(),
        fits
    );
}

#[tokio::test]
async fn a_plugin_may_store_5_mb_in_all_and_not_more() {
    let env = store_env();
    let chunk = "a".repeat(64 * 1024);
    for i in 0..80 {
        storage::put(&env.store, "lint@acme", &format!("k{i}"), &chunk)
            .await
            .unwrap();
    }
    let err = storage::put(&env.store, "lint@acme", "one-more", "x")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");
    storage::put(&env.store, "lint@acme", "k0", "small")
        .await
        .unwrap();
    storage::put(&env.store, "lint@acme", "one-more", "x")
        .await
        .unwrap();
    storage::put(&env.store, "fmt@acme", "k", &chunk)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_storage_key_is_a_short_plain_name() {
    let env = store_env();
    for bad in ["", "a/b", "a b", "..", &"k".repeat(129), "ü"] {
        let err = storage::put(&env.store, "lint@acme", bad, "v")
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{bad}: {err}");
    }
    for good in ["a", "a.b_c-D9", &"k".repeat(128)] {
        storage::put(&env.store, "lint@acme", good, "v")
            .await
            .unwrap();
    }
}
