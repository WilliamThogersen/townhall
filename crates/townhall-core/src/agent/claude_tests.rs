use std::os::unix::fs::PermissionsExt;

use super::*;

const ID: &str = "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b";
const UNSET: &str = "env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_ENTRYPOINT \
                     -u CLAUDE_CODE_SSE_PORT -u CLAUDE_EFFORT";

/// The launch command of a fake claude running `script`, and the folder that holds it.
fn fake_command(script: &str, options: &LaunchOptions) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake claude");
    std::fs::write(&fake, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = ClaudeCodeAdapter::new(fake.to_string_lossy())
        .launch_command(options)
        .unwrap();
    (dir, command)
}

fn opts(prompt: &str) -> LaunchOptions {
    LaunchOptions {
        prompt: Some(prompt.into()),
        permission_mode: PermissionMode::Plan,
        ..LaunchOptions::default()
    }
}

/// Runs the launch command through `sh` with a fake claude that prints its argv,
/// NUL-separated, so newlines inside a value survive.
fn argv_through_sh(options: &LaunchOptions) -> Vec<String> {
    let (_dir, command) = fake_command("printf '%s\\0' \"$@\"", options);
    let out = std::process::Command::new("sh")
        .args(["-c", &command])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    text.strip_suffix('\0')
        .unwrap_or(&text)
        .split('\0')
        .map(String::from)
        .collect()
}

/// What a flag parser sees: everything before the first `--` is options, after it data.
fn split(argv: &[String]) -> (&[String], &[String]) {
    let end = argv.iter().position(|a| a == "--").unwrap_or(argv.len());
    (&argv[..end], argv.get(end + 1..).unwrap_or(&[]))
}

#[test]
fn hostile_prompts_arrive_as_the_one_positional() {
    let hostile = [
        "--dangerously-skip-permissions",
        "-p",
        "--permission-mode=bypassPermissions",
        "--",
        "update",
    ];
    for p in hostile {
        let argv = argv_through_sh(&opts(p));
        let (flags, data) = split(&argv);
        assert_eq!(flags, ["--permission-mode=plan"], "{p}");
        assert_eq!(data, [format!("{p}\n")], "{p}");
    }
    let spaced = "-p and --dangerously-skip-permissions please";
    assert_eq!(split(&argv_through_sh(&opts(spaced))).1, [spaced]);
}

#[test]
fn every_option_is_one_element_before_the_prompt() {
    let o = LaunchOptions {
        prompt: Some("fix it".into()),
        model: Some("claude-opus's".into()),
        system_prompt: Some("--dangerously-skip-permissions\nthen $COMMIT".into()),
        allowed_tools: vec!["Read".into(), "Bash(git diff:*)".into(), "  ".into()],
        session_id: Some(ID.into()),
        permission_mode: PermissionMode::AcceptEdits,
        ..LaunchOptions::default()
    };
    let argv = argv_through_sh(&o);
    assert_eq!(
        argv,
        [
            "--model=claude-opus's".to_string(),
            "--permission-mode=dontAsk".into(),
            format!(
                "--append-system-prompt=--dangerously-skip-permissions\nthen $COMMIT\n\n{}",
                tools_notice("Read, Bash(git diff:*)")
            ),
            format!("--session-id={ID}"),
            "--tools=Read,Bash".into(),
            "--allowedTools=Read,Bash(git diff:*)".into(),
            "--".into(),
            "fix it".into(),
        ]
    );
    assert_eq!(ClaudeCodeAdapter::args(&o).unwrap(), argv);
}

fn tools_notice(list: &str) -> String {
    format!("these are the tools you have access to: {list}. every other tool is denied.")
}

fn with_tools(tools: &[&str], mode: PermissionMode) -> Vec<String> {
    let o = LaunchOptions {
        allowed_tools: tools.iter().map(|t| t.to_string()).collect(),
        permission_mode: mode,
        ..LaunchOptions::default()
    };
    ClaudeCodeAdapter::args(&o).unwrap()
}

#[test]
fn a_tool_list_is_the_whole_set_and_everything_else_is_denied() {
    let list = ["Read", "Grep", "Bash(gh pr *)", "Bash(git commit *)"];
    for mode in [
        PermissionMode::Default,
        PermissionMode::AcceptEdits,
        PermissionMode::Auto,
        PermissionMode::BypassPermissions,
    ] {
        let argv = with_tools(&list, mode);
        assert_eq!(argv[0], "--permission-mode=dontAsk", "{mode:?}");
        assert!(!argv.iter().any(|a| a.contains("dangerously")), "{mode:?}");
        assert!(argv.contains(&"--tools=Read,Grep,Bash".to_string()));
        assert!(
            argv.contains(&"--allowedTools=Read,Grep,Bash(gh pr *),Bash(git commit *)".to_string())
        );
    }
    assert_eq!(
        with_tools(&list, PermissionMode::Plan)[0],
        "--permission-mode=plan"
    );
}

#[test]
fn the_prompt_names_the_tools_the_agent_has() {
    let argv = with_tools(&["Read", "Bash(gh pr *)"], PermissionMode::Default);
    assert!(argv.contains(&format!(
        "--append-system-prompt={}",
        tools_notice("Read, Bash(gh pr *)")
    )));
}

