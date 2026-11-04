use serde_json::json;
use townhall_core::agent::{AgentAdapter, ClaudeCodeAdapter, ShellAdapter};
use townhall_core::definitions::{
    check_agent, parse_agent, validate_agent, AdapterKind, AgentDef, DefKind, LaunchFields,
    PermissionMode,
};

use crate::support::{agent_errors, CODER};

const MINIMAL: &str = "name = \"a\"\nadapter = \"claude\"\n";

#[test]
fn parses_the_plan_coder_and_keeps_its_source() {
    let parsed = parse_agent(CODER).unwrap();
    assert_eq!(parsed.source, CODER);
    assert!(parsed.source.starts_with("# the coder"));
    let def = parsed.def;
    assert_eq!(def.name, "coder");
    assert_eq!(def.description, "implements a spec, tests first");
    assert_eq!(def.adapter, AdapterKind::Claude);
    assert_eq!(def.model.as_deref(), Some("sonnet"));
    assert_eq!(def.permission_mode, Some(PermissionMode::AcceptEdits));
    assert_eq!(def.tools, ["Read", "Edit", "Write", "Bash"]);
    assert_eq!(
        def.prompt.as_deref(),
        Some("You implement the spec you are given.\nWrite tests first. Commit when the tests pass.\n")
    );
    assert_eq!(def.command, None);
    assert_eq!(validate_agent(&def), Ok(()));
}

#[test]
fn optional_fields_default_to_empty() {
    let def = parse_agent(MINIMAL).unwrap().def;
    assert_eq!(def.description, "");
    assert_eq!(def.model, None);
    assert_eq!(def.permission_mode, None);
    assert!(def.tools.is_empty());
    assert_eq!(def.prompt, None);
    assert!(agent_errors(MINIMAL).is_empty());
}

#[test]
fn rule_1_unknown_field_is_an_error() {
    let errors = agent_errors("name = \"coder\"\nadapter = \"claude\"\nmodle = \"opus\"\n");
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].starts_with("error: agent \"coder\": line 3: unknown field `modle`"),
        "{errors:?}"
    );
}

#[test]
fn rule_1_toml_is_snake_case_only() {
    let errors = agent_errors("name = \"a\"\nadapter = \"claude\"\npermissionMode = \"plan\"\n");
    assert!(
        errors[0].contains("unknown field `permissionMode`"),
        "{errors:?}"
    );
}

#[test]
fn rule_1_broken_toml_is_an_error_without_a_name() {
    let errors = agent_errors("name = \"a\"\nadapter = \n");
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].starts_with("error: agent: line 2: "),
        "{errors:?}"
    );
}

#[test]
fn rule_1_missing_required_field_is_an_error() {
    let errors = agent_errors("name = \"a\"\n");
    assert!(errors[0].starts_with("error: agent \"a\": "), "{errors:?}");
    assert!(errors[0].contains("missing field `adapter`"), "{errors:?}");
}

#[test]
fn unknown_adapter_is_an_error() {
    let errors = agent_errors("name = \"a\"\nadapter = \"codex\"\n");
    assert!(
        errors[0].starts_with("error: agent \"a\": line 2: "),
        "{errors:?}"
    );
    assert!(errors[0].contains("codex"), "{errors:?}");
}

#[test]
fn rule_2_agent_name_must_be_valid() {
    assert!(agent_errors("name = \"code-r_2\"\nadapter = \"claude\"\n").is_empty());
    assert_eq!(
        agent_errors("name = \"my coder\"\nadapter = \"claude\"\n"),
        ["error: agent \"my coder\": name \"my coder\" is not valid: use letters, digits, - and _ only"]
    );
}

#[test]
fn rule_7_bypass_permissions_is_rejected() {
    assert_eq!(
        agent_errors("name = \"coder\"\nadapter = \"claude\"\npermission_mode = \"bypassPermissions\"\n"),
        ["error: agent \"coder\": line 3: permission_mode \"bypassPermissions\" is not allowed in a definition: skip all is set only in settings"]
    );
}

