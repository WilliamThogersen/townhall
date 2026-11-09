use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::EntryBody;

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const REVIEWER: &str = "name = \"reviewer\"\ndescription = \"reviews a diff\"\nadapter = \"claude\"\nmodel = \"opus\"\npermission_mode = \"plan\"\nprompt = \"\"\"\nuse ```code``` fences\n\"\"\"\n";

async fn saved(m: &Mcp, name: &str) -> Option<Value> {
    let uri = format!("/api/library/agents/{name}");
    let (s, v) = call(&m.app, "GET", &uri, None).await;
    (s == StatusCode::OK).then(|| v["data"].clone())
}

async fn last_townhall(m: &Mcp) -> (u64, String) {
    let entries = m.store().entries(&m.id).await.unwrap();
    let last = entries.last().unwrap();
    match &last.body {
        EntryBody::Townhall { text } => (last.id, text.clone()),
        other => panic!("expected a townhall entry, got {other:?}"),
    }
}

async fn work_without_asking(m: &Mcp, on: bool) {
    let body = json!({"value": on});
    let (s, v) = call(&m.app, "PUT", "/api/settings/workWithoutAsking", Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn list_and_read_show_the_saved_types() {
    let m = Mcp::new().await;
    let listed = m.ok("list_agent_types", json!({})).await;
    assert_eq!(listed["default"], "builder");
    let types = listed["types"].as_array().unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[1]["name"], "default");
    assert_eq!(
        (
            &types[0]["name"],
            &types[0]["description"],
            &types[0]["model"]
        ),
        (
            &json!("builder"),
            &json!("implements a task, tests first"),
            &json!("sonnet")
        )
    );
    let read = m.ok("read_agent_type", json!({"name": "BUILDER"})).await;
    assert_eq!(read["name"], "builder");
    assert!(read["source"]
        .as_str()
        .unwrap()
        .contains("adapter = \"claude\""));
    let unknown = m.refused("read_agent_type", json!({"name": "ghost"})).await;
    assert!(
        unknown.contains("ghost") && unknown.contains("builder"),
        "{unknown}"
    );
    let extra = m.refused("list_agent_types", json!({"x": 1})).await;
    assert!(extra.contains("unknown field"), "{extra}");
}

#[tokio::test]
async fn read_agent_type_shows_what_a_type_requires() {
    let m = Mcp::new().await;
    let source =
        "name = \"shipper\"\nadapter = \"claude\"\nrequires = [\"tester\", \"reviewer\"]\n";
    m.store().save_agent_type(source).await.unwrap();
    let read = m.ok("read_agent_type", json!({"name": "shipper"})).await;
    assert_eq!(read["requires"], json!(["tester", "reviewer"]), "{read}");
    assert!(read["source"].as_str().unwrap().contains("requires = ["));
    let plain = m.ok("read_agent_type", json!({"name": "builder"})).await;
    assert_eq!(plain["requires"], json!([]), "{plain}");
    let bad = "name = \"x\"\nadapter = \"claude\"\nrequires = [\"a b\"]\n";
    let text = m.refused("save_agent_type", json!({"source": bad})).await;
    assert!(text.contains("requires"), "{text}");
}

#[tokio::test]
async fn an_invalid_type_comes_back_as_its_errors() {
    let m = Mcp::new().await;
    let bad = "name = \"x\"\nadapter = \"claude\"\npermission_mode = \"bypassPermissions\"\n";
    let text = m.refused("save_agent_type", json!({"source": bad})).await;
    assert!(
        text.contains("error: agent \"x\"") && text.contains("skip all"),
        "{text}"
    );
    assert!(saved(&m, "x").await.is_none());
}

#[tokio::test]
async fn a_new_type_asks_first_and_saves_once_confirmed() {
    let m = Mcp::new().await;
    let asked = m.ok("save_agent_type", json!({"source": REVIEWER})).await;
    assert_eq!(
        (&asked["status"], &asked["replaces"]),
        (&json!("asked"), &json!(false))
    );
    assert!(
        asked["next"].as_str().unwrap().contains("confirmed"),
        "{asked}"
    );
    assert!(saved(&m, "reviewer").await.is_none());
    let (_, text) = last_townhall(&m).await;
    assert!(
        text.contains("`reviewer`") && text.contains("````toml\n"),
        "{text}"
    );
    assert!(text.contains(REVIEWER) && text.contains("yes"), "{text}");

    let early = json!({"source": REVIEWER, "confirmed": true});
    let text = m.refused("save_agent_type", early.clone()).await;
    assert!(text.contains("confirmed"), "{text}");
    m.you("yes, save it").await;
    let changed = json!({"source": REVIEWER.replace("opus", "haiku"), "confirmed": true});
    let text = m.refused("save_agent_type", changed).await;
    assert!(
        text.contains("same source") || text.contains("confirm"),
        "{text}"
    );
    let done = m.ok("save_agent_type", early.clone()).await;
    assert_eq!(
        (&done["status"], &done["replaced"]),
        (&json!("saved"), &json!(false))
    );
    assert_eq!(saved(&m, "reviewer").await.unwrap()["source"], REVIEWER);
    let again = m.refused("save_agent_type", early).await;
    assert!(again.contains("confirm"), "{again}");
}

#[tokio::test]
async fn work_without_asking_saves_new_types_but_never_replaces() {
    let m = Mcp::new().await;
    work_without_asking(&m, true).await;
    let before = m.store().entries(&m.id).await.unwrap().len();
    let done = m.ok("save_agent_type", json!({"source": REVIEWER})).await;
    assert_eq!(done["status"], "saved");
    assert_eq!(m.store().entries(&m.id).await.unwrap().len(), before);
    assert_eq!(saved(&m, "reviewer").await.unwrap()["model"], "opus");

    let replace = REVIEWER.replace("opus", "haiku");
    let asked = m.ok("save_agent_type", json!({"source": replace})).await;
    assert_eq!(
        (&asked["status"], &asked["replaces"]),
        (&json!("asked"), &json!(true))
    );
    let (_, text) = last_townhall(&m).await;
    assert!(text.contains("replace"), "{text}");
    assert_eq!(saved(&m, "reviewer").await.unwrap()["model"], "opus");
    m.you("yes").await;
    let done = m
        .ok(
            "save_agent_type",
            json!({"source": replace, "confirmed": true}),
        )
        .await;
    assert_eq!(
        (&done["status"], &done["replaced"]),
        (&json!("saved"), &json!(true))
    );
    assert_eq!(saved(&m, "reviewer").await.unwrap()["model"], "haiku");
}

#[tokio::test]
async fn the_session_may_use_the_agent_type_tools() {
    let allowed = townhall_core::mcp::allowed_tools();
    for tool in ["list_agent_types", "read_agent_type", "save_agent_type"] {
        assert!(
            allowed.contains(&format!("mcp__townhall__{tool}")),
            "{tool}"
        );
    }
    let m = Mcp::new().await;
    let listed = m
        .rpc(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"save_agent_type"), "{names:?}");
}

