use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use townhall_core::model::{EntryBody, RepoRule, ServerEvent, TaskState};
use townhall_core::store::Store;
use townhall_daemon::AppState;

mod common;
use common::{call, unique, Env};

struct Mcp {
    env: Env,
    state: AppState,
    app: Router,
    id: String,
}

impl Mcp {
    async fn new() -> Self {
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

    fn store(&self) -> &Store {
        self.state.ctx().store().unwrap()
    }

    async fn post(&self, body: Value) -> (StatusCode, String) {
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

    async fn rpc(&self, body: Value) -> Value {
        let (status, text) = self.post(body).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        serde_json::from_str(&text).unwrap()
    }

    async fn tool(&self, name: &str, arguments: Value) -> Value {
        let body = json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
                          "params": {"name": name, "arguments": arguments}});
        let answer = self.rpc(body).await;
        assert_eq!(answer["id"], 7, "{answer}");
        answer["result"].clone()
    }

    async fn ok(&self, name: &str, arguments: Value) -> Value {
        let result = self.tool(name, arguments).await;
        assert!(result.get("isError").is_none(), "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    async fn refused(&self, name: &str, arguments: Value) -> String {
        let result = self.tool(name, arguments).await;
        assert_eq!(result["isError"], true, "{result}");
        assert_eq!(result["content"][0]["type"], "text");
        result["content"][0]["text"].as_str().unwrap().to_string()
    }

    async fn add_repo(&self, name: &str) {
        let path = self.env.repo(name).await;
        let body = json!({"path": path.to_string_lossy()});
        let (s, v) = call(&self.app, "POST", "/api/repos", Some(body)).await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }

    async fn note_repo(&self, repo: &str) {
        let body = EntryBody::Repo {
            repo: repo.into(),
            base_branch: "main".into(),
            rule: RepoRule::Named,
            reason: "you named it".into(),
        };
        self.store().append_entry(&self.id, body).await.unwrap();
    }

    async fn you(&self, text: &str) {
        let body = EntryBody::You {
            text: text.into(),
            attachments: vec![],
        };
        self.store().append_entry(&self.id, body).await.unwrap();
    }

    async fn tasks_created(&self) -> Vec<(u64, Vec<String>)> {
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

fn task(name: &str) -> Value {
    json!({"name": name, "title": format!("do {name}"), "prompt": "the brief"})
}

#[tokio::test]
async fn initialize_echoes_the_version_and_lists_the_tools() {
    let m = Mcp::new().await;
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                      "params": {"protocolVersion": "2025-03-26", "capabilities": {}}});
    let answer = m.rpc(init).await;
    assert_eq!(answer["jsonrpc"], "2.0");
    assert_eq!(answer["id"], 1);
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert_eq!(result["capabilities"], json!({"tools": {}}));
    assert_eq!(result["serverInfo"]["name"], "townhall");
    assert!(result["serverInfo"]["version"].is_string());
    assert_eq!(result["instructions"], townhall_core::mcp::INSTRUCTIONS);

    let bare = json!({"jsonrpc": "2.0", "id": "x", "method": "initialize"});
    assert_eq!(
        m.rpc(bare).await["result"]["protocolVersion"],
        townhall_core::mcp::PROTOCOL_VERSION
    );
    let ping = m
        .rpc(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}))
        .await;
    assert_eq!(ping, json!({"jsonrpc": "2.0", "id": 2, "result": {}}));

    let list = m
        .rpc(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}))
        .await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["create_task", "set_title"]);
    for t in tools {
        assert!(t["description"].is_string());
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(!t["inputSchema"].to_string().contains("ermission"), "{t}");
    }
}

