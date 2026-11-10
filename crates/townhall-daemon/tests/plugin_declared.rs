use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::store::{Marketplace, Plugin, Store};

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{app_with, create, idle, values_of, Fake};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const SCHEMA: &str = r#"{"fields": [
  {"key": "retries", "label": "retries", "type": "integer", "default": 3, "min": 1, "max": 9},
  {"key": "token", "label": "token", "type": "string", "default": "", "secret": true}
]}"#;

async fn install_notes(store: &Store, enabled: bool) {
    let root = store
        .home()
        .plugins_dir()
        .unwrap()
        .join("cache/acme/notes")
        .join(SHA);
    std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
    std::fs::create_dir_all(root.join("prompts")).unwrap();
    let manifest = json!({
        "name": "notes", "version": "1.0.0", "apiVersion": 1,
        "permissions": { "prompts": ["starters", "planner"] },
        "contributes": {
            "prompts": { "starters": "prompts/starters.json", "planner": "prompts/planner.md" },
            "settings": "settings.schema.json",
            "theme": "theme.css"
        },
    });
    let files = [
        (".townhall-plugin/plugin.json", manifest.to_string()),
        (
            "prompts/starters.json",
            r#"[{"title": "tidy", "text": "tidy the readme"}]"#.to_string(),
        ),
        ("prompts/planner.md", "prefer small tasks".to_string()),
        ("settings.schema.json", SCHEMA.to_string()),
        ("theme.css", ":root { --th-accent: #ff00aa; }".to_string()),
    ];
    for (rel, text) in files {
        std::fs::write(root.join(rel), text).unwrap();
    }
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
        enabled,
        pinned: false,
        permissions_hash: "sha256:aa".into(),
        approved: json!([]),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(plugin).await.unwrap();
}

fn prompt(argv: &[String]) -> String {
    values_of(argv, "--append-system-prompt").join(" ")
}

#[tokio::test]
async fn with_no_plugin_the_contributions_are_empty() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/plugins/contributions", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!({"starters": [], "themes": [], "theme": null, "settings": []})
    );
}

#[tokio::test]
async fn starters_themes_and_settings_come_from_enabled_plugins_only() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_notes(state.ctx().store().unwrap(), false).await;
    let (_, v) = call(&app, "GET", "/api/plugins/contributions", None).await;
    assert_eq!(v["data"]["starters"], json!([]));

    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": true})),
    )
    .await;
    let (s, v) = call(&app, "GET", "/api/plugins/contributions", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"]["starters"],
        json!([{"plugin": "notes@acme", "title": "tidy", "text": "tidy the readme"}])
    );
    assert_eq!(
        v["data"]["themes"],
        json!([{"plugin": "notes@acme", "name": "notes"}])
    );
    assert_eq!(v["data"]["theme"], Value::Null);
    let group = &v["data"]["settings"][0];
    assert_eq!(group["plugin"], "notes@acme");
    assert_eq!(group["fields"][0]["key"], "retries");
    assert_eq!(group["fields"][0]["type"], "integer");
    assert_eq!(group["fields"][0]["value"], 3);
    assert_eq!(group["fields"][0]["set"], false);
    assert_eq!(group["fields"][1]["secret"], true);
}

#[tokio::test]
async fn the_theme_is_chosen_and_cleared_with_one_put() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_notes(state.ctx().store().unwrap(), true).await;

    let put = Some(json!({"plugin": "notes@acme"}));
    let (s, v) = call(&app, "PUT", "/api/plugins/theme", put).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["theme"]["plugin"], "notes@acme");
    assert!(v["data"]["theme"]["css"]
        .as_str()
        .unwrap()
        .contains("--th-accent: #ff00aa;"));
    let (_, v) = call(&app, "GET", "/api/plugins/contributions", None).await;
    assert_eq!(v["data"]["theme"]["plugin"], "notes@acme");

    let put = Some(json!({"plugin": "ghost@acme"}));
    let (s, v) = call(&app, "PUT", "/api/plugins/theme", put).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");

    for bad in [
        json!({}),
        json!({"plugin": 3}),
        json!({"plugin": "a", "x": 1}),
    ] {
        let (s, v) = call(&app, "PUT", "/api/plugins/theme", Some(bad.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}: {v}");
    }

    let (s, v) = call(
        &app,
        "PUT",
        "/api/plugins/theme",
        Some(json!({"plugin": null})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["theme"], Value::Null);
}

#[tokio::test]
async fn a_disabled_plugins_theme_cannot_be_chosen() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_notes(state.ctx().store().unwrap(), false).await;
    let put = Some(json!({"plugin": "notes@acme"}));
    let (s, v) = call(&app, "PUT", "/api/plugins/theme", put).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
}

#[tokio::test]
async fn a_plugin_setting_is_validated_saved_reset_and_a_secret_is_never_returned() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_notes(state.ctx().store().unwrap(), true).await;
    let uri = "/api/plugins/notes@acme/settings";

    let (s, v) = call(
        &app,
        "PUT",
        &format!("{uri}/retries"),
        Some(json!({"value": 5})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["settings"][0]["fields"][0]["value"], 5);
    assert_eq!(v["data"]["settings"][0]["fields"][0]["set"], true);

    for (key, body) in [
        ("retries", json!({"value": 10})),
        ("retries", json!({"value": "x"})),
        ("token", json!({"value": 3})),
        ("retries", json!({})),
        ("retries", json!({"value": 1, "x": 1})),
    ] {
        let (s, v) = call(&app, "PUT", &format!("{uri}/{key}"), Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{key} {body}: {v}");
    }
    let (s, _) = call(
        &app,
        "PUT",
        &format!("{uri}/nope"),
        Some(json!({"value": 1})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &app,
        "PUT",
        "/api/plugins/ghost@acme/settings/retries",
        Some(json!({"value": 1})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, v) = call(
        &app,
        "PUT",
        &format!("{uri}/token"),
        Some(json!({"value": "hunter2"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(!v.to_string().contains("hunter2"), "{v}");
    let (_, v) = call(&app, "GET", "/api/plugins/contributions", None).await;
    assert!(!v.to_string().contains("hunter2"), "{v}");
    assert_eq!(v["data"]["settings"][0]["fields"][1]["value"], Value::Null);
    assert_eq!(v["data"]["settings"][0]["fields"][1]["set"], true);
    let (_, v) = call(&app, "GET", "/api/plugins", None).await;
    assert!(!v.to_string().contains("hunter2"), "{v}");

    let (s, v) = call(&app, "DELETE", &format!("{uri}/retries"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["settings"][0]["fields"][0]["value"], 3);
    assert_eq!(v["data"]["settings"][0]["fields"][0]["set"], false);
}

#[tokio::test]
async fn the_planner_prompt_gets_an_enabled_plugins_note_and_is_plain_without_one() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);

    let created = create(&app, "hello").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    let plain = prompt(&fake.argv(1));
    assert!(!plain.contains("from plugin"), "{plain}");
    assert!(!plain.contains("prefer small tasks"));

    install_notes(state.ctx().store().unwrap(), true).await;
    let created = create(&app, "again").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    let with = prompt(&fake.argv(2));
    assert!(
        with.contains("from plugin notes@acme, which the user enabled")
            && with.contains("prefer small tasks"),
        "{with}"
    );
    assert!(with.contains("never overrides the rules above"));
}