#[tokio::test]
async fn create_task_runs_as_a_saved_type() {
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let first = m
        .ok("create_task", mcp_support::task(&common::unique("a")))
        .await;
    assert_eq!(first["agent"], "builder");
    work_without_asking(&m, true).await;
    m.ok("save_agent_type", json!({"source": REVIEWER})).await;
    let mut args = mcp_support::task(&common::unique("b"));
    args["agent"] = json!("REVIEWER");
    assert_eq!(m.ok("create_task", args).await["agent"], "reviewer");
    let mut ghost = mcp_support::task(&common::unique("c"));
    ghost["agent"] = json!("ghost");
    let text = m.refused("create_task", ghost).await;
    assert!(
        text.contains("ghost") && text.contains("builder, reviewer"),
        "{text}"
    );

    let status = m.ok("task_status", json!({})).await;
    let agents: Vec<&Value> = status["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| &t["agent"])
        .collect();
    assert_eq!(agents, [&json!("builder"), &json!("reviewer")]);
    let (s, _) = call(&m.app, "DELETE", "/api/library/agents/builder", None).await;
    assert_eq!(s, StatusCode::OK);
    let fallback = m
        .ok("create_task", mcp_support::task(&common::unique("d")))
        .await;
    assert_eq!(fallback["agent"], "reviewer");
    let (_, v) = call(
        &m.app,
        "GET",
        &format!("/api/tasks/{}", first["name"].as_str().unwrap()),
        None,
    )
    .await;
    assert_eq!(v["data"]["agent"], "builder");
}
