use townhall_core::definitions::{add_tool, parse_agent};

use crate::support::CODER;

const MINIMAL: &str = "name = \"a\"\nadapter = \"claude\"\n";

fn tools(source: &str) -> Vec<String> {
    parse_agent(source).unwrap().def.tools
}

#[test]
fn a_tool_joins_the_list_and_nothing_else_changes() {
    let edited = add_tool(CODER, "Bash(gh pr *)").unwrap().unwrap();
    assert_eq!(
        tools(&edited),
        ["Read", "Edit", "Write", "Bash", "Bash(gh pr *)"]
    );
    let before = parse_agent(CODER).unwrap().def;
    let after = parse_agent(&edited).unwrap().def;
    assert_eq!(after.prompt, before.prompt);
    assert_eq!(after.model, before.model);
    assert!(edited.starts_with("# the coder"));
}

#[test]
fn a_tool_already_listed_changes_nothing() {
    assert_eq!(add_tool(CODER, "Edit"), Ok(None));
    assert_eq!(add_tool(CODER, "  Edit "), Ok(None));
}

#[test]
fn a_type_without_tools_gets_a_list() {
    let edited = add_tool(MINIMAL, "Read").unwrap().unwrap();
    assert_eq!(tools(&edited), ["Read"]);
    let unterminated = add_tool("name = \"a\"\nadapter = \"claude\"", "Read")
        .unwrap()
        .unwrap();
    assert_eq!(tools(&unterminated), ["Read"]);
}

#[test]
fn a_multiline_list_is_rewritten() {
    let source = "name = \"a\"\ntools = [\n  \"Read\", # looks\n  \"Bash(git diff *)\",\n]\nadapter = \"claude\"\n";
    let edited = add_tool(source, "Grep").unwrap().unwrap();
    assert_eq!(tools(&edited), ["Read", "Bash(git diff *)", "Grep"]);
    assert!(edited.contains("adapter = \"claude\""));
}

#[test]
fn a_tools_line_inside_the_prompt_is_left_alone() {
    let source = "name = \"a\"\nadapter = \"claude\"\nprompt = \"\"\"\ntools = [\"x\"]\n\"\"\"\n";
    let edited = add_tool(source, "Read").unwrap().unwrap();
    let def = parse_agent(&edited).unwrap().def;
    assert_eq!(def.tools, ["Read"]);
    assert_eq!(def.prompt.as_deref(), Some("tools = [\"x\"]\n"));
}

#[test]
fn a_tool_that_fails_validation_is_refused() {
    for bad in ["", "  ", "--evil", "Bash git", "A,B"] {
        assert!(add_tool(CODER, bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_source_that_does_not_parse_is_refused() {
    assert!(add_tool("not toml [", "Read").is_err());
}
