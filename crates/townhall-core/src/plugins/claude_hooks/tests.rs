use std::path::Path;

use serde_json::{json, Value};

use super::{contribution, merge};
use crate::plugins::manifest::{parse_townhall, TownhallManifest};

const HOOKS: &str = include_str!("../../../tests/fixtures/plugins/claude/hooks.json");
const ROOT: &str = "/h/plugins/cache/acme/fmt/abc";

fn manifest(events: Value) -> TownhallManifest {
    parse_townhall(
        &json!({
            "name": "fmt", "version": "1.0.0", "apiVersion": 1,
            "permissions": {"claude": {"hooks": {"events": events}}}
        })
        .to_string(),
    )
    .unwrap()
}

fn post_only() -> String {
    let mut all: Value = serde_json::from_str(HOOKS).unwrap();
    all["hooks"].as_object_mut().unwrap().remove("PreToolUse");
    all.to_string()
}

fn post_contribution() -> super::Contribution {
    contribution(
        Path::new(ROOT),
        &manifest(json!(["PostToolUse"])),
        &post_only(),
    )
    .unwrap()
}

#[test]
fn only_the_declared_events_are_kept_with_the_plugin_root_replaced() {
    let found = post_contribution();
    let names: Vec<&str> = found.events.iter().map(|(e, _)| e.as_str()).collect();
    assert_eq!(names, ["PostToolUse"]);
    let text = found.events[0].1[0].to_string();
    assert!(text.contains(&format!("{ROOT}/hooks/format.sh")), "{text}");
    assert!(!text.contains("CLAUDE_PLUGIN_ROOT"));
}

#[test]
fn an_event_the_manifest_did_not_declare_is_refused() {
    let error = contribution(
        Path::new(ROOT),
        &manifest(json!(["Stop"])),
        &post_only(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("PostToolUse"), "{error}");
}

#[test]
fn pre_tool_use_is_not_offered_even_when_declared() {
    let error = contribution(
        Path::new(ROOT),
        &manifest(json!(["PostToolUse", "PreToolUse"])),
        HOOKS,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("PreToolUse"), "{error}");
}

#[test]
fn the_merged_settings_have_no_disable_all_hooks_and_no_memory() {
    let found = post_contribution();
    let text = merge(&[found.clone(), found]).unwrap();
    let settings: Value = serde_json::from_str(&text).unwrap();
    assert!(settings.get("disableAllHooks").is_none());
    assert_eq!(settings["autoMemoryEnabled"], json!(false));
    assert_eq!(settings["hooks"].as_object().unwrap().len(), 1);
    assert_eq!(settings["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);
}

#[test]
fn nothing_to_merge_is_no_settings() {
    assert_eq!(merge(&[]), None);
    let none = contribution(
        Path::new(ROOT),
        &manifest(json!(["PostToolUse"])),
        r#"{"hooks": {}}"#,
    )
    .unwrap();
    assert_eq!(merge(&[none]), None);
}
