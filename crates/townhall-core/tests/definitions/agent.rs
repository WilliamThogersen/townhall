use serde_json::json;
use townhall_core::agent::{AgentAdapter, ClaudeCodeAdapter, ShellAdapter};
use townhall_core::definitions::{
    check_agent, parse_agent, validate_agent, AdapterKind, AgentDef, DefKind, Effort, LaunchFields,
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
fn rule_7_bypass_permissions_is_allowed() {
    let def = parse_agent(
        "name = \"coder\"\nadapter = \"claude\"\npermission_mode = \"bypassPermissions\"\n",
    )
    .unwrap()
    .def;
    assert_eq!(def.permission_mode, Some(PermissionMode::BypassPermissions));
}

#[test]
fn rule_7_permission_modes_pass() {
    for (text, mode) in [
        ("default", PermissionMode::Default),
        ("acceptEdits", PermissionMode::AcceptEdits),
        ("plan", PermissionMode::Plan),
        ("bypassPermissions", PermissionMode::BypassPermissions),
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
        ["error: agent \"a\": line 3: permission_mode \"yolo\" is not one of default, acceptEdits, plan, bypassPermissions"]
    );
}

#[test]
fn effort_levels_parse_and_are_optional() {
    assert_eq!(parse_agent(MINIMAL).unwrap().def.effort, None);
    for (text, effort) in [
        ("low", Effort::Low),
        ("medium", Effort::Medium),
        ("high", Effort::High),
        ("xhigh", Effort::Xhigh),
        ("max", Effort::Max),
    ] {
        let source = format!("name = \"a\"\nadapter = \"claude\"\neffort = \"{text}\"\n");
        let def = parse_agent(&source).unwrap().def;
        assert_eq!(def.effort, Some(effort));
        assert_eq!(effort.as_str(), text);
        assert!(agent_errors(&source).is_empty());
    }
}

#[test]
fn unknown_effort_is_an_error() {
    assert_eq!(
        agent_errors("name = \"a\"\nadapter = \"claude\"\neffort = \"extreme\"\n"),
        ["error: agent \"a\": line 3: effort \"extreme\" is not one of low, medium, high, xhigh, max"]
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
    assert_eq!(
        AdapterKind::Claude.as_str(),
        ClaudeCodeAdapter::default().kind()
    );
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
            effort: None,
            permission_mode: Some(PermissionMode::AcceptEdits),
            allowed_tools: vec!["Read".into(), "Edit".into(), "Write".into(), "Bash".into()],
            system_prompt: def.prompt.clone(),
            command: None,
            env: Default::default(),
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
        effort: None,
        permission_mode: None,
        tools: Vec::new(),
        prompt: Some(String::new()),
        command: Some("cargo test".into()),
        can_hand_back_to: None,
        requires: Vec::new(),
        avatar: None,
        env: Default::default(),
        plugins: Vec::new(),
        merges_on_green: None,
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

#[test]
fn can_hand_back_to_defaults_to_any_type() {
    let def = parse_agent(MINIMAL).unwrap().def;
    assert_eq!(def.can_hand_back_to, None);
    let all = ["coder".to_string(), "shipper".to_string()];
    assert_eq!(def.hand_back_targets("shipper", &all), ["coder"]);
}

#[test]
fn can_hand_back_to_limits_the_targets() {
    let source = format!("{MINIMAL}can_hand_back_to = [\"coder\", \"Tester\"]\n");
    let def = check_agent(&source).unwrap().def;
    assert_eq!(
        def.can_hand_back_to.as_deref(),
        Some(&["coder".to_string(), "Tester".to_string()][..])
    );
    let all = [
        "coder".to_string(),
        "tester".to_string(),
        "shipper".to_string(),
    ];
    assert_eq!(def.hand_back_targets("a", &all), ["coder", "tester"]);
    let none = check_agent(&format!("{MINIMAL}can_hand_back_to = []\n"))
        .unwrap()
        .def;
    assert!(none.hand_back_targets("a", &all).is_empty());
}

#[test]
fn can_hand_back_to_must_hold_valid_unique_names() {
    for bad in ["[\"a b\"]", "[\"\"]", "[\"coder\", \"CODER\"]"] {
        let errors = agent_errors(&format!("{MINIMAL}can_hand_back_to = {bad}\n"));
        assert_eq!(errors.len(), 1, "{bad}: {errors:?}");
        assert!(errors[0].contains("can_hand_back_to"), "{errors:?}");
    }
}

#[test]
fn requires_defaults_to_nothing_and_is_parsed() {
    assert!(parse_agent(MINIMAL).unwrap().def.requires.is_empty());
    let source = format!("{MINIMAL}requires = [\"tester\", \"Reviewer\"]\n");
    let def = check_agent(&source).unwrap().def;
    assert_eq!(def.requires, ["tester", "Reviewer"]);
    let json = serde_json::to_value(&def).unwrap();
    assert_eq!(json["requires"][1], "Reviewer");
}

#[test]
fn requires_accepts_only_tester_and_reviewer() {
    for bad in ["[\"coder\"]", "[\"tester\", \"builder\"]", "[\"shipper\"]"] {
        let errors = agent_errors(&format!("{MINIMAL}requires = {bad}\n"));
        assert_eq!(errors.len(), 1, "{bad}: {errors:?}");
        assert!(
            errors[0].contains("requires") && errors[0].contains("cannot be required"),
            "{errors:?}"
        );
    }
    for good in ["[\"Tester\"]", "[\"reviewer\", \"TESTER\"]"] {
        let source = format!("{MINIMAL}requires = {good}\n");
        assert!(check_agent(&source).is_ok(), "{good}");
    }
}

#[test]
fn requires_must_hold_valid_unique_names() {
    for bad in ["[\"a b\"]", "[\"\"]", "[\"tester\", \"TESTER\"]"] {
        let errors = agent_errors(&format!("{MINIMAL}requires = {bad}\n"));
        assert_eq!(errors.len(), 1, "{bad}: {errors:?}");
        assert!(errors[0].contains("requires"), "{errors:?}");
    }
}

#[test]
fn env_defaults_to_empty_and_parses_a_table() {
    assert!(parse_agent(MINIMAL).unwrap().def.env.is_empty());
    let source = format!("{MINIMAL}\n[env]\nA_VAR = \"1\"\nB_VAR = \"two words\"\n");
    let def = check_agent(&source).unwrap().def;
    assert_eq!(def.env.get("A_VAR").map(String::as_str), Some("1"));
    assert_eq!(def.env.get("B_VAR").map(String::as_str), Some("two words"));
    assert_eq!(def.launch_fields().env, def.env);
}

#[test]
fn env_rejects_empty_keys_and_keys_with_equals_without_echoing_values() {
    for key in ["\"\"", "\"A=B\""] {
        let errors = agent_errors(&format!("{MINIMAL}\n[env]\n{key} = \"hunter2\"\n"));
        assert_eq!(errors.len(), 1, "{key}: {errors:?}");
        assert!(errors[0].contains("env"), "{errors:?}");
        assert!(!errors[0].contains("hunter2"), "{errors:?}");
    }
}
