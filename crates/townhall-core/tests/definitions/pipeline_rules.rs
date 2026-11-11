//! The pipeline fields of a workflow and their save-time checks.

use townhall_core::definitions::{parse_workflow, validate_pipeline, KnownAgent};

const REQUIRES_TESTER: &[&str] = &["tester"];

fn errors(source: &str) -> Vec<String> {
    let requires = REQUIRES_TESTER
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let reviewer_requires = ["reviewer".to_string()];
    let agents = [
        KnownAgent::new("planner", &[]),
        KnownAgent::new("builder", &[]),
        KnownAgent::new("tester", &[]),
        KnownAgent::new("reviewer", &requires),
        KnownAgent::new("lander", &reviewer_requires),
    ];
    match parse_workflow(source) {
        Err(e) => vec![e.to_string()],
        Ok(parsed) => match validate_pipeline(&parsed.def, &agents) {
            Ok(()) => Vec::new(),
            Err(found) => found.iter().map(ToString::to_string).collect(),
        },
    }
}

const STANDARD: &str = r#"
name = "standard"
repos = ["app"]
default = true
work = "build"
max_fix_rounds = 2

[[step]]
id = "plan"
agent = "planner"

[[step]]
id = "build"
agent = "builder"
after = ["plan"]
check = "cargo test"
max_retries = 3

[[step]]
id = "test"
agent = "tester"
after = ["build"]

[[step]]
id = "review"
agent = "reviewer"
after = ["test"]
when = { paths = ["**/migrations/**"] }

[[step]]
id = "land"
agent = "lander"
after = ["test", "review"]
"#;

#[test]
fn the_new_fields_parse_with_their_defaults() {
    let parsed = parse_workflow(STANDARD).unwrap().def;
    assert_eq!(parsed.repos, ["app"]);
    assert!(parsed.default);
    assert_eq!(parsed.work.as_deref(), Some("build"));
    assert_eq!(parsed.max_fix_rounds, 2);
    assert_eq!(parsed.steps[1].check.as_deref(), Some("cargo test"));
    assert_eq!(parsed.steps[1].max_retries, Some(3));
    let review = parsed.steps[3].when.as_ref().unwrap();
    assert_eq!(review.paths, ["**/migrations/**"]);
    let bare = parse_workflow("name = \"w\"\n[[step]]\nid = \"a\"\nagent = \"builder\"\n")
        .unwrap()
        .def;
    assert!(bare.repos.is_empty() && !bare.default && bare.work.is_none());
    assert_eq!(bare.max_fix_rounds, 1);
    assert!(bare.steps[0].when.is_none() && bare.steps[0].max_retries.is_none());
}

#[test]
fn a_standard_pipeline_passes_when_a_sound_one_is_unconditional() {
    let source = STANDARD.replace(
        "when = { paths = [\"**/migrations/**\"] }\n",
        "",
    );
    assert_eq!(errors(&source), Vec::<String>::new());
}

#[test]
fn an_unknown_agent_is_refused() {
    let source = STANDARD.replace("agent = \"planner\"", "agent = \"ghost\"");
    let found = errors(&source);
    assert!(
        found
            .iter()
            .any(|e| e.contains("step \"plan\"") && e.contains("agent \"ghost\" is not a saved agent")),
        "{found:?}"
    );
}

#[test]
fn work_must_name_a_step() {
    let source = STANDARD.replace("work = \"build\"", "work = \"compile\"");
    let found = errors(&source);
    assert!(
        found
            .iter()
            .any(|e| e.starts_with("error: workflow \"standard\": work \"compile\" is not a step")),
        "{found:?}"
    );
}

#[test]
fn an_after_cycle_is_refused() {
    let source = STANDARD.replace("after = [\"plan\"]", "after = [\"land\"]");
    let found = errors(&source);
    assert!(found.iter().any(|e| e.contains("makes a cycle")), "{found:?}");
}

#[test]
fn a_required_type_needs_an_earlier_step_without_when() {
    let found = errors(STANDARD);
    assert!(
        found.iter().any(|e| e.contains("step \"land\"")
            && e.contains("requires")
            && e.contains("reviewer")
            && e.contains("no `when`")),
        "{found:?}"
    );
}

#[test]
fn a_required_step_after_the_step_does_not_count() {
    let source = STANDARD
        .replace("after = [\"test\", \"review\"]", "after = [\"test\"]")
        .replace(
            "when = { paths = [\"**/migrations/**\"] }\n",
            "",
        );
    let found = errors(&source);
    assert!(
        found
            .iter()
            .any(|e| e.contains("step \"land\"") && e.contains("reviewer")),
        "{found:?}"
    );
}

#[test]
fn a_conditional_step_may_not_require_a_conditional_step() {
    let source = STANDARD.replace(
        "after = [\"test\", \"review\"]",
        "after = [\"test\", \"review\"]\nwhen = { paths = [\"src/**\"] }",
    );
    let found = errors(&source);
    assert!(
        found
            .iter()
            .any(|e| e.contains("step \"land\"") && e.contains("a step that has one")),
        "{found:?}"
    );
}
