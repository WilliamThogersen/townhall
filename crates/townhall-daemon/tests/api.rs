use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::json;
use tempfile::TempDir;
use tower::ServiceExt;
use townhall_core::{Home, Workspace};
use townhall_daemon::{router, AppState};

mod common;
use common::{call, settle, unique};

struct Env {
    repo: TempDir,
    home: TempDir,
}

impl Env {
    fn workspace(&self) -> Workspace {
        Workspace::new(
            Home::at(self.home.path().join(".townhall")),
            self.repo.path(),
        )
        .unwrap()
    }
}

fn state_for(repo: &std::path::Path, home: &std::path::Path) -> AppState {
    let ws = Workspace::new(Home::at(home.join(".townhall")), repo).unwrap();
    AppState::new(ws)
}

async fn init_repo() -> (Env, Router) {
    common::private_tmux();
    let dir = TempDir::new().unwrap();
    let git =
        |args: &'static [&'static str]| townhall_core::cmd::run("git", args, Some(dir.path()));
    git(&["init", "-b", "main"]).await.unwrap();
    git(&[
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "--allow-empty",
        "-m",
        "init",
    ])
    .await
    .unwrap();
    let home = TempDir::new().unwrap();
    let app = router(state_for(dir.path(), home.path()));
    (Env { repo: dir, home }, app)
}

#[tokio::test]
async fn a_running_agent_is_listed_sent_to_and_stopped_over_http() {
    let (env, app) = init_repo().await;
    let name = unique("api");

    let (s, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])));
    assert_eq!(v["success"], true);

    common::start_shell(&env.workspace(), &name, "sh -c 'echo up; cat'").await;
    settle().await;

    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"][0]["name"], name.as_str());
    assert_eq!(v["data"][0]["kind"], "shell", "{v}");
    assert_eq!(v["data"][0]["alive"], true, "{v}");
    let wt = v["data"][0]["worktree"].as_str().unwrap();
    assert!(wt.starts_with(env.home.path().to_str().unwrap()), "{wt}");
    assert!(!wt.starts_with(env.repo.path().to_str().unwrap()), "{wt}");

    let uri = format!("/api/agents/{name}/send");
    let (s, _) = call(&app, "POST", &uri, Some(json!({"text": "hello-api"}))).await;
    assert_eq!(s, StatusCode::OK);
    settle().await;

    let (s, v) = call(&app, "GET", &format!("/api/agents/{name}/output"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["data"].as_str().unwrap().contains("hello-api"), "{v}");

    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"], json!([]));
}

#[tokio::test]
async fn agents_cannot_be_created_over_http() {
    let (_env, app) = init_repo().await;
    let name = unique("direct");
    let body = json!({"name": name, "adapter": "shell", "command": "sleep 30"});
    let (s, _) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"], json!([]), "nothing was started");
}

#[tokio::test]
async fn an_invalid_name_is_400() {
    let (_env, app) = init_repo().await;
    let (s, v) = call(&app, "DELETE", "/api/agents/bad%20name!", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["success"], false);
    assert!(v["error"].as_str().unwrap().contains("invalid name"), "{v}");
}

#[tokio::test]
async fn missing_agent_is_404() {
    let (_dir, app) = init_repo().await;
    let n = unique("ghost");
    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{n}"), None).await;
    assert_eq!((s, &v["success"]), (StatusCode::NOT_FOUND, &json!(false)));
    let (s, _) = call(&app, "GET", &format!("/api/agents/{n}/output"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/agents/{n}/send"),
        Some(json!({"text": "x"})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cors_allows_desktop_origin_only() {
    use axum::http::{header, Method};
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let app = townhall_daemon::router_with_cors(state_for(dir.path(), home.path()));
    let preflight = |origin: &'static str| {
        Request::builder()
            .method(Method::OPTIONS)
            .uri("/api/repos")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
            .body(Body::empty())
            .unwrap()
    };
    let ok = app
        .clone()
        .oneshot(preflight("tauri://localhost"))
        .await
        .unwrap();
    assert_eq!(
        ok.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "tauri://localhost"
    );
    let bad = app
        .oneshot(preflight("https://evil.example"))
        .await
        .unwrap();
    assert!(bad
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}

#[tokio::test]
async fn cors_preflight_allows_the_request_id_header() {
    use axum::http::{header, Method};
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let app = townhall_daemon::router_with_cors(state_for(dir.path(), home.path()));
    let preflight = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/repos")
        .header(header::ORIGIN, "tauri://localhost")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "x-request-id")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(preflight).await.unwrap();
    let allowed = res
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        allowed.contains("x-request-id"),
        "allow-headers: {allowed:?}"
    );
}

#[tokio::test]
async fn info_reports_the_build_version() {
    let (_env, app) = init_repo().await;
    let (s, v) = call(&app, "GET", "/api/info", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["version"], townhall_core::version::VERSION);
    assert!(
        townhall_core::version::VERSION.starts_with(env!("CARGO_PKG_VERSION")),
        "{}",
        townhall_core::version::VERSION
    );
}

#[test]
fn the_daemon_prints_its_build_version() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_townhall-daemon"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        format!("townhall-daemon {}", townhall_core::version::VERSION)
    );
}

#[tokio::test]
async fn info_reports_home_repo_and_key() {
    let (env, app) = init_repo().await;
    let (s, v) = call(&app, "GET", "/api/info", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["success"], true);
    let home = env.home.path().join(".townhall");
    assert_eq!(v["data"]["home"], home.to_str().unwrap());
    let repo = env.repo.path().canonicalize().unwrap();
    assert_eq!(v["data"]["repo"], repo.to_str().unwrap());
    let key = v["data"]["repoKey"].as_str().unwrap();
    assert!(
        key.ends_with(|c: char| c.is_ascii_hexdigit()) && key.contains('-'),
        "{key}"
    );
}
