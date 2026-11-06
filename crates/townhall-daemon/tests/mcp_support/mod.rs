#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use townhall_core::model::{EntryBody, RepoRule};
use townhall_core::store::Store;
use townhall_daemon::AppState;

use crate::common::{call, Env};

/// One conversation of a daemon on a temp repo `shop`, and calls to its MCP endpoint.
pub struct Mcp {
    pub env: Env,
    pub state: AppState,
    pub app: Router,
    pub id: String,
}

impl Mcp {
    pub async fn new() -> Self {
        let env = Env::new("shop").await;
        let (state, app) = env.start();
        state.ctx().repos().await.unwrap();
        let id = state
            .ctx()
            .store()
            .unwrap()
            .create_conversation()
            .await
            .unwrap()
            .id;
        Self {
            env,
            state,
            app,
            id,
        }
    }

    pub fn store(&self) -> &Store {
        self.state.ctx().store().unwrap()
    }

    pub async fn post(&self, body: Value) -> (StatusCode, String) {
        let req = Request::builder()
            .method("POST")
            .uri(format!("/api/conversations/{}/mcp", self.id))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub async fn rpc(&self, body: Value) -> Value {
        let (status, text) = self.post(body).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        serde_json::from_str(&text).unwrap()
    }

    pub async fn tool(&self, name: &str, arguments: Value) -> Value {
        let body = json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
                          "params": {"name": name, "arguments": arguments}});
        let answer = self.rpc(body).await;
        assert_eq!(answer["id"], 7, "{answer}");
        answer["result"].clone()
    }

    pub async fn ok(&self, name: &str, arguments: Value) -> Value {
        let result = self.tool(name, arguments).await;
        assert!(result.get("isError").is_none(), "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    pub async fn refused(&self, name: &str, arguments: Value) -> String {
        let result = self.tool(name, arguments).await;
        assert_eq!(result["isError"], true, "{result}");
        assert_eq!(result["content"][0]["type"], "text");
        result["content"][0]["text"].as_str().unwrap().to_string()
    }

    pub async fn add_repo(&self, name: &str) {
        let path = self.env.repo(name).await;
        let body = json!({"path": path.to_string_lossy()});
        let (s, v) = call(&self.app, "POST", "/api/repos", Some(body)).await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }

    pub async fn note_repo(&self, repo: &str) {
        let body = EntryBody::Repo {
            repo: repo.into(),
            base_branch: "main".into(),
            rule: RepoRule::Named,
            reason: "you named it".into(),
        };
        self.store().append_entry(&self.id, body).await.unwrap();
    }

    pub async fn you(&self, text: &str) {
        let body = EntryBody::You {
            text: text.into(),
            attachments: vec![],
            reply_to: None,
        };
        self.store().append_entry(&self.id, body).await.unwrap();
    }

    pub async fn tasks_created(&self) -> Vec<(u64, Vec<String>)> {
        let entries = self.store().entries(&self.id).await.unwrap();
        entries
            .into_iter()
            .filter_map(|e| match e.body {
                EntryBody::TasksCreated { tasks } => Some((e.id, tasks)),
                _ => None,
            })
            .collect()
    }
}

pub fn task(name: &str) -> Value {
    json!({"name": name, "title": format!("do {name}"), "prompt": "the brief"})
}
