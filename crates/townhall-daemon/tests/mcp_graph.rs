use serde_json::json;
use townhall_core::model::{Checks, EntryBody, NewTask, Pr, Task, TaskState};

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";
const REVIEWER: &str = "name = \"reviewer\"\nadapter = \"claude\"\n";

async fn make_task(m: &Mcp, name: &str, agent: &str, state: TaskState, files: &[&str]) {
    let new = NewTask {
        name: name.into(),
        title: format!("work on {name}"),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: agent.into(),
        pr_base: None,
        done: Default::default(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
    m.store()
        .update_task(name, move |t| Ok(Task { state, ..t }))
        .await
        .unwrap();
    let files = files.iter().map(|f| f.to_string()).collect();
    m.store()
        .record_task_graph(name, files, vec!["00aa00aa00aa00aa".into()])
        .await
        .unwrap();
}

async fn seed(m: &Mcp) {
    m.store().save_agent_type(CODER).await.unwrap();
    m.store().save_agent_type(REVIEWER).await.unwrap();
    make_task(
        m,
        "first",
        "coder",
        TaskState::Merged,
        &["src/insights/mod.rs", "src/a.rs"],
    )
    .await;
    m.store()
        .update_task("first", |t| {
            Ok(Task {
                pr: Some(Pr {
                    number: 4,
                    url: None,
                    checks: Checks::Passed,
                }),
                ..t
            })
        })
        .await
        .unwrap();
    make_task(
        m,
        "second",
        "coder",
        TaskState::Error,
        &["src/insights/tests.rs"],
    )
    .await;
    make_task(
        m,
        "third",
        "reviewer",
        TaskState::Ended,
        &["src/insights/mod.rs"],
    )
    .await;
    let text = "task second failed: API error 529 overloaded token=hunter2hunter2";
    let body = EntryBody::Event { text: text.into() };
    m.store().append_entry(&m.id, body).await.unwrap();
    m.store()
        .record_task_graph("second", vec!["src/insights/tests.rs".into()], vec![])
        .await
        .unwrap();
}

#[tokio::test]
async fn both_tools_go_to_the_planner_and_to_every_agent() {
    let m = Mcp::new().await;
    seed(&m).await;
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let planner = m.rpc(list.clone()).await;
    let (status, agent) = call(&m.app, "POST", "/api/tasks/first/mcp", Some(list)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{agent}");
    for tool in ["history_graph", "agent_stats"] {
        for answer in [&planner, &agent] {
            let tools = answer["result"]["tools"].as_array().unwrap();
            let found = tools.iter().find(|t| t["name"] == tool).unwrap();
            assert_eq!(found["annotations"]["readOnlyHint"], true, "{tool}");
        }
        let allowed = format!("mcp__townhall__{tool}");
        assert!(townhall_core::mcp::allowed_tools().contains(&allowed));
        assert!(townhall_core::mcp::task_allowed_tools().contains(&allowed));
    }
    let ask = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "history_graph", "arguments": {"node": "src/a.rs"}}});
    let (_, answer) = call(&m.app, "POST", "/api/tasks/first/mcp", Some(ask)).await;
    assert_ne!(answer["result"]["isError"], true, "{answer}");
}

