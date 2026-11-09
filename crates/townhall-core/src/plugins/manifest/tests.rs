use serde_json::{json, Value};

use super::*;
use crate::Error;

const FULL: &str = include_str!("../../../tests/fixtures/plugins/townhall/release-notes.json");

fn base() -> Value {
    json!({ "name": "demo", "version": "1.0.0", "apiVersion": 1 })
}

fn with(key: &str, value: Value) -> Value {
    let mut manifest = base();
    manifest[key] = value;
    manifest
}

fn refused(manifest: &Value) -> String {
    match parse_townhall(&manifest.to_string()) {
        Ok(parsed) => panic!("accepted {manifest}: {parsed:?}"),
        Err(e) => {
            assert!(matches!(e, Error::InvalidInput(_)), "{e:?}");
            e.to_string()
        }
    }
}

#[test]
fn a_full_townhall_manifest_decodes_every_field() {
    let m = parse_townhall(FULL).unwrap();
    assert_eq!(m.name, "release-notes");
    assert_eq!(m.version, "0.3.0");
    assert_eq!(
        m.description.as_deref(),
        Some("Writes release notes when a task is merged")
    );
    let author = m.author.unwrap();
    assert_eq!(author.name, "Acme");
    assert_eq!(author.url.as_deref(), Some("https://acme.example"));
    assert_eq!(author.email, None);
    assert_eq!(
        m.homepage.as_deref(),
        Some("https://github.com/acme/townhall-release-notes")
    );
    assert_eq!(m.license.as_deref(), Some("MIT"));
    assert_eq!(m.api_version, 1);
    assert_eq!(m.min_townhall.as_deref(), Some("0.5.0"));

    let p = m.permissions;
    assert_eq!(p.api, ["tasks:read", "tasks:create", "storage"]);
    assert_eq!(p.hooks, ["task.merged", "pr.before_open"]);
    let hooks = p.claude.hooks.unwrap();
    assert_eq!(hooks.events, ["PostToolUse"]);
    assert_eq!(hooks.agent_types, ["release-writer"]);
    let servers = p.claude.mcp.unwrap().servers;
    assert_eq!(
        servers["notes"],
        McpTools::List(vec!["search_notes".into(), "add_note".into()])
    );
    assert!(!p.claude.planner);
    assert_eq!(p.prompts, ["starters", "brief"]);
    assert_eq!(p.env, ["ACME_TOKEN"]);
    assert_eq!(p.ui, ["settings.group", "session.section"]);

    let c = m.contributes;
    assert_eq!(c.agent_types, ["agent-types/release-writer.toml"]);
    assert_eq!(c.prompts["starters"], "prompts/starters.json");
    assert_eq!(c.prompts["brief"], "prompts/brief.md");
    assert_eq!(c.settings.as_deref(), Some("settings.schema.json"));
    assert_eq!(c.theme.as_deref(), Some("theme.css"));
    assert_eq!(c.hooks.as_deref(), Some("townhall-hooks.json"));
    assert_eq!(c.ui.len(), 2);
    assert_eq!(c.ui[0].slot, "session.section");
    assert_eq!(c.ui[0].title, "release notes");
    assert_eq!(c.ui[0].entry, "ui/session.html");
    assert_eq!(c.ui[0].height, Some(160));
    assert_eq!(c.ui[1].height, None);

    assert_eq!(m.claude.hooks.as_deref(), Some("hooks/hooks.json"));
    assert_eq!(m.claude.mcp.as_deref(), Some(".mcp.json"));
}

#[test]
fn a_minimal_manifest_defaults_everything_else() {
    let m = parse_townhall(&base().to_string()).unwrap();
    assert_eq!(m.permissions, Permissions::default());
    assert_eq!(m.contributes, Contributes::default());
    assert_eq!(m.claude, ClaudeFiles::default());
    assert_eq!(m.author, None);
}

#[test]
fn mcp_servers_may_mark_tools_read_only() {
    let manifest = with(
        "permissions",
        json!({ "claude": { "mcp": { "servers": {
            "notes": { "tools": ["search_notes"], "readOnly": true },
            "all": ["*"]
        } }, "planner": true } }),
    );
    let m = parse_townhall(&manifest.to_string()).unwrap();
    let servers = m.permissions.claude.mcp.unwrap().servers;
    assert_eq!(
        servers["notes"],
        McpTools::Detailed(McpToolsDetail {
            tools: vec!["search_notes".into()],
            read_only: true
        })
    );
    assert_eq!(servers["all"], McpTools::List(vec!["*".into()]));
    assert!(m.permissions.claude.planner);
}