#[test]
fn mcp_tools_are_allowed_but_are_not_built_in_tools() {
    let argv = with_tools(
        &["Read", "mcp__townhall__task_status"],
        PermissionMode::Default,
    );
    assert!(argv.contains(&"--tools=Read".to_string()), "{argv:?}");
    assert!(argv.contains(&"--allowedTools=Read,mcp__townhall__task_status".to_string()));
}

#[test]
fn no_tool_list_leaves_the_mode_and_tools_alone() {
    assert_eq!(
        with_tools(&[], PermissionMode::AcceptEdits),
        ["--permission-mode=acceptEdits"]
    );
}

#[test]
fn values_that_look_like_flags_are_refused() {
    let cases = [
        LaunchOptions {
            model: Some("-p".into()),
            ..opts("x")
        },
        LaunchOptions {
            model: Some("son\u{7}net".into()),
            ..opts("x")
        },
        LaunchOptions {
            session_id: Some("-x".into()),
            ..opts("x")
        },
        LaunchOptions {
            session_id: Some("not-a-uuid".into()),
            ..opts("x")
        },
        LaunchOptions {
            allowed_tools: vec!["--dangerously-skip-permissions".into()],
            ..opts("x")
        },
        LaunchOptions {
            allowed_tools: vec!["Read\nBash".into()],
            ..opts("x")
        },
        LaunchOptions {
            allowed_tools: vec!["Read,Bash".into()],
            ..opts("x")
        },
    ];
    for o in cases {
        let err = ClaudeCodeAdapter::default().launch_command(&o).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{o:?}: {err:?}");
    }
}

#[test]
fn permission_modes() {
    let mode = |m| {
        ClaudeCodeAdapter::args(&LaunchOptions {
            permission_mode: m,
            ..LaunchOptions::default()
        })
        .unwrap()
    };
    assert_eq!(
        mode(PermissionMode::AcceptEdits),
        ["--permission-mode=acceptEdits"]
    );
    assert_eq!(mode(PermissionMode::Plan), ["--permission-mode=plan"]);
    assert_eq!(mode(PermissionMode::Auto), ["--permission-mode=auto"]);
    assert_eq!(mode(PermissionMode::Default), ["--permission-mode=default"]);
    assert_eq!(
        mode(PermissionMode::BypassPermissions),
        ["--dangerously-skip-permissions"]
    );
}

#[test]
fn no_prompt_means_no_positional_and_blank_values_add_nothing() {
    let o = LaunchOptions {
        model: Some(" ".into()),
        system_prompt: Some("  ".into()),
        prompt: Some(String::new()),
        ..LaunchOptions::default()
    };
    assert_eq!(
        ClaudeCodeAdapter::args(&o).unwrap(),
        ["--permission-mode=acceptEdits"]
    );
    assert_eq!(
        ClaudeCodeAdapter::default().launch_command(&o).unwrap(),
        format!("{UNSET} claude '--permission-mode=acceptEdits'")
    );
    assert_eq!(split(&argv_through_sh(&opts("  "))).1, ["  "]);
}

#[test]
fn custom_bin_is_quoted_only_when_needed() {
    let o = LaunchOptions::default();
    let plain = ClaudeCodeAdapter::new("/opt/fake/claude-1.0")
        .launch_command(&o)
        .unwrap();
    let spaced = ClaudeCodeAdapter::new("/my tools/claude")
        .launch_command(&o)
        .unwrap();
    assert!(
        plain.starts_with(&format!("{UNSET} /opt/fake/claude-1.0 ")),
        "{plain}"
    );
    assert!(
        spaced.starts_with(&format!("{UNSET} '/my tools/claude' ")),
        "{spaced}"
    );
}

#[test]
fn claude_runs_without_the_outer_sessions_variables() {
    let (_dir, command) = fake_command("env", &LaunchOptions::default());
    let outer = OUTER_SESSION_ENV.map(|name| (name, "outer"));
    let out = std::process::Command::new("sh")
        .args(["-c", &command])
        .envs(outer)
        .envs([
            ("CLAUDE_CONFIG_DIR", "/kept"),
            ("ANTHROPIC_API_KEY", "kept"),
        ])
        .output()
        .unwrap();
    let env = String::from_utf8(out.stdout).unwrap();
    for name in OUTER_SESSION_ENV {
        assert!(!env.contains(&format!("{name}=")), "{name}:\n{env}");
    }
    for kept in ["CLAUDE_CONFIG_DIR=/kept", "ANTHROPIC_API_KEY=kept"] {
        assert!(env.lines().any(|l| l == kept), "{kept}:\n{env}");
    }
}

#[test]
fn add_dirs_are_absolute_folders_before_the_prompt() {
    let o = LaunchOptions {
        add_dirs: vec!["/h/tasks/fix-login".into(), "/h/my files".into()],
        ..opts("go on")
    };
    assert_eq!(
        argv_through_sh(&o),
        [
            "--permission-mode=plan",
            "--add-dir=/h/tasks/fix-login",
            "--add-dir=/h/my files",
            "--",
            "go on",
        ]
    );
    for bad in ["relative/dir", "", "/a\nb", "-/x"] {
        let o = LaunchOptions {
            add_dirs: vec![bad.into()],
            ..opts("x")
        };
        let err = ClaudeCodeAdapter::args(&o).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{bad:?}: {err:?}");
    }
}
