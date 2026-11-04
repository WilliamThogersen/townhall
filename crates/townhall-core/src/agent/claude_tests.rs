use super::*;

const ID: &str = "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b";

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
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake claude");
    std::fs::write(&fake, "#!/bin/sh\nprintf '%s\\0' \"$@\"\n").unwrap();
    let command = ClaudeCodeAdapter::new(fake.to_string_lossy())
        .launch_command(options)
        .unwrap();
    let out = std::process::Command::new("sh")
        .args(["-c", &format!("sh {command}")])
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
            "--permission-mode=acceptEdits".into(),
            "--append-system-prompt=--dangerously-skip-permissions\nthen $COMMIT".into(),
            format!("--session-id={ID}"),
            "--allowedTools=Read,Bash(git diff:*)".into(),
            "--".into(),
            "fix it".into(),
        ]
    );
    assert_eq!(ClaudeCodeAdapter::args(&o).unwrap(), argv);
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
        "claude '--permission-mode=acceptEdits'"
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
    assert!(plain.starts_with("/opt/fake/claude-1.0 "), "{plain}");
    assert!(spaced.starts_with("'/my tools/claude' "), "{spaced}");
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
