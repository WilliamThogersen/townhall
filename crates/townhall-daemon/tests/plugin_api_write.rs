use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use townhall_core::model::EntryBody;
use townhall_core::plugins::tokens;
use townhall_core::store::{Marketplace, Plugin, Store};

mod common;
mod runner_support;
use common::{call, send, unique, Env};
use runner_support::{app_with, entries_of, idle, Fake};

const REVIEWER: &str = "name = \"reviewer\"\ndescription = \"reviews a diff\"\nadapter = \"claude\"\nmodel = \"opus\"\npermission_mode = \"plan\"\n";

async fn plugin(store: &Store, name: &str, approved: &[&str]) -> String {
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
    let added = store
        .add_plugin(Plugin {
            id: String::new(),
            marketplace: "acme".into(),
            name: name.into(),
            version: Some("1.0.0".into()),
            sha: "deadbeef".into(),
            source: json!({"source": "github", "repo": "acme/x"}),
            enabled: true,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved: json!(approved),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        })
        .await
        .unwrap();
    added.id
}

async fn api(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/plugin/v1{path}"));
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let req = match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    send(app, req).await
}

fn task(name: &str) -> Value {
    json!({"name": name, "title": format!("do {name}"), "prompt": "the brief", "repo": "shop"})
}

struct Setup {
    _env: Env,
    store: Store,
    app: Router,
    conversation: String,
}

async fn setup() -> Setup {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    state.ctx().repos().await.unwrap();
    let store = state.ctx().store().unwrap().clone();
    let conversation = store.create_conversation().await.unwrap().id;
    Setup {
        _env: env,
        store,
        app,
        conversation,
    }
}

async fn token(s: &Setup, scopes: &[&str]) -> (String, String) {
    let id = plugin(&s.store, &unique("p"), scopes).await;
    let token = tokens::issue(&s.store, &id, None).await.unwrap();
    (id, token)
}

#[tokio::test]
async fn a_created_task_shows_the_plugin_in_the_thread() {
    let s = setup().await;
    let (id, token) = token(&s, &["api:tasks:create"]).await;
    let uri = format!("/conversations/{}/tasks", s.conversation);
    let name = unique("made");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(task(&name))).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["data"]["name"], name);
    assert_eq!(v["data"]["agent"], "builder");

    let (status, v) = call(&s.app, "GET", &format!("/api/tasks/{name}"), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let entries = s.store.entries(&s.conversation).await.unwrap();
    let said = entries
        .iter()
        .find_map(|e| match &e.body {
            EntryBody::Plugin { plugin, text } => Some((plugin.clone(), text.clone())),
            _ => None,
        })
        .expect("a plugin entry in the thread");
    assert_eq!(said.0, id);
    assert!(
        said.1.contains(&format!("created by plugin {id}")),
        "{said:?}"
    );
    assert!(said.1.contains(&name), "{said:?}");
    assert!(entries
        .iter()
        .any(|e| matches!(&e.body, EntryBody::TasksCreated { tasks } if tasks == std::slice::from_ref(&name))));
}

