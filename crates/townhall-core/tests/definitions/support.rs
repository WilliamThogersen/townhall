use townhall_core::definitions::{parse_agent, parse_workflow, validate_agent, validate_workflow};

pub const CODER: &str = include_str!("fixtures/coder.toml");
pub const SHIP: &str = include_str!("fixtures/ship.toml");
pub const KNOWN: [&str; 4] = ["planner", "coder", "tester", "reviewer"];

/// Parse and validate an agent, returning every error as text.
pub fn agent_errors(source: &str) -> Vec<String> {
    match parse_agent(source) {
        Err(e) => vec![e.to_string()],
        Ok(parsed) => match validate_agent(&parsed.def) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(ToString::to_string).collect(),
        },
    }
}

/// Parse and validate a workflow against `KNOWN`, returning every error as text.
pub fn workflow_errors(source: &str) -> Vec<String> {
    match parse_workflow(source) {
        Err(e) => vec![e.to_string()],
        Ok(parsed) => match validate_workflow(&parsed.def, &KNOWN) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(ToString::to_string).collect(),
        },
    }
}

/// A workflow named `w` with one `[[step]]` per `(id, after, extra)`.
pub fn workflow(steps: &[(&str, &[&str], &str)]) -> String {
    let body: Vec<String> = steps
        .iter()
        .map(|(id, after, extra)| {
            let after: Vec<String> = after.iter().map(|a| format!("\"{a}\"")).collect();
            format!(
                "[[step]]\nid = \"{id}\"\nagent = \"coder\"\nafter = [{}]\n{extra}\n",
                after.join(", ")
            )
        })
        .collect();
    format!("name = \"w\"\n\n{}", body.join("\n"))
}
