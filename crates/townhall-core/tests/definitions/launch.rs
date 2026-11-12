use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use townhall_core::agent::{
    AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter, AGENT_ISOLATION, FOREGROUND_RULE,
};
use townhall_core::definitions::{parse_agent, AgentDef, Effort, PermissionMode};
use townhall_core::model::PermissionMode as SettingsMode;

use crate::support::{agent_errors, CODER};

/// The argv the real `ClaudeCodeAdapter` command hands to its program, without the
/// isolation flags every agent gets: checked here, right before the prompt.
fn argv(options: &LaunchOptions) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let echo = dir.path().join("echo-args");
    std::fs::write(&echo, "#!/bin/sh\nprintf '%s|' \"$@\"\n").unwrap();
    std::fs::set_permissions(&echo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = ClaudeCodeAdapter::new(echo.to_string_lossy())
        .launch_command(options)
        .unwrap();
    let out = Command::new("sh").args(["-c", &command]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let words: Vec<String> = text.split('|').map(str::to_string).collect();
    let words = &words[..words.len() - 1];
    let end = words.iter().position(|w| w == "--").unwrap_or(words.len());
    let start = end - AGENT_ISOLATION.len();
    assert_eq!(words[start..end], AGENT_ISOLATION, "{words:?}");
    [&words[..start], &words[end..]].concat()
}

fn agent(extra: &str) -> AgentDef {
    parse_agent(&format!("name = \"a\"\nadapter = \"claude\"\n{extra}\n"))
        .unwrap()
        .def
}

#[test]
fn the_coder_runs_with_its_own_flags() {
    let coder = parse_agent(CODER).unwrap().def;
    let prompt = coder.prompt.clone().unwrap();
    assert_eq!(
        argv(&coder.launch_options()),
        [
            "--model=sonnet".to_string(),
            "--permission-mode=dontAsk".into(),
            format!("--append-system-prompt={prompt}\n\nthese are the tools you have access to: Read, Edit, Write, Bash. every other tool is denied.\n\n{FOREGROUND_RULE}"),
            "--tools=Read,Edit,Write,Bash".into(),
            "--allowedTools=Read,Edit,Write,Bash".into(),
        ]
    );
}

/// The words a permission mode adds to claude's argv.
fn mode_words(mode: &str) -> Vec<String> {
    let flag = match mode {
        "skip all" => "--dangerously-skip-permissions".to_string(),
        value => format!("--permission-mode={value}"),
    };
    vec![flag, rule_flag()]
}

fn rule_flag() -> String {
    format!("--append-system-prompt={FOREGROUND_RULE}")
}

#[test]
fn a_definition_runs_in_its_own_mode_and_accept_edits_when_it_sets_none() {
    let table = [
        (None, "acceptEdits"),
        (Some("plan"), "plan"),
        (Some("default"), "default"),
        (Some("acceptEdits"), "acceptEdits"),
        (Some("bypassPermissions"), "skip all"),
    ];
    for (own, want) in table {
        let def = agent(&own.map_or(String::new(), |m| format!("permission_mode = \"{m}\"")));
        assert_eq!(argv(&def.launch_options()), mode_words(want), "{own:?}");
    }
}

#[test]
fn definition_modes_map_onto_the_same_settings_modes() {
    for mode in [
        PermissionMode::Default,
        PermissionMode::AcceptEdits,
        PermissionMode::Plan,
    ] {
        assert_eq!(SettingsMode::from(mode).cli_value(), Some(mode.as_str()));
    }
    assert_eq!(
        SettingsMode::from(PermissionMode::BypassPermissions),
        SettingsMode::BypassPermissions
    );
    assert_eq!(
        PermissionMode::BypassPermissions.as_str(),
        "bypassPermissions"
    );
}

#[test]
fn an_effort_reaches_the_claude_launch_only_when_set() {
    let medium = agent("effort = \"medium\"").launch_options();
    assert_eq!(medium.effort, Some(Effort::Medium));
    assert_eq!(
        argv(&medium),
        [
            "--effort=medium".to_string(),
            "--permission-mode=acceptEdits".into(),
            rule_flag()
        ]
    );
    let unset = agent("").launch_options();
    assert_eq!(unset.effort, None);
    assert_eq!(argv(&unset), mode_words("acceptEdits"));
}

#[test]
fn a_blank_model_is_unset() {
    let def = agent("model = \"  \"");
    assert_eq!(def.launch_options().model, None);
    assert_eq!(argv(&def.launch_options()), mode_words("acceptEdits"));
}

#[test]
fn the_step_prompt_goes_last_after_the_end_of_options() {
    let def = agent("tools = [\"Read\", \"Bash(rm -rf:*)\"]");
    let options = LaunchOptions {
        prompt: Some("go on".into()),
        ..def.launch_options()
    };
    assert_eq!(
        argv(&options),
        [
            "--permission-mode=dontAsk".to_string(),
            format!("--append-system-prompt=these are the tools you have access to: Read, Bash(rm -rf:*). every other tool is denied.\n\n{FOREGROUND_RULE}"),
            "--tools=Read,Bash".into(),
            "--allowedTools=Read,Bash(rm -rf:*)".into(),
            "--".into(),
            "go on".into(),
        ]
    );
}

#[test]
fn a_tool_with_a_comma_is_refused_at_launch() {
    let def = agent("tools = [\"Read\", \"Bash(git diff:*,x)\"]");
    let error = ClaudeCodeAdapter::default()
        .launch_command(&def.launch_options())
        .unwrap_err();
    assert!(
        error.to_string().contains("must not hold a comma"),
        "{error}"
    );
}

#[test]
fn a_tool_with_a_comma_is_refused_at_validation() {
    let source =
        "name = \"a\"\nadapter = \"claude\"\ntools = [\"Read,Edit\", \"Bash(git diff:*,x)\"]\n";
    assert_eq!(
        agent_errors(source),
        [
            "error: agent \"a\": tools entry \"Read,Edit\" must not hold a comma: give each tool its own entry",
            "error: agent \"a\": tools entry \"Bash(git diff:*,x)\" must not hold a comma: give each tool its own entry",
        ]
    );
}

#[test]
fn a_shell_definition_carries_its_command() {
    let def = parse_agent("name = \"s\"\nadapter = \"shell\"\ncommand = \"cargo test\"\n")
        .unwrap()
        .def;
    let options = def.launch_options();
    assert_eq!(options.command.as_deref(), Some("cargo test"));
    assert_eq!(ShellAdapter.launch_command(&options).unwrap(), "cargo test");
}

#[test]
fn tools_cannot_smuggle_flags() {
    assert_eq!(
        agent_errors("name = \"a\"\nadapter = \"claude\"\ntools = [\"Read\", \"--dangerously-skip-permissions\"]\n"),
        ["error: agent \"a\": tools entry \"--dangerously-skip-permissions\" must not start with \"-\""]
    );
    assert_eq!(
        agent_errors("name = \"a\"\nadapter = \"claude\"\ntools = [\" --dangerously-skip-permissions\", \"Read Edit\"]\n"),
        [
            "error: agent \"a\": tools entry \" --dangerously-skip-permissions\" has a space outside parentheses: give each tool its own entry",
            "error: agent \"a\": tools entry \"Read Edit\" has a space outside parentheses: give each tool its own entry",
        ]
    );
    assert_eq!(
        agent_errors("name = \"a\"\nadapter = \"claude\"\ntools = [\"Read\\n\"]\n"),
        ["error: agent \"a\": tools entry \"Read\\n\" must not hold control characters"]
    );
}

#[test]
fn tool_rules_with_spaces_inside_parentheses_pass() {
    let source = "name = \"a\"\nadapter = \"claude\"\ntools = [\"Bash(git diff:*)\", \"mcp__townhall__create_task\", \"Read\"]\n";
    assert!(agent_errors(source).is_empty());
}

#[test]
fn model_cannot_smuggle_flags() {
    let errors = |model: &str| {
        agent_errors(&format!(
            "name = \"a\"\nadapter = \"claude\"\nmodel = \"{model}\"\n"
        ))
    };
    assert!(errors("claude-opus-4-5[1m]").is_empty());
    assert!(errors("").is_empty());
    assert!(errors("  ").is_empty());
    assert_eq!(
        errors("--dangerously-skip-permissions"),
        ["error: agent \"a\": model \"--dangerously-skip-permissions\" must not start with \"-\""]
    );
    assert_eq!(
        errors("son net"),
        ["error: agent \"a\": model \"son net\" must not hold spaces"]
    );
    assert_eq!(
        errors("sonnet\\t"),
        ["error: agent \"a\": model \"sonnet\\t\" must not hold control characters"]
    );
}

#[test]
fn every_word_error_is_returned() {
    let source =
        "name = \"a\"\nadapter = \"claude\"\nmodel = \"-m\"\ntools = [\"-a\", \"Read\", \"-b\"]\n";
    assert_eq!(agent_errors(source).len(), 3);
}

#[test]
fn malformed_tool_patterns_get_clear_errors() {
    let cases = [
        ("Bash(git status", "has unbalanced parentheses"),
        ("Bash)", "has unbalanced parentheses"),
        ("Bash()", "has an empty pattern"),
        ("(git status)", "has no tool name before the parentheses"),
        (
            "Bash(git status)x",
            "has text after the closing parenthesis",
        ),
        (
            "Re ad",
            "has a space outside parentheses: give each tool its own entry",
        ),
        (
            "Bash!",
            "has a tool name with characters other than letters, digits and underscores",
        ),
    ];
    for (tool, problem) in cases {
        let source = format!("name = \"a\"\nadapter = \"claude\"\ntools = [\"{tool}\"]\n");
        assert_eq!(
            agent_errors(&source),
            [format!(
                "error: agent \"a\": tools entry \"{tool}\" {problem}"
            )]
        );
    }
}

#[test]
fn the_shipper_tool_list_is_valid() {
    let source = "name = \"shipper\"\nadapter = \"claude\"\ntools = [\"Read\", \"Grep\", \"Glob\", \"Bash(git status:*)\", \"Bash(git diff:*)\", \"Bash(git add:*)\", \"Bash(git commit:*)\", \"Bash(git push:*)\", \"Bash(gh pr *)\"]\n";
    assert!(agent_errors(source).is_empty());
}
