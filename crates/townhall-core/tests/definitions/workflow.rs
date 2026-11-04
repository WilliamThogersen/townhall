use serde_json::json;
use townhall_core::definitions::{
    check_workflow, parse_workflow, validate_workflow, DefKind, DefinitionError,
};

use crate::support::{workflow_errors, KNOWN, SHIP};

#[test]
fn parses_the_plan_ship_and_keeps_its_source() {
    let parsed = parse_workflow(SHIP).unwrap();
    assert_eq!(parsed.source, SHIP);
    assert!(parsed.source.starts_with("# the ship workflow"));
    let def = parsed.def;
    assert_eq!(def.name, "ship");
    assert_eq!(
        def.description,
        "plan, build, then test and review in parallel"
    );
    let ids: Vec<&str> = def.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["plan", "build", "test", "review"]);
    let test = &def.steps[2];
    assert_eq!(test.agent, "tester");
    assert_eq!(test.after, ["build"]);
    assert_eq!(test.from.as_deref(), Some("build"));
    assert_eq!(
        test.prompt.as_deref(),
        Some("Test the changes on this branch against the spec:\n\n{{steps.plan.result}}")
    );
    assert!(def.steps[0].after.is_empty());
    assert_eq!(def.steps[0].from, None);
    assert_eq!(validate_workflow(&def, &KNOWN), Ok(()));
}

#[test]
fn lists_the_agents_it_uses_once_each() {
    let def = parse_workflow(SHIP).unwrap().def;
    assert_eq!(
        def.agent_names(),
        ["planner", "coder", "tester", "reviewer"]
    );
    let twice = crate::support::workflow(&[("a", &[], ""), ("b", &["a"], "")]);
    assert_eq!(parse_workflow(&twice).unwrap().def.agent_names(), ["coder"]);
}

#[test]
fn serializes_to_camel_case_json_with_steps() {
    let def = parse_workflow(SHIP).unwrap().def;
    let value = serde_json::to_value(&def).unwrap();
    assert_eq!(value["name"], json!("ship"));
    assert!(value.get("step").is_none());
    assert_eq!(value["steps"][2]["from"], json!("build"));
    assert_eq!(value["steps"][2]["after"], json!(["build"]));
    assert_eq!(value["steps"][0]["from"], json!(null));
}

#[test]
fn toml_uses_step_not_steps() {
    let errors = workflow_errors("name = \"w\"\n[[steps]]\nid = \"a\"\nagent = \"coder\"\n");
    assert!(errors[0].contains("unknown field `steps`"), "{errors:?}");
}

#[test]
fn error_text_names_definition_step_and_field() {
    let source = SHIP.replace(
        "from = \"build\"\nprompt = \"Test",
        "from = \"deploy\"\nprompt = \"Test",
    );
    assert_eq!(
        workflow_errors(&source),
        ["error: workflow \"ship\" step \"test\": from \"deploy\" is not a step"]
    );
}

#[test]
fn errors_carry_their_parts() {
    let source = SHIP.replace(
        "from = \"build\"\nprompt = \"Test",
        "from = \"deploy\"\nprompt = \"Test",
    );
    let errors = check_workflow(&source, &KNOWN).unwrap_err();
    assert_eq!(
        errors,
        [DefinitionError {
            kind: DefKind::Workflow,
            name: Some("ship".into()),
            step: Some("test".into()),
            field: Some("from".into()),
            message: "\"deploy\" is not a step".into(),
            line: None,
        }]
    );
    assert_eq!(
        serde_json::to_value(&errors[0]).unwrap(),
        json!({
            "kind": "workflow",
            "name": "ship",
            "step": "test",
            "field": "from",
            "message": "\"deploy\" is not a step",
            "line": null,
        })
    );
}

#[test]
fn parse_errors_carry_a_line() {
    let source = SHIP.replace(
        "from = \"build\"\nprompt = \"Test",
        "form = \"build\"\nprompt = \"Test",
    );
    let errors = check_workflow(&source, &KNOWN).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, Some(20));
    assert_eq!(errors[0].name.as_deref(), Some("ship"));
    assert!(
        errors[0]
            .to_string()
            .starts_with("error: workflow \"ship\": line 20: unknown field `form`"),
        "{errors:?}"
    );
}

#[test]
fn check_workflow_returns_the_parsed_source() {
    let parsed = check_workflow(SHIP, &KNOWN).unwrap();
    assert_eq!(parsed.source, SHIP);
    assert_eq!(parsed.def.steps.len(), 4);
}