#[tokio::test]
async fn a_file_leads_to_the_tasks_that_touched_it_with_agent_outcome_and_date() {
    let m = Mcp::new().await;
    seed(&m).await;
    let out = m
        .ok("history_graph", json!({"node": "src/insights/mod.rs"}))
        .await;
    assert_eq!(
        out["node"],
        json!({"kind": "file", "id": "src/insights/mod.rs"})
    );
    assert_eq!(out["matched"], 2);
    let tasks = out["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2, "{out}");
    let first = tasks.iter().find(|t| t["task"] == "first").unwrap();
    assert_eq!(first["agent"], "coder");
    assert_eq!(first["outcome"], "merged");
    assert_eq!(first["conversation"], m.id.as_str());
    assert_eq!(first["pr"], "shop#4");
    assert_eq!(first["branch"], "townhall/first");
    assert!(first["at"].is_string() && first["title"].is_string());
    assert!(first.get("files").is_none(), "{first}");
    let third = tasks.iter().find(|t| t["task"] == "third").unwrap();
    assert_eq!(third["agent"], "reviewer");
    assert_eq!(third["outcome"], "ended");

    let glob = m
        .ok(
            "history_graph",
            json!({"node": "src/insights/*", "kind": "file", "limit": 2}),
        )
        .await;
    assert_eq!(glob["matched"], 3);
    assert_eq!(glob["tasks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn depth_two_adds_files_and_fingerprints_and_other_nodes_work() {
    let m = Mcp::new().await;
    seed(&m).await;
    let deep = m
        .ok("history_graph", json!({"node": "src/a.rs", "depth": 2}))
        .await;
    let task = &deep["tasks"][0];
    assert_eq!(task["files"], json!(["src/a.rs", "src/insights/mod.rs"]));
    assert_eq!(task["fingerprints"], json!(["00aa00aa00aa00aa"]));

    for (node, kind, count) in [
        ("first", "task", 1),
        ("townhall/second", "branch", 1),
        ("coder", "agent", 2),
        ("shop#4", "pr", 1),
        ("00aa00aa00aa00aa", "fingerprint", 2),
        ("merged", "outcome", 1),
    ] {
        let out = m.ok("history_graph", json!({"node": node})).await;
        assert_eq!(out["node"]["kind"], kind, "{node}: {out}");
        assert_eq!(out["tasks"].as_array().unwrap().len(), count, "{node}");
    }
    let by_conversation = m.ok("history_graph", json!({"node": m.id})).await;
    assert_eq!(by_conversation["node"]["kind"], "conversation");
    assert_eq!(by_conversation["tasks"].as_array().unwrap().len(), 3);
    let recent = m
        .ok(
            "history_graph",
            json!({"node": "src/a.rs", "since": "2999-01-01T00:00:00Z"}),
        )
        .await;
    assert_eq!(recent["tasks"], json!([]));
}

#[tokio::test]
async fn history_graph_refuses_unknown_nodes_and_bad_arguments() {
    let m = Mcp::new().await;
    seed(&m).await;
    let unknown = m
        .refused("history_graph", json!({"node": "no/such/file.rs"}))
        .await;
    assert!(unknown.contains("no/such/file.rs"), "{unknown}");
    m.refused("history_graph", json!({})).await;
    m.refused("history_graph", json!({"node": "  "})).await;
    m.refused("history_graph", json!({"node": "a", "depth": 3}))
        .await;
    m.refused("history_graph", json!({"node": "a", "depth": 0}))
        .await;
    m.refused("history_graph", json!({"node": "a", "kind": "tag"}))
        .await;
    m.refused("history_graph", json!({"node": "a", "limit": 0}))
        .await;
    m.refused("history_graph", json!({"node": "a", "extra": 1}))
        .await;
    let since = m
        .refused(
            "history_graph",
            json!({"node": "src/a.rs", "since": "yesterday"}),
        )
        .await;
    assert!(since.contains("since"), "{since}");
}

#[tokio::test]
async fn agent_stats_summarise_each_type_compactly_and_redacted() {
    let m = Mcp::new().await;
    seed(&m).await;
    let out = m.ok("agent_stats", json!({})).await;
    let agents = out["agents"].as_array().unwrap();
    let coder = agents.iter().find(|a| a["agent"] == "coder").unwrap();
    assert_eq!(coder["tasks"], 2);
    assert_eq!(coder["outcomes"], json!({"api-error": 1, "merged": 1}));
    assert_eq!(coder["avgRetries"], 0.0);
    assert_eq!(coder["handBacks"], 0);
    assert_eq!(coder["failures"][0]["count"], 1);
    assert!(coder["failures"][0]["reason"]
        .as_str()
        .unwrap()
        .starts_with("API error"));
    assert_eq!(
        coder["failsOn"],
        json!([{"area": "src/insights", "count": 1}])
    );
    let reviewer = agents.iter().find(|a| a["agent"] == "reviewer").unwrap();
    assert_eq!(reviewer["tasks"], 1);
    assert_eq!(reviewer["failures"], json!([]));
    assert!(!out.to_string().contains("hunter2"), "{out}");

    let only = m
        .ok("agent_stats", json!({"agent": "Reviewer", "since": "7d"}))
        .await;
    assert_eq!(only["agents"].as_array().unwrap().len(), 1, "{only}");
    let none = m
        .ok("agent_stats", json!({"since": "2999-01-01T00:00:00Z"}))
        .await;
    assert_eq!(none["agents"], json!([]));
    m.refused("agent_stats", json!({"since": "yesterday"}))
        .await;
    m.refused("agent_stats", json!({"extra": true})).await;
}
