#![allow(dead_code)]

use serde_json::json;
use townhall_core::store::{Marketplace, Plugin, Store};

pub const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

pub const WRITER: &str = "name = \"writer\"\ndescription = \"writes release notes\"\nadapter = \"claude\"\nmodel = \"haiku\"\npermission_mode = \"plan\"\nplugins = [\"notes@acme\"]\nprompt = \"You write release notes.\"\n";

pub fn type_file(name: &str, extra: &str) -> String {
    format!("name = \"{name}\"\nadapter = \"claude\"\n{extra}")
}

pub async fn install(
    store: &Store,
    name: &str,
    market: &str,
    types: &[(&str, String)],
    enabled: bool,
) -> Plugin {
    let root = store
        .home()
        .plugins_dir()
        .unwrap()
        .join("cache")
        .join(market)
        .join(name)
        .join(SHA);
    std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
    std::fs::create_dir_all(root.join("agent-types")).unwrap();
    let listed: Vec<String> = types
        .iter()
        .map(|(file, _)| format!("agent-types/{file}.toml"))
        .collect();
    let manifest = json!({
        "name": name,
        "version": "1.0.0",
        "apiVersion": 1,
        "contributes": { "agentTypes": listed },
    });
    std::fs::write(
        root.join(".townhall-plugin/plugin.json"),
        manifest.to_string(),
    )
    .unwrap();
    for (file, text) in types {
        std::fs::write(root.join(format!("agent-types/{file}.toml")), text).unwrap();
    }
    if store.marketplace(market).await.is_err() {
        let row = Marketplace {
            name: market.into(),
            source: json!({"source": "github", "repo": "acme/market"}),
            account: None,
            enabled: true,
            official: false,
            head_sha: None,
            fetched_at: None,
            error: None,
        };
        store.add_marketplace(row).await.unwrap();
    }
    let row = Plugin {
        id: String::new(),
        marketplace: market.into(),
        name: name.into(),
        version: Some("1.0.0".into()),
        sha: SHA.into(),
        source: json!({"source": "path", "path": "./"}),
        enabled,
        pinned: false,
        permissions_hash: "sha256:aa".into(),
        approved: json!([]),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(row).await.unwrap()
}
