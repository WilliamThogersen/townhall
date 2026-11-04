use std::collections::HashMap;

use townhall_core::definitions::{render, variables, TemplateError, Values, Var};

fn values() -> Values {
    Values {
        input: "add dark mode".into(),
        repo: "townhall".into(),
        results: HashMap::from([("plan".to_string(), "the spec".to_string())]),
        branches: HashMap::from([("build".to_string(), "townhall/build-3f2a".to_string())]),
    }
}

#[test]
fn renders_every_variable() {
    let prompt = "{{input}} in {{repo}}: {{steps.plan.result}} on {{steps.build.branch}}";
    assert_eq!(
        render(prompt, &values()).unwrap(),
        "add dark mode in townhall: the spec on townhall/build-3f2a"
    );
}

#[test]
fn allows_spaces_inside_the_braces() {
    assert_eq!(render("{{ input }}!", &values()).unwrap(), "add dark mode!");
}

#[test]
fn leaves_text_without_variables_alone() {
    let text = "fn main() { let x = {y}; } }} done";
    assert_eq!(render(text, &values()).unwrap(), text);
    assert_eq!(render("", &values()).unwrap(), "");
}

#[test]
fn does_not_expand_variables_inside_values() {
    let v = Values {
        input: "{{repo}}".into(),
        ..values()
    };
    assert_eq!(render("{{input}}", &v).unwrap(), "{{repo}}");
}

#[test]
fn fails_on_unknown_variables() {
    assert_eq!(
        render("hi {{ who }}", &values()),
        Err(TemplateError::Unknown("who".into()))
    );
    assert_eq!(
        render("{{steps.plan.diff}}", &values()),
        Err(TemplateError::Unknown("steps.plan.diff".into()))
    );
    assert_eq!(
        render("{{}}", &values()),
        Err(TemplateError::Unknown(String::new()))
    );
}

#[test]
fn fails_on_a_missing_value() {
    assert_eq!(
        render("{{steps.test.result}}", &values()),
        Err(TemplateError::Missing(Var::StepResult("test".into())))
    );
    assert_eq!(
        render("{{steps.plan.branch}}", &values()),
        Err(TemplateError::Missing(Var::StepBranch("plan".into())))
    );
}

#[test]
fn fails_on_unclosed_braces() {
    assert_eq!(
        render("do {{input", &values()),
        Err(TemplateError::Unclosed)
    );
}

#[test]
fn lists_the_variables_a_prompt_uses() {
    assert_eq!(
        variables("{{input}} {{repo}} {{steps.a.result}} {{steps.b.branch}}").unwrap(),
        [
            Var::Input,
            Var::Repo,
            Var::StepResult("a".into()),
            Var::StepBranch("b".into()),
        ]
    );
    assert_eq!(Var::StepResult("a".into()).step(), Some("a"));
    assert_eq!(Var::Input.step(), None);
}

#[test]
fn lists_every_problem_in_a_prompt() {
    assert_eq!(
        variables("{{a}} {{input}} {{b}} {{c"),
        Err(vec![
            TemplateError::Unknown("a".into()),
            TemplateError::Unknown("b".into()),
            TemplateError::Unclosed,
        ])
    );
}

#[test]
fn errors_read_as_prompt_problems() {
    assert_eq!(
        TemplateError::Unknown("who".into()).to_string(),
        "prompt uses unknown variable \"{{who}}\""
    );
    assert_eq!(
        TemplateError::Unclosed.to_string(),
        "prompt has a \"{{\" without a closing \"}}\""
    );
    assert_eq!(
        TemplateError::Missing(Var::StepResult("plan".into())).to_string(),
        "prompt has no value for \"{{steps.plan.result}}\""
    );
    assert_eq!(
        Var::StepBranch("b".into()).to_string(),
        "{{steps.b.branch}}"
    );
}