#[test]
fn claude_api_scopes_decode() {
    let manifest = with(
        "permissions",
        json!({ "claude": { "api": ["tasks:read"] } }),
    );
    let m = parse_townhall(&manifest.to_string()).unwrap();
    assert_eq!(m.permissions.claude.api, ["tasks:read"]);
}

#[test]
fn real_claude_plugin_manifests_decode() {
    let cases = [
        (
            include_str!("../../../tests/fixtures/plugins/plugins/typesafe.json"),
            "typesafe",
            Some("0.5.7"),
        ),
        (
            include_str!("../../../tests/fixtures/plugins/plugins/sample-team.json"),
            "sample-team",
            Some("0.1.2"),
        ),
        (
            include_str!("../../../tests/fixtures/plugins/plugins/ddev.json"),
            "ddev",
            Some("1.0.0"),
        ),
        (
            include_str!("../../../tests/fixtures/plugins/plugins/agent-browser.json"),
            "agent-browser",
            Some("1.0.0"),
        ),
    ];
    for (text, name, version) in cases {
        let m = parse_claude(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(m.name, name);
        assert_eq!(m.version.as_deref(), version);
        assert!(m.description.is_some(), "{name}");
        assert!(m.author.is_some(), "{name}");
    }
}

#[test]
fn a_claude_manifest_keeps_its_own_fields() {
    let m = parse_claude(include_str!(
        "../../../tests/fixtures/plugins/plugins/typesafe.json"
    ))
    .unwrap();
    assert_eq!(m.homepage.as_deref(), Some("https://typesafe.ai"));
    assert_eq!(
        m.repository.as_deref(),
        Some("https://github.com/typesafe-ai/skills")
    );
    assert_eq!(m.license.as_deref(), Some("MIT"));
    let ddev = parse_claude(include_str!(
        "../../../tests/fixtures/plugins/plugins/ddev.json"
    ))
    .unwrap();
    assert_eq!(
        ddev.author.unwrap().email.as_deref(),
        Some("dev@example.com")
    );
}

#[test]
fn an_unknown_top_level_key_is_refused() {
    let why = refused(&with("permisions", json!({})));
    assert!(why.contains("permisions"), "{why}");
}

#[test]
fn an_unknown_key_in_permissions_is_refused() {
    let why = refused(&with("permissions", json!({ "hook": ["task.merged"] })));
    assert!(why.contains("hook"), "{why}");
}

#[test]
fn an_unknown_key_in_claude_permissions_is_refused() {
    let why = refused(&with("permissions", json!({ "claude": { "plan": true } })));
    assert!(why.contains("plan"), "{why}");
}

#[test]
fn an_unknown_key_in_contributes_is_refused() {
    let why = refused(&with("contributes", json!({ "themes": "t.css" })));
    assert!(why.contains("themes"), "{why}");
}

#[test]
fn an_unknown_key_in_a_ui_entry_is_refused() {
    let manifest = json!({
        "name": "demo", "version": "1.0.0", "apiVersion": 1,
        "permissions": { "ui": ["page"] },
        "contributes": { "ui": [{ "slot": "page", "title": "t", "entry": "a.html", "width": 1 }] }
    });
    let why = refused(&manifest);
    assert!(why.contains("width"), "{why}");
}

#[test]
fn an_unknown_key_in_the_claude_section_is_refused() {
    let why = refused(&with("claude", json!({ "commands": "commands" })));
    assert!(why.contains("commands"), "{why}");
}

#[test]
fn an_unknown_key_in_a_claude_manifest_is_refused() {
    let err = parse_claude(r#"{"name":"x","hooks":"hooks.json"}"#).unwrap_err();
    assert!(err.to_string().contains("hooks"), "{err}");
}

#[test]
fn a_parent_directory_path_is_refused() {
    for path in ["../x.css", "a/../../x.css", "a/..", ".."] {
        let why = refused(&with("contributes", json!({ "theme": path })));
        assert!(why.contains("`..`"), "{path}: {why}");
    }
}

#[test]
fn an_absolute_path_is_refused() {
    for path in ["/etc/passwd", "\\\\host\\share", "C:\\x", "C:/x"] {
        let why = refused(&with("contributes", json!({ "theme": path })));
        assert!(why.contains("relative"), "{path}: {why}");
    }
}

#[test]
fn a_backslash_path_is_refused() {
    let why = refused(&with("contributes", json!({ "theme": "a\\b.css" })));
    assert!(why.contains("relative"), "{why}");
}

#[test]
fn an_empty_path_is_refused() {
    let why = refused(&with("claude", json!({ "hooks": "" })));
    assert!(why.contains("empty"), "{why}");
}

#[test]
fn every_kind_of_path_is_checked() {
    let bad = "../x";
    let granted = json!({ "prompts": ["brief"], "ui": ["page"] });
    let with_grants = |key: &str, value: Value| {
        let mut manifest = with(key, value);
        manifest["permissions"] = granted.clone();
        manifest
    };
    let cases = [
        with_grants("contributes", json!({ "agentTypes": [bad] })),
        with_grants("contributes", json!({ "prompts": { "brief": bad } })),
        with_grants("contributes", json!({ "settings": bad })),
        with_grants("contributes", json!({ "theme": bad })),
        with_grants("contributes", json!({ "hooks": bad })),
        with_grants("claude", json!({ "hooks": bad })),
        with_grants("claude", json!({ "mcp": bad })),
        with_grants(
            "contributes",
            json!({ "ui": [{ "slot": "page", "title": "t", "entry": bad }] }),
        ),
    ];
    for manifest in cases {
        let why = refused(&manifest);
        assert!(why.contains("`..`"), "{manifest}: {why}");
    }
}

#[test]
fn a_bad_plugin_name_is_refused() {
    for name in ["", "-x", ".x", "a b", "a/b", "a@b", &"a".repeat(65)] {
        let mut manifest = base();
        manifest["name"] = json!(name);
        let why = refused(&manifest);
        assert!(why.contains("name"), "{name}: {why}");
    }
}

#[test]
fn a_version_that_is_not_semver_is_refused() {
    for version in ["1", "1.0", "v1.0.0", "01.0.0", "1.0.0.0", "1.0.x", ""] {
        let mut manifest = base();
        manifest["version"] = json!(version);
        let why = refused(&manifest);
        assert!(why.contains("version"), "{version}: {why}");
    }
}

#[test]
fn prerelease_and_build_versions_are_accepted() {
    for version in ["1.0.0-rc.1", "1.0.0+build.5", "0.0.1-alpha+x"] {
        let mut manifest = base();
        manifest["version"] = json!(version);
        parse_townhall(&manifest.to_string()).unwrap_or_else(|e| panic!("{version}: {e}"));
    }
}

#[test]
fn a_min_townhall_that_is_not_semver_is_refused() {
    let why = refused(&with("minTownhall", json!("0.5")));
    assert!(why.contains("minTownhall"), "{why}");
}

#[test]
fn an_api_version_of_zero_is_refused() {
    let why = refused(&with("apiVersion", json!(0)));
    assert!(why.contains("apiVersion"), "{why}");
}

#[test]
fn a_missing_api_version_is_refused() {
    let manifest = json!({ "name": "demo", "version": "1.0.0" });
    let why = refused(&manifest);
    assert!(why.contains("apiVersion"), "{why}");
}

#[test]
fn a_missing_name_is_refused() {
    let why = refused(&json!({ "version": "1.0.0", "apiVersion": 1 }));
    assert!(why.contains("name"), "{why}");
}

#[test]
fn an_unknown_api_scope_is_refused() {
    let why = refused(&with("permissions", json!({ "api": ["tasks:delete"] })));
    assert!(why.contains("tasks:delete"), "{why}");
}

#[test]
fn an_unknown_claude_api_scope_is_refused() {
    let why = refused(&with(
        "permissions",
        json!({ "claude": { "api": ["everything"] } }),
    ));
    assert!(why.contains("everything"), "{why}");
}

#[test]
fn an_unknown_townhall_hook_event_is_refused() {
    let why = refused(&with("permissions", json!({ "hooks": ["task.exploded"] })));
    assert!(why.contains("task.exploded"), "{why}");
}

#[test]
fn an_unknown_ui_slot_is_refused() {
    let why = refused(&with("permissions", json!({ "ui": ["everywhere"] })));
    assert!(why.contains("everywhere"), "{why}");
}

#[test]
fn an_unknown_prompt_kind_is_refused() {
    let why = refused(&with("permissions", json!({ "prompts": ["system"] })));
    assert!(why.contains("system"), "{why}");
}

#[test]
fn a_bad_env_name_is_refused() {
    for name in ["", "A=B", "1A", "A B"] {
        let why = refused(&with("permissions", json!({ "env": [name] })));
        assert!(why.contains("env"), "{name}: {why}");
    }
}

#[test]
fn a_bad_claude_hook_event_name_is_refused() {
    let why = refused(&with(
        "permissions",
        json!({ "claude": { "hooks": { "events": ["Post Tool"] } } }),
    ));
    assert!(why.contains("Post Tool"), "{why}");
}

#[test]
fn a_bad_agent_type_name_in_claude_hooks_is_refused() {
    let why = refused(&with(
        "permissions",
        json!({ "claude": { "hooks": { "events": ["PostToolUse"], "agentTypes": ["a/b"] } } }),
    ));
    assert!(why.contains("a/b"), "{why}");
}

#[test]
fn a_bad_mcp_server_or_tool_name_is_refused() {
    let server = with(
        "permissions",
        json!({ "claude": { "mcp": { "servers": { "a b": ["t"] } } } }),
    );
    assert!(refused(&server).contains("a b"));
    let tool = with(
        "permissions",
        json!({ "claude": { "mcp": { "servers": { "notes": ["a b"] } } } }),
    );
    assert!(refused(&tool).contains("a b"));
}

#[test]
fn the_reserved_townhall_mcp_server_name_is_refused() {
    let why = refused(&with(
        "permissions",
        json!({ "claude": { "mcp": { "servers": { "townhall": ["*"] } } } }),
    ));
    assert!(why.contains("townhall"), "{why}");
}

#[test]
fn a_ui_slot_without_its_permission_is_refused() {
    let manifest = with(
        "contributes",
        json!({ "ui": [{ "slot": "page", "title": "t", "entry": "a.html" }] }),
    );
    let why = refused(&manifest);
    assert!(why.contains("permissions.ui"), "{why}");
}

#[test]
fn a_ui_entry_with_an_unknown_slot_is_refused() {
    let manifest = json!({
        "name": "demo", "version": "1.0.0", "apiVersion": 1,
        "permissions": { "ui": ["page"] },
        "contributes": { "ui": [{ "slot": "nowhere", "title": "t", "entry": "a.html" }] }
    });
    let why = refused(&manifest);
    assert!(why.contains("nowhere"), "{why}");
}

#[test]
fn a_ui_height_over_the_limit_is_refused() {
    let manifest = json!({
        "name": "demo", "version": "1.0.0", "apiVersion": 1,
        "permissions": { "ui": ["session.section"] },
        "contributes": { "ui": [{ "slot": "session.section", "title": "t", "entry": "a.html", "height": 401 }] }
    });
    let why = refused(&manifest);
    assert!(why.contains("height"), "{why}");
}

#[test]
fn a_prompt_without_its_permission_is_refused() {
    let manifest = with(
        "contributes",
        json!({ "prompts": { "planner": "prompts/planner.md" } }),
    );
    let why = refused(&manifest);
    assert!(why.contains("permissions.prompts"), "{why}");
}

#[test]
fn a_contributed_prompt_of_an_unknown_kind_is_refused() {
    let manifest = json!({
        "name": "demo", "version": "1.0.0", "apiVersion": 1,
        "permissions": { "prompts": ["starters"] },
        "contributes": { "prompts": { "system": "s.md" } }
    });
    let why = refused(&manifest);
    assert!(why.contains("system"), "{why}");
}

#[test]
fn a_homepage_that_is_not_a_web_address_is_refused() {
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "acme.example",
        "https://a b",
    ] {
        let why = refused(&with("homepage", json!(url)));
        assert!(why.contains("homepage"), "{url}: {why}");
    }
}

#[test]
fn an_author_url_that_is_not_a_web_address_is_refused() {
    let why = refused(&with(
        "author",
        json!({ "name": "A", "url": "ftp://a.example" }),
    ));
    assert!(why.contains("author.url"), "{why}");
}

#[test]
fn an_author_without_a_name_is_refused() {
    let why = refused(&with("author", json!({ "url": "https://a.example" })));
    assert!(why.contains("name"), "{why}");
}

#[test]
fn an_oversize_manifest_is_refused() {
    let padding = "x".repeat(MAX_MANIFEST_BYTES);
    let text = with("description", json!(padding)).to_string();
    let err = parse_townhall(&text).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
    let err = parse_claude(&text).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err:?}");
}

#[test]
fn a_manifest_of_exactly_the_limit_is_read() {
    let head = r#"{"name":"demo","version":"1.0.0","apiVersion":1,"description":""#;
    let tail = r#""}"#;
    let fill = "x".repeat(MAX_MANIFEST_BYTES - head.len() - tail.len());
    let text = format!("{head}{fill}{tail}");
    assert_eq!(text.len(), MAX_MANIFEST_BYTES);
    parse_townhall(&text).unwrap();
}

#[test]
fn text_that_is_not_json_is_refused() {
    for text in ["", "name = 'x'", "[]", "null", "{\"name\":"] {
        let err = parse_townhall(text).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{text}: {err:?}");
    }
}

#[test]
fn a_claude_manifest_with_a_bad_name_or_version_is_refused() {
    assert!(parse_claude(r#"{"name":"a b"}"#).is_err());
    assert!(parse_claude(r#"{"name":"a","version":"one"}"#).is_err());
}