#[test]
fn rule_7_safe_permission_modes_pass() {
    for (text, mode) in [
        ("default", PermissionMode::Default),
        ("acceptEdits", PermissionMode::AcceptEdits),
        ("plan", PermissionMode::Plan),
    ] {
        let source = format!("name = \"a\"\nadapter = \"claude\"\npermission_mode = \"{text}\"\n");
        let def = parse_agent(&source).unwrap().def;
        assert_eq!(def.permission_mode, Some(mode));
        assert_eq!(mode.as_str(), text);
    }
}

#[test]
fn unknown_permission_mode_is_an_error() {
    assert_eq!(
        agent_errors("name = \"a\"\nadapter = \"claude\"\npermission_mode = \"yolo\"\n"),
        ["error: agent \"a\": line 3: permission_mode \"yolo\" is not one of default, acceptEdits, plan"]
    );
}

#[test]
fn shell_needs_a_command() {
    assert!(
        agent_errors("name = \"s\"\nadapter = \"shell\"\ncommand = \"cargo test\"\n").is_empty()
    );
    let expected = ["error: agent \"s\": command is required for the shell adapter"];
    assert_eq!(
        agent_errors("name = \"s\"\nadapter = \"shell\"\n"),
        expected
    );
    assert_eq!(
        agent_errors("name = \"s\"\nadapter = \"shell\"\ncommand = \"  \"\n"),
        expected
    );
}

#[test]
fn claude_refuses_a_command() {
    assert_eq!(
        agent_errors("name = \"c\"\nadapter = \"claude\"\ncommand = \"ls\"\n"),
        ["error: agent \"c\": command is only for the shell adapter"]
    );
}

#[test]
fn every_agent_error_is_returned() {
    let def = parse_agent("name = \"bad name\"\nadapter = \"claude\"\ncommand = \"ls\"\n")
        .unwrap()
        .def;
    let errors = validate_agent(&def).unwrap_err();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].kind, DefKind::Agent);
    assert_eq!(errors[0].field.as_deref(), Some("name"));
    assert_eq!(errors[1].field.as_deref(), Some("command"));
}

#[test]
fn adapter_kinds_match_the_adapters() {
    assert_eq!(AdapterKind::Claude.as_str(), ClaudeCodeAdapter.kind());
    assert_eq!(AdapterKind::Shell.as_str(), ShellAdapter.kind());
}

#[test]
fn serializes_to_camel_case_json() {
    let def = parse_agent(CODER).unwrap().def;
    let value = serde_json::to_value(&def).unwrap();
    assert_eq!(value["permissionMode"], json!("acceptEdits"));
    assert_eq!(value["adapter"], json!("claude"));
    assert_eq!(value["tools"], json!(["Read", "Edit", "Write", "Bash"]));
    assert_eq!(value["command"], json!(null));
    assert!(value.get("permission_mode").is_none());
}

#[test]
fn launch_fields_carry_the_definition() {
    let def = parse_agent(CODER).unwrap().def;
    assert_eq!(
        def.launch_fields(),
        LaunchFields {
            model: Some("sonnet".into()),
            permission_mode: Some(PermissionMode::AcceptEdits),
            allowed_tools: vec!["Read".into(), "Edit".into(), "Write".into(), "Bash".into()],
            system_prompt: def.prompt.clone(),
            command: None,
        }
    );
}

#[test]
fn launch_fields_drop_empty_text() {
    let def = AgentDef {
        name: "s".into(),
        description: String::new(),
        adapter: AdapterKind::Shell,
        model: Some(" ".into()),
        permission_mode: None,
        tools: Vec::new(),
        prompt: Some(String::new()),
        command: Some("cargo test".into()),
    };
    let fields = def.launch_fields();
    assert_eq!(fields.model, None);
    assert_eq!(fields.system_prompt, None);
    assert_eq!(fields.command.as_deref(), Some("cargo test"));
}

#[test]
fn check_agent_parses_and_validates() {
    assert_eq!(check_agent(CODER).unwrap().def.name, "coder");
    let parse_failure = check_agent("name = \"a\"\n").unwrap_err();
    assert_eq!(parse_failure.len(), 1);
    let rule_failure = check_agent("name = \"a b\"\nadapter = \"shell\"\n").unwrap_err();
    assert_eq!(rule_failure.len(), 2);
}
