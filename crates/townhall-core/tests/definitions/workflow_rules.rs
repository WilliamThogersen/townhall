//! One passing and one failing workflow per validation rule.

use crate::support::{workflow, workflow_errors};

fn none() -> Vec<String> {
    Vec::new()
}

#[test]
fn rule_1_unknown_workflow_field_is_an_error() {
    assert_eq!(workflow_errors(&workflow(&[("a", &[], "")])), none());
    let errors = workflow_errors(&format!(
        "descripton = \"x\"\n{}",
        workflow(&[("a", &[], "")])
    ));
    assert!(
        errors[0].starts_with("error: workflow \"w\": line 1: unknown field `descripton`"),
        "{errors:?}"
    );
}

#[test]
fn rule_1_unknown_step_field_is_an_error() {
    let errors = workflow_errors(&workflow(&[("a", &[], "agnet = \"coder\"")]));
    assert!(errors[0].contains("unknown field `agnet`"), "{errors:?}");
}

#[test]
fn rule_2_workflow_name_must_be_valid() {
    let source = workflow(&[("a", &[], "")]).replace("name = \"w\"", "name = \"my flow\"");
    assert_eq!(
        workflow_errors(&source),
        ["error: workflow \"my flow\": name \"my flow\" is not valid: use letters, digits, - and _ only"]
    );
}

#[test]
fn rule_2_step_id_must_be_valid() {
    assert_eq!(workflow_errors(&workflow(&[("step_1-a", &[], "")])), none());
    assert_eq!(
        workflow_errors(&workflow(&[("my step", &[], "")])),
        ["error: workflow \"w\" step \"my step\": id \"my step\" is not valid: use letters, digits, - and _ only"]
    );
}

#[test]
fn step_ids_are_unique() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], ""), ("a", &[], "")])),
        ["error: workflow \"w\" step \"a\": id \"a\" is used by more than one step"]
    );
}

#[test]
fn a_workflow_has_steps() {
    assert_eq!(
        workflow_errors("name = \"w\"\n"),
        ["error: workflow \"w\": step is missing: add at least one [[step]]"]
    );
}

#[test]
fn rule_3_agent_must_be_saved() {
    let source = workflow(&[("a", &[], "")]).replace("\"coder\"", "\"ghost\"");
    assert_eq!(
        workflow_errors(&source),
        ["error: workflow \"w\" step \"a\": agent \"ghost\" is not a saved agent"]
    );
}

#[test]
fn rule_3_after_must_name_a_step() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], ""), ("b", &["a"], "")])),
        none()
    );
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], ""), ("b", &["z"], "")])),
        ["error: workflow \"w\" step \"b\": after \"z\" is not a step"]
    );
}

#[test]
fn rule_3_from_must_name_a_step() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], ""), ("b", &["a"], "from = \"z\"")])),
        ["error: workflow \"w\" step \"b\": from \"z\" is not a step"]
    );
}

#[test]
fn rule_4_after_has_no_cycles() {
    let chain = workflow(&[("a", &[], ""), ("b", &["a"], ""), ("c", &["b"], "")]);
    assert_eq!(workflow_errors(&chain), none());
    assert_eq!(
        workflow_errors(&workflow(&[("a", &["b"], ""), ("b", &["a"], "")])),
        ["error: workflow \"w\" step \"a\": after makes a cycle: a waits for b, b waits for a"]
    );
}

#[test]
fn rule_4_a_step_cannot_wait_for_itself() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &["a"], "")])),
        ["error: workflow \"w\" step \"a\": after makes a cycle: a waits for a"]
    );
}

#[test]
fn rule_4_longer_cycles_are_named_in_full() {
    let source = workflow(&[
        ("x", &[], ""),
        ("a", &["c", "x"], ""),
        ("b", &["a"], ""),
        ("c", &["b"], ""),
    ]);
    assert_eq!(
        workflow_errors(&source),
        ["error: workflow \"w\" step \"a\": after makes a cycle: a waits for c, c waits for b, b waits for a"]
    );
}