#[tokio::test]
async fn notifications_get_202_and_batches_answer_in_order() {
    let m = Mcp::new().await;
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    assert_eq!(
        m.post(note.clone()).await,
        (StatusCode::ACCEPTED, String::new())
    );
    let batch = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "ping"},
        note,
        {"jsonrpc": "2.0", "id": "b", "method": "tools/list"},
        {"jsonrpc": "2.0", "id": 4, "method": "nope"}
    ]);
    let answers = m.rpc(batch).await;
    let answers = answers.as_array().unwrap();
    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0], json!({"jsonrpc": "2.0", "id": 1, "result": {}}));
    assert_eq!(answers[1]["id"], "b");
    assert!(answers[1]["result"]["tools"].is_array());
    assert_eq!(answers[2]["error"]["code"], -32601);
    let only_notes = json!([{"jsonrpc": "2.0", "method": "notifications/initialized"}]);
    assert_eq!(
        m.post(only_notes).await,
        (StatusCode::ACCEPTED, String::new())
    );
    let empty = m.rpc(json!([])).await;
    assert_eq!(empty["error"]["code"], -32600);
    assert_eq!(empty["id"], Value::Null);
    let invalid = m.rpc(json!({"id": 5, "method": "ping"})).await;
    assert_eq!(invalid["error"]["code"], -32600);
}

#[tokio::test]
async fn the_endpoint_checks_the_conversation_and_the_body() {
    let m = Mcp::new().await;
    let ping = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let (s, v) = call(
        &m.app,
        "POST",
        "/api/conversations/nope00000000/mcp",
        Some(ping),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["success"], false);
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/conversations/{}/mcp", m.id))
        .header("content-type", "application/json")
        .body(Body::from("{not json"))
        .unwrap();
    let res = m.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let no_name = json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {}});
    assert_eq!(m.rpc(no_name).await["error"]["code"], -32602);
    let unknown = m.refused("delete_everything", json!({})).await;
    assert!(unknown.contains("create_task"), "{unknown}");
}

#[tokio::test]
async fn create_task_needs_a_repo_and_defaults_to_the_conversations() {
    let m = Mcp::new().await;
    m.add_repo("admin").await;
    let none = m.refused("create_task", task(&unique("a"))).await;
    assert!(none.contains("admin") && none.contains("shop"), "{none}");

    m.note_repo("shop").await;
    let name = unique("fix-login");
    let created = m.ok("create_task", task(&name)).await;
    assert_eq!(created["name"], name.as_str());
    assert_eq!(created["repo"], "shop");
    assert_eq!(created["baseBranch"], "main");
    assert_eq!(created["state"], "queued");
    assert_eq!(created["conversationId"], m.id.as_str());

    let patch = |c: townhall_core::model::Conversation| {
        Ok(townhall_core::model::Conversation {
            linked_repo: Some("admin".into()),
            ..c
        })
    };
    m.store().update_conversation(&m.id, patch).await.unwrap();
    let linked = m.ok("create_task", task(&unique("b"))).await;
    assert_eq!(linked["repo"], "admin");

    let explicit = json!({"repo": "SHOP", "baseBranch": "main", "after": [name]});
    let mut args = task(&unique("c"));
    args.as_object_mut()
        .unwrap()
        .extend(explicit.as_object().unwrap().clone());
    let picked = m.ok("create_task", args).await;
    assert_eq!(
        (&picked["repo"], &picked["after"]),
        (&json!("shop"), &json!([name]))
    );

    let mut bad_repo = task(&unique("d"));
    bad_repo["repo"] = json!("nope");
    let text = m.refused("create_task", bad_repo).await;
    assert!(
        text.contains("nope") && text.contains("admin, shop"),
        "{text}"
    );
    let mut bad_base = task(&unique("e"));
    bad_base["baseBranch"] = json!("no-such-branch");
    let text = m.refused("create_task", bad_base).await;
    assert!(text.contains("no-such-branch"), "{text}");
}

