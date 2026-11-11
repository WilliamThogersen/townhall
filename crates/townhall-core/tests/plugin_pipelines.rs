use serde_json::json;
use townhall_core::model::Origin;
use townhall_core::Error;

mod common;
use common::store_env;
mod plugin_support;
use plugin_support::install_files;

const SHIP: &str = "name = \"ship\"\ndescription = \"plan then build\"\nwork = \"build\"\n\n[[step]]\nid = \"plan\"\nagent = \"builder\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\nafter = [\"plan\"]\n";

fn manifest(files: &[&str]) -> serde_json::Value {
    json!({
        "name": "flows",
        "version": "1.0.0",
        "apiVersion": 1,
        "contributes": { "pipelines": files },
    })
}

async fn names(store: &townhall_core::store::Store) -> Vec<String> {
    store
        .workflows()
        .await
        .unwrap()
        .into_iter()
        .map(|w| w.def.name)
        .collect()
}

async fn install_ship(store: &townhall_core::store::Store, enabled: bool) {
    install_files(
        store,
        "flows",
        "acme",
        manifest(&["pipelines/ship.toml"]),
        &[("pipelines/ship.toml", SHIP)],
        enabled,
    )
    .await;
}

#[tokio::test]
async fn an_enabled_plugins_pipeline_shows_up_read_only_as_plugin_dash_name() {
    let env = store_env();
    install_ship(&env.store, true).await;
    assert_eq!(names(&env.store).await, ["flows-ship"]);
    let found = env.store.workflow("Flows-Ship").await.unwrap().unwrap();
    assert_eq!(found.def.name, "flows-ship");
    assert_eq!(found.origin, Origin::Plugin);
    assert_eq!(found.plugin.as_deref(), Some("flows@acme"));
    assert_eq!(found.source, SHIP);
    let wire = serde_json::to_value(&found).unwrap();
    assert_eq!(wire["origin"], json!("plugin"));
    assert_eq!(wire["plugin"], json!("flows@acme"));
}

#[tokio::test]
async fn saved_pipelines_are_user_pipelines() {
    let env = store_env();
    let (saved, _) = env.store.save_workflow(SHIP).await.unwrap();
    assert_eq!(saved.origin, Origin::User);
    assert_eq!(saved.plugin, None);
    let wire = serde_json::to_value(&saved).unwrap();
    assert_eq!(wire["origin"], json!("user"));
    assert_eq!(wire["plugin"], json!(null));
}

#[tokio::test]
async fn a_disabled_plugin_contributes_no_pipelines() {
    let env = store_env();
    install_ship(&env.store, false).await;
    assert!(names(&env.store).await.is_empty());
    assert!(env.store.workflow("flows-ship").await.unwrap().is_none());
}

#[tokio::test]
async fn a_plugin_pipeline_cannot_be_overwritten_or_deleted() {
    let env = store_env();
    install_ship(&env.store, true).await;
    let edited = SHIP.replace("name = \"ship\"", "name = \"flows-ship\"");
    let saved = env.store.save_workflow(&edited).await;
    assert!(matches!(saved, Err(Error::Conflict(_))), "{saved:?}");
    let deleted = env.store.delete_workflow("flows-ship").await;
    assert!(matches!(deleted, Err(Error::Conflict(_))), "{deleted:?}");
}

#[tokio::test]
async fn a_broken_or_default_plugin_pipeline_is_left_out_without_hiding_the_rest() {
    let env = store_env();
    let ghost = SHIP.replace("agent = \"builder\"", "agent = \"ghost\"");
    let default = SHIP.replace("work =", "default = true\nwork =");
    install_files(
        &env.store,
        "flows",
        "acme",
        manifest(&[
            "pipelines/ghost.toml",
            "pipelines/default.toml",
            "pipelines/ship.toml",
        ]),
        &[
            ("pipelines/ghost.toml", &ghost),
            ("pipelines/default.toml", &default),
            ("pipelines/ship.toml", SHIP),
        ],
        true,
    )
    .await;
    assert_eq!(names(&env.store).await, ["flows-ship"]);
}
