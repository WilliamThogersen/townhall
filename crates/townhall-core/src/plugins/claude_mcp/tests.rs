use std::path::Path;

use serde_json::{json, Value};

use super::{contribution, for_planner, merge, Contribution};
use crate::plugins::manifest::{parse_townhall, TownhallManifest};

const MCP: &str = include_str!("../../../tests/fixtures/plugins/claude/mcp.json");
const ROOT: &str = "/h/plugins/cache/acme/notes/abc";

fn manifest(servers: Value, env: Value) -> TownhallManifest {
    parse_townhall(
        &json!({
            "name": "notes", "version": "1.0.0", "apiVersion": 1,
            "permissions": {"claude": {"mcp": {"servers": servers}}, "env": env}
        })
        .to_string(),
    )
    .unwrap()
}

fn declared() -> TownhallManifest {
    manifest(
        json!({"notes": ["search_notes", "add_note"], "remote": {"tools": ["*"]}}),
        json!(["ACME_TOKEN"]),
    )
}

fn without_townhall() -> String {
    let mut all: Value = serde_json::from_str(MCP).unwrap();
    all["mcpServers"]
        .as_object_mut()
        .unwrap()
        .remove("townhall");
    all.to_string()
}

#[test]
fn servers_are_renamed_and_their_tools_are_the_approved_ones() {
    let found = contribution("notes", Path::new(ROOT), &declared(), &without_townhall()).unwrap();
    let keys: Vec<&str> = found.servers.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["plugin_notes_notes", "plugin_notes_remote"]);
    assert_eq!(
        found.tools,
        [
            "mcp__plugin_notes_notes__search_notes",
            "mcp__plugin_notes_notes__add_note",
            "mcp__plugin_notes_remote",
        ]
    );
}

#[test]
fn the_plugin_root_is_replaced_in_the_command_and_the_args() {
    let found = contribution("notes", Path::new(ROOT), &declared(), &without_townhall()).unwrap();
    let notes = &found.servers[0].1;
    assert_eq!(notes["command"], format!("{ROOT}/servers/notes"));
    assert_eq!(notes["args"], json!(["--data", format!("{ROOT}/data")]));
    assert!(!notes.to_string().contains("CLAUDE_PLUGIN_ROOT"));
}

#[test]
fn an_env_var_the_plugin_did_not_declare_is_dropped() {
    let found = contribution("notes", Path::new(ROOT), &declared(), &without_townhall()).unwrap();
    assert_eq!(
        found.servers[0].1["env"],
        json!({"ACME_TOKEN": "${ACME_TOKEN}"})
    );
    let none = manifest(json!({"notes": ["*"], "remote": ["*"]}), json!([]));
    let found = contribution("notes", Path::new(ROOT), &none, &without_townhall()).unwrap();
    assert_eq!(found.servers[0].1["env"], json!({}));
}

#[test]
fn the_townhall_key_is_refused() {
    let error = contribution("notes", Path::new(ROOT), &declared(), MCP).unwrap_err();
    assert!(error.to_string().contains("townhall"), "{error}");
}

#[test]
fn a_server_the_manifest_does_not_declare_is_refused() {
    let only = manifest(json!({"notes": ["*"]}), json!([]));
    let error = contribution("notes", Path::new(ROOT), &only, &without_townhall()).unwrap_err();
    assert!(error.to_string().contains("remote"), "{error}");
}

#[test]
fn merging_keeps_townhalls_own_server_and_adds_the_plugins() {
    let found = contribution("notes", Path::new(ROOT), &declared(), &without_townhall()).unwrap();
    let own = json!({"mcpServers": {"townhall": {"type": "http", "url": "http://x/mcp"}}});
    let merged: Value =
        serde_json::from_str(&merge(Some(&own.to_string()), std::slice::from_ref(&found))).unwrap();
    let keys: Vec<&String> = merged["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["plugin_notes_notes", "plugin_notes_remote", "townhall"]
    );
    assert_eq!(
        merged["mcpServers"]["townhall"],
        own["mcpServers"]["townhall"]
    );
    let alone: Value = serde_json::from_str(&merge(None, &[found])).unwrap();
    assert!(alone["mcpServers"].get("townhall").is_none());
    assert_eq!(merge(None, &[]), json!({"mcpServers": {}}).to_string());
}

fn planner_manifest(planner: bool) -> TownhallManifest {
    parse_townhall(
        &json!({
            "name": "notes", "version": "1.0.0", "apiVersion": 1,
            "permissions": {
                "claude": {
                    "mcp": {"servers": {
                        "notes": {"tools": ["search_notes"], "readOnly": true},
                        "remote": ["add_note"]
                    }},
                    "planner": planner
                },
                "env": ["ACME_TOKEN"]
            }
        })
        .to_string(),
    )
    .unwrap()
}

fn planner_found(planner: bool) -> Contribution {
    let manifest = planner_manifest(planner);
    let mut found = contribution("notes", Path::new(ROOT), &manifest, &without_townhall()).unwrap();
    found.plugin = "notes@acme".into();
    found
}

#[test]
fn a_plugin_without_claude_planner_gives_the_planner_nothing() {
    let manifest = planner_manifest(false);
    assert_eq!(for_planner(&manifest, "notes", planner_found(false)), None);
}

#[test]
fn the_planner_keeps_only_read_only_servers_and_their_tools() {
    let manifest = planner_manifest(true);
    let part = for_planner(&manifest, "notes", planner_found(true)).unwrap();
    let keys: Vec<&str> = part.servers.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["plugin_notes_notes"]);
    assert_eq!(part.tools, ["mcp__plugin_notes_notes__search_notes"]);
    assert_eq!(part.plugin, "notes@acme");
}

#[test]
fn a_planner_plugin_with_no_read_only_server_gives_nothing() {
    let manifest = parse_townhall(
        &json!({
            "name": "notes", "version": "1.0.0", "apiVersion": 1,
            "permissions": {"claude": {
                "mcp": {"servers": {"notes": ["add_note"], "remote": ["*"]}},
                "planner": true
            }}
        })
        .to_string(),
    )
    .unwrap();
    let found = contribution("notes", Path::new(ROOT), &manifest, &without_townhall()).unwrap();
    assert_eq!(for_planner(&manifest, "notes", found), None);
}
