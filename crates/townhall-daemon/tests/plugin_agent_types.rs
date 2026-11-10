use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use townhall_core::store::{Marketplace, Plugin, Store};

mod common;
use common::{call, send, Env};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const WRITER: &str = "name = \"writer\"\nadapter = \"claude\"\nmodel = \"haiku\"\n";

async fn install_notes(store: &Store) {
    let root = store
        .home()
        .plugins_dir()
        .unwrap()
        .join("cache/acme/notes")
        .join(SHA);
    std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
    std::fs::create_dir_all(root.join("agent-types")).unwrap();
    let manifest = json!({
        "name": "notes", "version": "1.0.0", "apiVersion": 1,
        "contributes": { "agentTypes": ["agent-types/writer.toml"] },
    });
    std::fs::write(
        root.join(".townhall-plugin/plugin.json"),
        manifest.to_string(),
    )
    .unwrap();
    std::fs::write(root.join("agent-types/writer.toml"), WRITER).unwrap();
    let market = Marketplace {
        name: "acme".into(),
        source: json!({"source": "github", "repo": "acme/market"}),
        account: None,
        enabled: true,
        official: false,
        head_sha: None,
        fetched_at: None,
        error: None,
    };
    store.add_marketplace(market).await.unwrap();
    let plugin = Plugin {
        id: String::new(),
        marketplace: "acme".into(),
        name: "notes".into(),
        version: Some("1.0.0".into()),
        sha: SHA.into(),
        source: json!({"source": "path", "path": "./"}),
        enabled: true,
        pinned: false,
        permissions_hash: "sha256:aa".into(),
        approved: json!([]),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(plugin).await.unwrap();
}

#[tokio::test]
async fn a_plugin_type_is_listed_and_read_but_put_and_delete_are_refused() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_notes(state.ctx().store().unwrap()).await;

    let (s, v) = call(&app, "GET", "/api/library/agents", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let names: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"notes-writer"), "{names:?}");

    let (s, v) = call(&app, "GET", "/api/library/agents/notes-writer", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["origin"], "plugin");
    assert_eq!(v["data"]["plugin"], "notes@acme");

    let body = "name = \"notes-writer\"\nadapter = \"claude\"\n";
    let req = Request::builder()
        .method("PUT")
        .uri("/api/library/agents/notes-writer")
        .header("content-type", "application/toml")
        .body(Body::from(body))
        .unwrap();
    let (s, v) = send(&app, req).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("owned by plugin notes@acme"),
        "{v}"
    );

    let (s, v) = call(&app, "DELETE", "/api/library/agents/notes-writer", None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("owned by plugin notes@acme"),
        "{v}"
    );
}