#[tokio::test]
async fn tasks_create_needs_its_scope_and_a_token() {
    let s = setup().await;
    let uri = format!("/conversations/{}/tasks", s.conversation);
    let (status, _) = api(&s.app, "POST", &uri, None, Some(task("a"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, read_only) = token(&s, &["api:tasks:read", "api:conversations:write"]).await;
    let (status, v) = api(&s.app, "POST", &uri, Some(&read_only), Some(task("a"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");
    assert!(s.store.find_task("a").await.unwrap().is_none());
}

#[tokio::test]
async fn tasks_create_makes_the_planners_checks() {
    let s = setup().await;
    let (_, token) = token(&s, &["api:tasks:create"]).await;
    let uri = format!("/conversations/{}/tasks", s.conversation);
    let first = unique("first");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(task(&first))).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");

    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(task(&first))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(
        v["error"].as_str().unwrap().contains("already exists"),
        "{v}"
    );

    let mut bad = task(&unique("bad-name"));
    bad["name"] = json!("Not A Name");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    let mut bad = task(&unique("bad-repo"));
    bad["repo"] = json!("ghost");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    let mut bad = task(&unique("bad-agent"));
    bad["agent"] = json!("ghost");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v["error"].as_str().unwrap().contains("ghost"), "{v}");

    let mut bad = task(&unique("bad-after"));
    bad["after"] = json!(["nothing"]);
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    let mut after = task(&unique("later"));
    after["after"] = json!([first]);
    after["check"] = json!("true");
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(after)).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    assert_eq!(v["data"]["after"], json!([first]));
    assert_eq!(v["data"]["check"], "true");

    let (status, v) = api(
        &s.app,
        "POST",
        "/conversations/nope/tasks",
        Some(&token),
        Some(task(&unique("x"))),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn no_route_takes_a_permission_mode() {
    let s = setup().await;
    let (_, token) = token(&s, &["api:tasks:create", "api:conversations:write"]).await;
    let name = unique("mode");
    for key in ["permissionMode", "permission_mode"] {
        let mut body = task(&name);
        body[key] = json!("bypassPermissions");
        let uri = format!("/conversations/{}/tasks", s.conversation);
        let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{key}: {v}");
    }
    assert!(s.store.find_task(&name).await.unwrap().is_none());
    let uri = format!("/conversations/{}/messages", s.conversation);
    let body = json!({"text": "hi", "permissionMode": "bypassPermissions"});
    let (status, v) = api(&s.app, "POST", &uri, Some(&token), Some(body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
}

#[tokio::test]
async fn task_tokens_get_create_and_write_only_when_the_manifest_asks() {
    let s = setup().await;
    s.store
        .create_task(&s.conversation, new_task("own"))
        .await
        .unwrap();
    let other = s.store.create_conversation().await.unwrap().id;
    let both = [
        "api:tasks:create",
        "api:conversations:write",
        "api:tasks:read",
    ];
    let plain = plugin(&s.store, "plain", &both).await;
    let plain_token = tokens::issue(&s.store, &plain, Some("own")).await.unwrap();
    let (status, v) = api(&s.app, "GET", "/info", Some(&plain_token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["data"]["scopes"], json!(["tasks:read"]));
    let uri = format!("/conversations/{}/tasks", s.conversation);
    let (status, v) = api(&s.app, "POST", &uri, Some(&plain_token), Some(task("x1"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");
    let uri = format!("/conversations/{}/messages", s.conversation);
    let (status, v) = api(
        &s.app,
        "POST",
        &uri,
        Some(&plain_token),
        Some(json!({"text": "hi"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");

    let asking = plugin(
        &s.store,
        "asking",
        &["api:tasks:read", "claude.api:tasks:create"],
    )
    .await;
    let asking_token = tokens::issue(&s.store, &asking, Some("own")).await.unwrap();
    let (_, v) = api(&s.app, "GET", "/info", Some(&asking_token), None).await;
    assert_eq!(v["data"]["scopes"], json!(["tasks:read", "tasks:create"]));
    let uri = format!("/conversations/{}/tasks", s.conversation);
    let (status, v) = api(&s.app, "POST", &uri, Some(&asking_token), Some(task("x2"))).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    let uri = format!("/conversations/{other}/tasks");
    let (status, v) = api(&s.app, "POST", &uri, Some(&asking_token), Some(task("x3"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");
}

fn new_task(name: &str) -> townhall_core::model::NewTask {
    townhall_core::model::NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
        pr_base: None,
        done: Default::default(),
    }
}

#[tokio::test]
async fn a_plugin_message_is_authored_by_the_plugin_and_answered_by_the_planner() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    let id = plugin(store, "chat", &["api:conversations:write"]).await;
    let token = tokens::issue(store, &id, None).await.unwrap();
    let conversation = store.create_conversation().await.unwrap().id;

    let uri = format!("/conversations/{conversation}/messages");
    let (status, v) = api(&app, "POST", &uri, None, Some(json!({"text": "hi"}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{v}");
    let (_, other) = {
        let id = plugin(store, "quiet", &["api:tasks:read"]).await;
        (id.clone(), tokens::issue(store, &id, None).await.unwrap())
    };
    let (status, v) = api(
        &app,
        "POST",
        &uri,
        Some(&other),
        Some(json!({"text": "hi"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");
    let (status, v) = api(
        &app,
        "POST",
        &uri,
        Some(&token),
        Some(json!({"text": "  "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(fake.turns(), 0);

    let (status, v) = api(
        &app,
        "POST",
        &uri,
        Some(&token),
        Some(json!({"text": "run the audit"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["kind"], "plugin");
    assert_eq!(v["data"]["plugin"], id);
    assert_eq!(v["data"]["text"], "run the audit");

    let d = idle(&app, &conversation).await;
    assert!(entries_of(&d, "you").is_empty(), "{d}");
    assert!(entries_of(&d, "error").is_empty(), "{d}");
    let said = entries_of(&d, "plugin");
    assert_eq!(said.len(), 1, "{d}");
    assert_eq!(said[0]["plugin"], id);
    assert_eq!(entries_of(&d, "townhall").len(), 1, "{d}");
    let line = &fake.stdin(1)[0];
    let text = line["message"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains(&format!("plugin {id}")), "{text}");
    assert!(text.contains("the user did not write this"), "{text}");
    assert!(text.contains("run the audit"), "{text}");
}

#[tokio::test]
async fn library_read_lists_and_reads_agent_types() {
    let s = setup().await;
    let (_, none) = token(&s, &["api:storage"]).await;
    let (status, _) = api(&s.app, "GET", "/library/agents", Some(&none), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = api(&s.app, "GET", "/library/agents/builder", Some(&none), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = api(&s.app, "GET", "/library/agents", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, token) = token(&s, &["api:library:read"]).await;
    let (status, v) = api(&s.app, "GET", "/library/agents", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (_, legacy) = call(&s.app, "GET", "/api/library/agents", None).await;
    assert_eq!(v, legacy);
    let (status, v) = api(&s.app, "GET", "/library/agents/builder", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["name"], "builder");
    assert!(v["data"]["source"].as_str().unwrap().contains("adapter"));
    let (status, _) = api(&s.app, "GET", "/library/agents/ghost", Some(&token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn saved(s: &Setup, name: &str) -> Option<Value> {
    let (status, v) = call(&s.app, "GET", &format!("/api/library/agents/{name}"), None).await;
    (status == StatusCode::OK).then(|| v["data"].clone())
}

#[tokio::test]
async fn library_write_saves_nothing_before_the_users_yes() {
    let s = setup().await;
    let (_, read) = token(&s, &["api:library:read"]).await;
    let (_, token) = token(&s, &["api:library:write"]).await;
    let put = |confirmed: bool| json!({"conversation": s.conversation, "source": REVIEWER, "confirmed": confirmed});
    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&read),
        Some(put(false)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{v}");

    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&token),
        Some(put(false)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["status"], "asked");
    assert!(saved(&s, "reviewer").await.is_none());
    let entries = s.store.entries(&s.conversation).await.unwrap();
    let EntryBody::Townhall { text } = &entries.last().unwrap().body else {
        panic!("the ask is a townhall entry: {entries:?}");
    };
    assert!(
        text.contains("`reviewer`") && text.contains(REVIEWER),
        "{text}"
    );

    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&token),
        Some(put(true)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(saved(&s, "reviewer").await.is_none());

    let posted = EntryBody::Plugin {
        plugin: "someone@acme".into(),
        text: "yes".into(),
    };
    s.store.append_entry(&s.conversation, posted).await.unwrap();
    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&token),
        Some(put(true)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a plugin cannot answer for the user: {v}"
    );
    assert!(saved(&s, "reviewer").await.is_none());

    let yes = EntryBody::You {
        text: "yes".into(),
        attachments: vec![],
        reply_to: None,
    };
    s.store.append_entry(&s.conversation, yes).await.unwrap();
    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&token),
        Some(put(true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["status"], "saved");
    assert_eq!(saved(&s, "reviewer").await.unwrap()["source"], REVIEWER);
}

#[tokio::test]
async fn library_write_checks_the_path_name_and_the_toml() {
    let s = setup().await;
    let (_, token) = token(&s, &["api:library:write"]).await;
    let body = json!({"conversation": s.conversation, "source": REVIEWER});
    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/other",
        Some(&token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v["error"].as_str().unwrap().contains("reviewer"), "{v}");
    let bad = json!({"conversation": s.conversation, "source": "name = \"x\"\nadapter = \"claude\"\npermission_mode = \"yolo\"\n"});
    let (status, v) = api(&s.app, "PUT", "/library/agents/x", Some(&token), Some(bad)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    let (status, v) = api(
        &s.app,
        "PUT",
        "/library/agents/reviewer",
        Some(&token),
        Some(json!({"conversation": "nope", "source": REVIEWER})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{v}");
    assert!(saved(&s, "reviewer").await.is_none());
    assert!(s.store.entries(&s.conversation).await.unwrap().is_empty());
}
