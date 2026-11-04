use std::process::Command;

use townhall_core::agent::{AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};
use townhall_core::definitions::{parse_agent, AgentDef, PermissionMode};
use townhall_core::model::PermissionMode as SettingsMode;

use crate::support::{agent_errors, CODER};

const SETTINGS_MODES: [SettingsMode; 5] = [
    SettingsMode::Default,
    SettingsMode::AcceptEdits,
    SettingsMode::Plan,
    SettingsMode::Auto,
    SettingsMode::BypassPermissions,
];

/// The argv the real `ClaudeCodeAdapter` command hands to its program.
fn argv(options: &LaunchOptions) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let echo = dir.path().join("echo-args");
    std::fs::write(&echo, "#!/bin/sh\nprintf '%s|' \"$@\"\n").unwrap();
    let command = ClaudeCodeAdapter::new(echo.to_string_lossy())
        .launch_command(options)
        .unwrap();
    let out = Command::new("sh")
        .args(["-c", &format!("sh {command}")])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let words: Vec<String> = text.split('|').map(str::to_string).collect();
    words[..words.len() - 1].to_vec()
}

fn agent(extra: &str) -> AgentDef {
    parse_agent(&format!("name = \"a\"\nadapter = \"claude\"\n{extra}\n"))
        .unwrap()
        .def
}

#[test]
fn the_definition_mode_wins_over_settings() {
    let coder = parse_agent(CODER).unwrap().def;
    let prompt = coder.prompt.clone().unwrap();
    assert_eq!(
        argv(&coder.launch_options(SettingsMode::Plan)),
        [
            "--model",
            "sonnet",
            "--permission-mode",
            "acceptEdits",
            "--append-system-prompt",
            prompt.trim(),
            "--allowedTools",
            "Read",
            "Edit",
            "Write",
            "Bash",
        ]
    );
}

#[test]
fn settings_decide_when_the_definition_sets_no_mode() {
    let def = agent("");
    assert_eq!(
        argv(&def.launch_options(SettingsMode::Plan)),
        ["--permission-mode", "plan"]
    );
    assert_eq!(
        argv(&def.launch_options(SettingsMode::Auto)),
        ["--permission-mode", "auto"]
    );
    assert_eq!(
        argv(&def.launch_options(SettingsMode::BypassPermissions)),
        ["--dangerously-skip-permissions"]
    );
}

#[test]
fn a_definition_never_yields_skip_all() {
    for mode in [
        PermissionMode::Default,
        PermissionMode::AcceptEdits,
        PermissionMode::Plan,
    ] {
        assert_eq!(SettingsMode::from(mode).cli_value(), Some(mode.as_str()));
        let def = agent(&format!("permission_mode = \"{}\"", mode.as_str()));
        for settings in SETTINGS_MODES {
            let options = def.launch_options(settings);
            assert_eq!(options.permission_mode, SettingsMode::from(mode));
            assert_eq!(argv(&options), ["--permission-mode", mode.as_str()]);
        }
    }
}

#[test]
fn the_step_prompt_goes_before_the_tools() {
    let def = agent("tools = [\"Read\", \"Bash(rm -rf:*)\"]");
    let options = LaunchOptions {
        prompt: Some("go on".into()),
        ..def.launch_options(SettingsMode::AcceptEdits)
    };
    assert_eq!(
        argv(&options),
        [
            "--permission-mode",
            "acceptEdits",
            "go on",
            "--allowedTools",
            "Read",
            "Bash(rm -rf:*)",
        ]
    );
}

#[test]
fn a_shell_definition_carries_its_command() {
    let def = parse_agent("name = \"s\"\nadapter = \"shell\"\ncommand = \"cargo test\"\n")
        .unwrap()
        .def;
    let options = def.launch_options(SettingsMode::default());
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
