use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use townhall_daemon::start_background;

mod common;
use common::{call, send, Env};

#[tokio::test]
async fn the_startup_repo_is_listed_and_registered() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!([{"name": "alpha", "path": env.startup, "baseBranch": "main"}])
    );
    let stored = state.ctx().store().unwrap().repos().await.unwrap();
    assert_eq!(stored[0].name, "alpha");
}

#[tokio::test]
async fn start_background_registers_the_startup_repo() {
    let env = Env::new("alpha").await;
    let state = env.state();
    let background = start_background(&state).await;
    assert!(background.running() >= 1, "the task scheduler (2.3) runs");
    background.stop();
    let stored = state.ctx().store().unwrap().repos().await.unwrap();
    assert_eq!(stored[0].path, env.startup);
}

#[tokio::test]
async fn add_and_remove_a_repo() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    let (s, v) = call(&app, "POST", "/api/repos", Some(json!({"path": beta}))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(
        v["data"],
        json!({"name": "beta", "path": beta, "baseBranch": "main"})
    );
    let (_, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(v["data"].as_array().unwrap().len(), 2);

    let (s, v) = call(&app, "DELETE", "/api/repos/beta", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["name"], "beta");
    assert!(beta.join(".git").exists(), "the folder stays");
    let (_, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
    let (s, v) = call(&app, "DELETE", "/api/repos/beta", None).await;
    assert_eq!(
        (s, v["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("repo `beta` not found"))
    );
}

#[tokio::test]
async fn add_with_name_and_base_branch() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    let body = json!({"path": beta, "name": "store", "baseBranch": "main"});
    let (s, v) = call(&app, "POST", "/api/repos", Some(body)).await;
    assert_eq!(
        (s, &v["data"]["name"]),
        (StatusCode::CREATED, &json!("store"))
    );
    let bad = json!({"path": env.repo("gamma").await, "baseBranch": "nope"});
    let (s, v) = call(&app, "POST", "/api/repos", Some(bad)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert!(v["error"]
        .as_str()
        .unwrap()
        .contains("base branch `nope` not found"));
}

#[tokio::test]
async fn case_only_and_path_conflicts_are_409_blank_names_400() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(
        &app,
        "POST",
        "/api/repos",
        Some(json!({"path": env.startup})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    let other = env.repo("other").await;
    let (s, v) = call(
        &app,
        "POST",
        "/api/repos",
        Some(json!({"path": other, "name": "ALPHA"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"], "a repo named `ALPHA` already exists");
    let (s, v) = call(
        &app,
        "POST",
        "/api/repos",
        Some(json!({"path": other, "name": "  "})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"], "invalid input: repo name must not be blank");
    let (s, v) = call(&app, "DELETE", "/api/repos/alpha", None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(v["error"]
        .as_str()
        .unwrap()
        .contains("the repo the daemon started with"));
}

#[tokio::test]
async fn bad_bodies_are_400_in_the_envelope() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let plain = env.root.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    for body in [
        json!({"path": plain}),
        json!({"path": "relative/x"}),
        json!({"path": "/definitely/not/here"}),
        json!({"path": env.startup, "base_branch": "main"}),
        json!({"name": "x"}),
    ] {
        let (s, v) = call(&app, "POST", "/api/repos", Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
        assert_eq!(v["success"], false);
    }
    let text = Request::post("/api/repos")
        .header("content-type", "text/plain")
        .body(Body::from("{}"))
        .unwrap();
    let (s, v) = send(&app, text).await;
    assert_eq!((s, &v["success"]), (StatusCode::BAD_REQUEST, &json!(false)));
}

#[tokio::test]
async fn a_subfolder_is_not_a_repo() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let sub = env.startup.join("src");
    std::fs::create_dir_all(&sub).unwrap();
    let (s, v) = call(&app, "POST", "/api/repos", Some(json!({"path": sub}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let msg = v["error"].as_str().unwrap();
    assert!(
        msg.ends_with("is not the top folder of a git repo"),
        "{msg}"
    );
}

#[tokio::test]
async fn json_rejections_use_lowercase_contract_copy() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let raw = |content_type: &'static str, body: &'static str| {
        Request::post("/api/repos")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap()
    };
    let (s, v) = send(&app, raw("text/plain", "{}")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"], "expected content-type application/json");
    for (body, needle) in [
        ("{nope", "invalid json body: "),
        ("{\"path\": 1}", "invalid json body: "),
        (
            "{\"path\": \"/x\", \"base_branch\": \"m\"}",
            "unknown field `base_branch`",
        ),
    ] {
        let (s, v) = send(&app, raw("application/json", body)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
        let msg = v["error"].as_str().unwrap();
        assert!(msg.starts_with("invalid json body: "), "{msg}");
        assert!(msg.contains(needle), "{msg}");
        let detail = &msg["invalid json body: ".len()..];
        assert!(!detail.starts_with(char::is_uppercase), "{msg}");
    }
}
