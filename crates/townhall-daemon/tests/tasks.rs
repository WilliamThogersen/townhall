use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{NewTask, Task, TaskState};

mod common;
use common::{call, Env};

fn new_task(name: &str, repo: &str) -> NewTask {
    NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: "the brief".into(),
        repo: repo.into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
    }
}

fn names(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn tasks_are_listed_oldest_first_and_filtered() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let (a, b) = (
        store.create_conversation().await.unwrap(),
        store.create_conversation().await.unwrap(),
    );
    store
        .create_task(&a.id, new_task("one", "shop"))
        .await
        .unwrap();
    store
        .create_task(&b.id, new_task("two", "admin"))
        .await
        .unwrap();
    store
        .create_task(&a.id, new_task("three", "shop"))
        .await
        .unwrap();
    let error = |t: Task| {
        Ok(Task {
            state: TaskState::Error,
            ..t
        })
    };
    store.update_task("three", error).await.unwrap();

    let (s, v) = call(&app, "GET", "/api/tasks", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(names(&v), ["one", "two", "three"]);
    assert_eq!(v["data"][0]["state"], "queued");
    assert_eq!(v["data"][0]["baseBranch"], "main");

    let cases = [
        (
            format!("/api/tasks?conversation={}", a.id),
            vec!["one", "three"],
        ),
        ("/api/tasks?repo=ADMIN".to_string(), vec!["two"]),
        ("/api/tasks?state=error".to_string(), vec!["three"]),
        (
            format!("/api/tasks?conversation={}&repo=shop&state=queued", a.id),
            vec!["one"],
        ),
        (
            "/api/tasks?repo=&state=".to_string(),
            vec!["one", "two", "three"],
        ),
        ("/api/tasks?conversation=nope00000000".to_string(), vec![]),
    ];
    for (uri, want) in cases {
        let (s, v) = call(&app, "GET", &uri, None).await;
        assert_eq!(s, StatusCode::OK, "{uri}: {v}");
        assert_eq!(names(&v), want, "{uri}");
    }
    let (s, v) = call(&app, "GET", "/api/tasks?state=done", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert!(v["error"].as_str().unwrap().contains("queued"), "{v}");
}

#[tokio::test]
async fn one_task_by_name_ignoring_case() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let c = store.create_conversation().await.unwrap();
    store
        .create_task(&c.id, new_task("fix-login", "shop"))
        .await
        .unwrap();
    let (s, v) = call(&app, "GET", "/api/tasks/Fix-Login", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["name"], "fix-login");
    assert_eq!(v["data"]["conversationId"], c.id.as_str());
    assert_eq!(v["data"]["pr"], Value::Null);
    let (s, v) = call(&app, "GET", "/api/tasks/ghost", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(
        v,
        json!({"success": false, "data": null, "error": "task `ghost` not found"})
    );
}