#[test]
fn rule_4_each_cycle_is_named_once_from_its_first_step() {
    let source = workflow(&[
        ("s", &["b"], ""),
        ("a", &["b"], ""),
        ("b", &["a"], ""),
        ("x", &["y"], ""),
        ("y", &["x"], ""),
    ]);
    assert_eq!(
        workflow_errors(&source),
        [
            "error: workflow \"w\" step \"a\": after makes a cycle: a waits for b, b waits for a",
            "error: workflow \"w\" step \"x\": after makes a cycle: x waits for y, y waits for x",
        ]
    );
}

#[test]
fn rule_5_from_may_be_a_direct_or_indirect_wait() {
    let source = workflow(&[
        ("a", &[], ""),
        ("b", &["a"], "from = \"a\""),
        ("c", &["b"], "from = \"a\""),
    ]);
    assert_eq!(workflow_errors(&source), none());
}

#[test]
fn rule_5_from_must_be_a_step_this_step_waits_for() {
    let sibling = workflow(&[
        ("a", &[], ""),
        ("b", &["a"], ""),
        ("c", &["a"], "from = \"b\""),
    ]);
    assert_eq!(
        workflow_errors(&sibling),
        ["error: workflow \"w\" step \"c\": from \"b\" is not a step this step waits for"]
    );
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], "from = \"a\"")])),
        ["error: workflow \"w\" step \"a\": from \"a\" is not a step this step waits for"]
    );
}

#[test]
fn rule_6_known_variables_pass() {
    let source = workflow(&[
        ("a", &[], "prompt = \"do {{input}} in {{ repo }}\""),
        ("b", &["a"], ""),
        (
            "c",
            &["b"],
            "prompt = \"{{steps.a.result}} on {{steps.b.branch}}\"",
        ),
    ]);
    assert_eq!(workflow_errors(&source), none());
}

#[test]
fn rule_6_unknown_variables_fail() {
    assert_eq!(
        workflow_errors(&workflow(&[(
            "a",
            &[],
            "prompt = \"{{inptu}} and {{steps.a.diff}}\""
        )])),
        [
            "error: workflow \"w\" step \"a\": prompt uses unknown variable \"{{inptu}}\"",
            "error: workflow \"w\" step \"a\": prompt uses unknown variable \"{{steps.a.diff}}\"",
        ]
    );
}

#[test]
fn rule_6_unclosed_variables_fail() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], "prompt = \"do {{input\"")])),
        ["error: workflow \"w\" step \"a\": prompt has a \"{{\" without a closing \"}}\""]
    );
}

#[test]
fn rule_6_step_variables_name_a_step() {
    assert_eq!(
        workflow_errors(&workflow(&[("a", &[], "prompt = \"{{steps.zz.result}}\"")])),
        ["error: workflow \"w\" step \"a\": prompt variable \"{{steps.zz.result}}\" names \"zz\", which is not a step"]
    );
}

#[test]
fn rule_6_step_variables_name_a_step_that_runs_before() {
    let source = workflow(&[
        ("a", &[], ""),
        (
            "b",
            &["a"],
            "prompt = \"{{steps.c.branch}} {{steps.b.result}}\"",
        ),
        ("c", &["a"], ""),
    ]);
    assert_eq!(
        workflow_errors(&source),
        [
            "error: workflow \"w\" step \"b\": prompt variable \"{{steps.c.branch}}\" names \"c\", which does not run before this step",
            "error: workflow \"w\" step \"b\": prompt variable \"{{steps.b.result}}\" names \"b\", which does not run before this step",
        ]
    );
}

#[test]
fn every_workflow_error_is_returned_in_order() {
    let source = workflow(&[
        ("a", &["z"], ""),
        ("a", &[], "from = \"q\""),
        ("b", &[], "prompt = \"{{nope}}\""),
    ])
    .replace("name = \"w\"", "name = \"w x\"");
    assert_eq!(
        workflow_errors(&source),
        [
            "error: workflow \"w x\": name \"w x\" is not valid: use letters, digits, - and _ only",
            "error: workflow \"w x\" step \"a\": after \"z\" is not a step",
            "error: workflow \"w x\" step \"a\": id \"a\" is used by more than one step",
            "error: workflow \"w x\" step \"a\": from \"q\" is not a step",
            "error: workflow \"w x\" step \"b\": prompt uses unknown variable \"{{nope}}\"",
        ]
    );
}
