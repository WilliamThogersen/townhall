use serde_json::json;
use townhall_core::plugins::claude_mcp;

mod common;
use common::store_env;
mod plugin_support;
use plugin_support::install_files;

const MCP: &str = r#"{"mcpServers": {
  "notes": {"command": "${CLAUDE_PLUGIN_ROOT}/notes"},
  "writer": {"command": "${CLAUDE_PLUGIN_ROOT}/writer"}
}}"#;

fn manifest(planner: bool) -> serde_json::Value {
    json!({
        "name": "notes", "version": "1.0.0", "apiVersion": 1,
        "permissions": {"claude": {
            "mcp": {"servers": {
                "notes": {"tools": ["search_notes"], "readOnly": true},
                "writer": ["add_note"]
            }},
            "planner": planner
        }}
    })
}

async fn names(env: &common::StoreEnv, chosen: &[&str]) -> Vec<claude_mcp::Contribution> {
    let chosen: Vec<String> = chosen.iter().map(|s| s.to_string()).collect();
    claude_mcp::selected_for_planner(&env.store, &chosen)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_planner_plugin_reaches_the_planner_with_its_read_only_tools_only() {
    let env = store_env();
    install_files(
        &env.store,
        "notes",
        "acme",
        manifest(true),
        &[(".mcp.json", MCP)],
        true,
    )
    .await;
    let parts = names(&env, &["notes@acme"]).await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].plugin, "notes@acme");
    assert_eq!(parts[0].tools, ["mcp__plugin_notes_notes__search_notes"]);
    let keys: Vec<&str> = parts[0].servers.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["plugin_notes_notes"]);
}

#[tokio::test]
async fn a_plugin_without_claude_planner_never_reaches_the_planner() {
    let env = store_env();
    install_files(
        &env.store,
        "notes",
        "acme",
        manifest(false),
        &[(".mcp.json", MCP)],
        true,
    )
    .await;
    assert!(names(&env, &["notes@acme"]).await.is_empty());
}

#[tokio::test]
async fn a_plugin_the_planner_type_does_not_list_or_that_is_disabled_gives_nothing() {
    let env = store_env();
    install_files(
        &env.store,
        "notes",
        "acme",
        manifest(true),
        &[(".mcp.json", MCP)],
        false,
    )
    .await;
    assert!(names(&env, &["notes@acme"]).await.is_empty());
    assert!(names(&env, &[]).await.is_empty());
}