#[tokio::test]
async fn create_task_refuses_bad_input_with_a_reason() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let name = unique("dup");
    m.ok("create_task", task(&name)).await;
    let dup = m.refused("create_task", task(&name.to_uppercase())).await;
    assert!(dup.contains("already exists"), "{dup}");

    let mut ghost = task(&unique("g"));
    ghost["after"] = json!(["ghost-task"]);
    let text = m.refused("create_task", ghost).await;
    assert!(text.contains("ghost-task"), "{text}");

    let mut sneaky = task(&unique("s"));
    sneaky["permissionMode"] = json!("bypassPermissions");
    let text = m.refused("create_task", sneaky).await;
    assert!(text.contains("permissionMode"), "{text}");

    let cases = [
        json!({"name": "has space", "title": "t", "prompt": "p"}),
        json!({"name": "x".repeat(65), "title": "t", "prompt": "p"}),
        json!({"name": unique("t"), "title": "t".repeat(81), "prompt": "p"}),
        json!({"name": unique("t"), "title": "  ", "prompt": "p"}),
        json!({"name": unique("t"), "title": "t", "prompt": ""}),
        json!({"name": unique("t"), "title": "t", "prompt": "p", "after": vec![name.clone(); 21]}),
        json!({"title": "t", "prompt": "p"}),
    ];
    for args in cases {
        let text = m.refused("create_task", args.clone()).await;
        assert!(!text.is_empty(), "{args}");
    }
    assert_eq!(m.store().all_tasks().await.unwrap().len(), 1);
}

#[tokio::test]
async fn create_task_takes_attachments_of_this_conversation() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let file = m
        .store()
        .save_attachment(&m.id, "shot.png", "image/png", vec![1, 2, 3])
        .await
        .unwrap();
    let mut args = task(&unique("att"));
    args["attachments"] = json!([file.id, file.id]);
    let created = m.ok("create_task", args).await;
    assert_eq!(created["attachments"], json!([file.id]));
    let mut unknown = task(&unique("att"));
    unknown["attachments"] = json!(["nope00000000"]);
    let text = m.refused("create_task", unknown).await;
    assert!(text.contains("nope00000000"), "{text}");
}

#[tokio::test]
async fn one_tasks_created_entry_per_turn() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    m.you("make two tasks").await;
    let (a, b, c) = (unique("a"), unique("b"), unique("c"));
    m.ok("create_task", task(&a)).await;
    let mut events = m.state.ctx().bus().subscribe();
    m.ok("create_task", task(&b)).await;
    let created = m.tasks_created().await;
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].1, [a.clone(), b.clone()]);
    let mut resent = None;
    while let Ok(event) = events.try_recv() {
        if let ServerEvent::EntryAppended { entry, .. } = event {
            resent = Some(entry);
        }
    }
    let resent = resent.expect("the merged entry is sent again");
    assert_eq!(resent.id, created[0].0);

    m.you("and one more").await;
    m.ok("create_task", task(&c)).await;
    let created = m.tasks_created().await;
    assert_eq!(created.len(), 2);
    assert_eq!(created[1].1, [c]);
    assert!(created[1].0 > created[0].0);
    let tasks = m.store().tasks(&m.id).await.unwrap();
    assert!(tasks.iter().all(|t| t.state == TaskState::Queued));
}

#[tokio::test]
async fn set_title_names_the_conversation() {
    let m = Mcp::new().await;
    let named = m
        .ok("set_title", json!({"title": "  fix the\n login   loop "}))
        .await;
    assert_eq!(named["title"], "fix the login loop");
    assert_eq!(named["id"], m.id.as_str());
    assert_eq!(
        m.store().conversation(&m.id).await.unwrap().title,
        "fix the login loop"
    );
    for bad in [
        json!({"title": " "}),
        json!({"title": "t".repeat(81)}),
        json!({"title": "ok", "extra": 1}),
        json!({}),
    ] {
        m.refused("set_title", bad).await;
    }
}

#[tokio::test]
async fn create_task_refuses_a_name_an_agent_or_branch_has() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let branch_name = unique("branch");
    let branch = format!("townhall/{branch_name}");
    common::git(&m.env.startup, &["branch", &branch]).await;
    let text = m.refused("create_task", task(&branch_name)).await;
    assert!(text.contains(&branch) && text.contains("shop"), "{text}");

    let agent = unique("agent");
    let body = json!({"name": agent, "adapter": "shell", "command": "sleep 60"});
    let (s, v) = call(&m.app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let text = m.refused("create_task", task(&agent)).await;
    call(&m.app, "DELETE", &format!("/api/agents/{agent}"), None).await;
    assert!(text.contains("an agent named"), "{text}");
    assert!(m.store().all_tasks().await.unwrap().is_empty());
}
