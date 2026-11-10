use serde_json::json;
use townhall_core::definitions::PermissionMode;
use townhall_core::model::Origin;
use townhall_core::Error;

mod common;
use common::store_env;
mod plugin_support;
use plugin_support::{install, type_file, WRITER};

async fn names(store: &townhall_core::store::Store) -> Vec<String> {
    store
        .agent_types()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.def.name)
        .collect()
}

#[tokio::test]
async fn an_enabled_plugins_type_shows_up_read_only_as_plugin_dash_name() {
    let env = store_env();
    install(
        &env.store,
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        true,
    )
    .await;
    assert!(names(&env.store)
        .await
        .contains(&"notes-writer".to_string()));
    let found = env.store.agent_type("Notes-Writer").await.unwrap().unwrap();
    assert_eq!(found.def.name, "notes-writer");
    assert_eq!(found.def.permission_mode, Some(PermissionMode::Plan));
    assert_eq!(found.def.plugins, ["notes@acme"]);
    assert_eq!(found.origin, Origin::Plugin);
    assert_eq!(found.plugin.as_deref(), Some("notes@acme"));
    assert_eq!(found.source, WRITER);
    let wire = serde_json::to_value(&found).unwrap();
    assert_eq!(wire["origin"], json!("plugin"));
    assert_eq!(wire["plugin"], json!("notes@acme"));
    assert_eq!(wire["plugins"], json!(["notes@acme"]));
}

#[tokio::test]
async fn saved_types_are_user_types_with_no_plugins() {
    let env = store_env();
    let builder = env.store.agent_type("builder").await.unwrap().unwrap();
    assert_eq!(builder.origin, Origin::User);
    assert_eq!(builder.plugin, None);
    assert!(builder.def.plugins.is_empty());
    let wire = serde_json::to_value(&builder).unwrap();
    assert_eq!(wire["origin"], json!("user"));
    assert_eq!(wire["plugin"], json!(null));
}

#[tokio::test]
async fn a_disabled_plugin_contributes_no_types() {
    let env = store_env();
    let plugin = install(
        &env.store,
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        false,
    )
    .await;
    assert!(!names(&env.store)
        .await
        .contains(&"notes-writer".to_string()));
    assert!(env
        .store
        .agent_type("notes-writer")
        .await
        .unwrap()
        .is_none());
    env.store
        .update_plugin(townhall_core::store::Plugin {
            enabled: true,
            ..plugin
        })
        .await
        .unwrap();
    assert!(env
        .store
        .agent_type("notes-writer")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn a_plugin_type_named_default_or_a_builtin_name_is_skipped() {
    let env = store_env();
    let types = [
        ("default", type_file("default", "")),
        ("builder", type_file("builder", "")),
        ("ok", type_file("ok", "")),
    ];
    install(&env.store, "notes", "acme", &types, true).await;
    let all = names(&env.store).await;
    assert!(all.contains(&"notes-ok".to_string()), "{all:?}");
    assert!(!all.contains(&"notes-default".to_string()), "{all:?}");
    assert!(!all.contains(&"notes-builder".to_string()), "{all:?}");
    assert_eq!(all.iter().filter(|n| *n == "default").count(), 1);
}

#[tokio::test]
async fn a_plugin_type_with_bypass_permissions_is_refused() {
    let env = store_env();
    let skip = type_file("skip", "permission_mode = \"bypassPermissions\"\n");
    let types = [("skip", skip), ("ok", type_file("ok", ""))];
    install(&env.store, "notes", "acme", &types, true).await;
    let all = names(&env.store).await;
    assert!(!all.contains(&"notes-skip".to_string()), "{all:?}");
    assert!(all.contains(&"notes-ok".to_string()), "{all:?}");
}

#[tokio::test]
async fn a_broken_plugin_type_is_left_out_without_hiding_the_rest() {
    let env = store_env();
    let types = [
        (
            "broken",
            "name = \"broken\"\nadapter = \"nope\"\n".to_string(),
        ),
        ("ok", type_file("ok", "")),
    ];
    install(&env.store, "notes", "acme", &types, true).await;
    let all = names(&env.store).await;
    assert!(all.contains(&"notes-ok".to_string()) && !all.contains(&"notes-broken".to_string()));
}

#[tokio::test]
async fn a_user_type_of_the_same_name_wins() {
    let env = store_env();
    env.store
        .save_agent_type(&type_file("notes-writer", "model = \"opus\"\n"))
        .await
        .unwrap();
    install(
        &env.store,
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        true,
    )
    .await;
    let all = names(&env.store).await;
    assert_eq!(all.iter().filter(|n| *n == "notes-writer").count(), 1);
    let found = env.store.agent_type("notes-writer").await.unwrap().unwrap();
    assert_eq!(found.origin, Origin::User);
    assert_eq!(found.def.model.as_deref(), Some("opus"));
}

#[tokio::test]
async fn saving_or_deleting_a_plugin_type_is_refused_naming_the_plugin() {
    let env = store_env();
    install(
        &env.store,
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        true,
    )
    .await;
    let save = env
        .store
        .save_agent_type(&type_file("notes-writer", ""))
        .await
        .unwrap_err();
    assert!(
        matches!(&save, Error::Conflict(m) if m.contains("owned by plugin notes@acme")),
        "{save}"
    );
    let delete = env
        .store
        .delete_agent_type("notes-writer")
        .await
        .unwrap_err();
    assert!(
        matches!(&delete, Error::Conflict(m) if m.contains("owned by plugin notes@acme")),
        "{delete}"
    );
    assert!(env
        .store
        .agent_type("notes-writer")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn the_plugins_key_parses_and_older_types_still_load() {
    let env = store_env();
    let source = type_file("uses", "plugins = [\"notes@acme\"]\n");
    let (saved, _) = env.store.save_agent_type(&source).await.unwrap();
    assert_eq!(saved.def.plugins, ["notes@acme"]);
    let again = env.store.agent_type("uses").await.unwrap().unwrap();
    assert_eq!(again, saved);
    assert!(env
        .store
        .agent_type("builder")
        .await
        .unwrap()
        .unwrap()
        .def
        .plugins
        .is_empty());
}
