use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};

mod common;
use common::{call, send, Env};

const CODER: &str = "# my coder\nname = \"coder\"\ndescription = \"writes code\"\nadapter = \"claude\"\nmodel = \"opus\"\npermission_mode = \"plan\"\ntools = [\"Read\", \"Edit\"]\nprompt = \"be brief\"\n";

async fn toml(
    app: &Router,
    method: &str,
    uri: &str,
    kind: &str,
    body: &str,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", kind)
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

async fn put(app: &Router, name: &str, body: &str) -> (StatusCode, Value) {
    let uri = format!("/api/library/agents/{name}");
    toml(app, "PUT", &uri, "application/toml", body).await
}

#[tokio::test]
async fn the_list_starts_with_builder() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/library/agents", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let rows = v["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let keys: Vec<&String> = rows[0].as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "adapter",
            "description",
            "model",
            "name",
            "permissionMode",
            "updatedAt"
        ]
    );
    assert_eq!(
        (
            &rows[0]["name"],
            &rows[0]["adapter"],
            &rows[0]["model"],
            &rows[0]["permissionMode"]
        ),
        (
            &json!("builder"),
            &json!("claude"),
            &json!("sonnet"),
            &json!("acceptEdits")
        )
    );
}

#[tokio::test]
async fn put_saves_toml_and_get_reads_it_back() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = put(&app, "coder", CODER).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["data"]["source"], CODER);
    assert_eq!(v["data"]["tools"], json!(["Read", "Edit"]));
    let plain = CODER.replace("opus", "haiku");
    let uri = "/api/library/agents/coder";
    let (s, v) = toml(&app, "PUT", uri, "text/plain; charset=utf-8", &plain).await;
    assert_eq!(
        (s, &v["data"]["model"]),
        (StatusCode::OK, &json!("haiku")),
        "{v}"
    );

    let (s, v) = call(&app, "GET", "/api/library/agents/CODER", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["source"], plain.as_str());
    assert_eq!(v["data"]["permissionMode"], "plan");
    assert_eq!(v["data"]["prompt"], "be brief");
    assert!(v["data"]["updatedAt"].is_string());
    let (s, _) = call(&app, "GET", "/api/library/agents/nope", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, v) = call(&app, "GET", "/api/library/agents", None).await;
    let names: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["builder", "coder"]);
}

#[tokio::test]
async fn a_bad_put_answers_the_error_list() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "PUT", "/api/library/agents/coder", Some(json!({}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        v["error"],
        "expected content-type text/plain or application/toml"
    );

    let bad = "name = \"x\"\nadapter = \"claude\"\nmodel = \"-p\"\ncommand = \"ls\"\n";
    let (s, v) = put(&app, "x", bad).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["success"], false);
    let errors = v["data"].as_array().unwrap();
    let fields: Vec<&str> = errors
        .iter()
        .map(|e| e["field"].as_str().unwrap())
        .collect();
    assert_eq!(fields, ["model", "command"]);
    assert_eq!(errors[0]["kind"], "agent");
    let text = v["error"].as_str().unwrap();
    assert!(
        text.starts_with("agent \"x\": model") && text.contains("; "),
        "{text}"
    );

    let (s, v) = put(&app, "x", "name = \"x\"\nadapter = \"claude\"\nmood = 1\n").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["data"][0]["line"].is_number(), "{v}");

    let (s, v) = put(&app, "other", CODER).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["data"][0]["field"], "name");
    assert!(v["error"].as_str().unwrap().contains("other"), "{v}");
    let (_, v) = call(&app, "GET", "/api/library/agents", None).await;
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn delete_refuses_unknown_and_the_last_type() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, _) = call(&app, "DELETE", "/api/library/agents/ghost", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    put(&app, "coder", CODER).await;
    let (s, v) = call(&app, "DELETE", "/api/library/agents/Builder", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!("builder")));
    let (s, v) = call(&app, "DELETE", "/api/library/agents/coder", None).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert!(v["error"].as_str().unwrap().contains("last"), "{v}");
}

#[tokio::test]
async fn import_takes_one_new_agent_toml() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let uri = "/api/library/import";
    let (s, v) = toml(&app, "POST", uri, "application/toml", CODER).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(
        (&v["data"]["name"], &v["data"]["source"]),
        (&json!("coder"), &json!(CODER))
    );
    let (s, v) = toml(&app, "POST", uri, "text/plain", CODER).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");

    let (s, v) = toml(&app, "POST", uri, "text/plain", "name = \"y\"\n").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(v["data"].is_array(), "{v}");
    let workflow = "name = \"ship\"\n\n[[step]]\nid = \"a\"\nagent = \"coder\"\n";
    let (s, _) = toml(&app, "POST", uri, "text/plain", workflow).await;
    assert_eq!(s, StatusCode::NOT_IMPLEMENTED);

    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "text/plain")
        .header("origin", "https://evil.example")
        .body(Body::from(CODER.replace("coder", "sneaky")))
        .unwrap();
    let (s, _) = send(&app, req).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "GET", "/api/library/agents/sneaky", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn usage_sums_runs_and_tokens_per_agent_type() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let c = store.create_conversation().await.unwrap().id;
    for name in ["one", "two"] {
        let task = townhall_core::model::NewTask {
            name: name.into(),
            title: format!("do {name}"),
            prompt: "the brief".into(),
            repo: "alpha".into(),
            base_branch: "main".into(),
            after: vec![],
            attachments: vec![],
            agent: "builder".into(),
            pr_base: None,
        };
        store.create_task(&c, task).await.unwrap();
    }
    store.set_task_tokens("one", 1200).await.unwrap();
    store.set_task_tokens("two", 300).await.unwrap();
    let (s, v) = call(&app, "GET", "/api/library/agents/usage", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!([{"agent": "builder", "runs": 2, "tokens": 1500}])
    );
}
