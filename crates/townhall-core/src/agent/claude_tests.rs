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

/// `flags`, then the isolation flags every agent gets.
fn with_isolation(flags: &[&str]) -> Vec<String> {
    flags
        .iter()
        .copied()
        .chain(AGENT_ISOLATION)
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
        assert_eq!(flags, with_isolation(&["--permission-mode=plan"]), "{p}");
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
        ]
        .into_iter()
        .chain(AGENT_ISOLATION.map(String::from))
        .chain(["--".into(), "fix it".into()])
        .collect::<Vec<String>>()
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
    let cases = [
        (PermissionMode::AcceptEdits, "--permission-mode=acceptEdits"),
        (PermissionMode::Plan, "--permission-mode=plan"),
        (PermissionMode::Auto, "--permission-mode=auto"),
        (PermissionMode::Default, "--permission-mode=default"),
        (
            PermissionMode::BypassPermissions,
            "--dangerously-skip-permissions",
        ),
    ];
    for (m, flag) in cases {
        assert_eq!(mode(m), with_isolation(&[flag]), "{flag}");
    }
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
        with_isolation(&["--permission-mode=acceptEdits"])
    );
    let quoted: Vec<String> = with_isolation(&["--permission-mode=acceptEdits"])
        .iter()
        .map(|a| shell_quote(a))
        .collect();
    assert_eq!(
        ClaudeCodeAdapter::default().launch_command(&o).unwrap(),
        format!("{UNSET} claude {}", quoted.join(" "))
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
    let mut expected = with_isolation(&[
        "--permission-mode=plan",
        "--add-dir=/h/tasks/fix-login",
        "--add-dir=/h/my files",
    ]);
    expected.extend(["--".to_string(), "go on".to_string()]);
    assert_eq!(argv_through_sh(&o), expected);
    for bad in ["relative/dir", "", "/a\nb", "-/x"] {
        let o = LaunchOptions {
            add_dirs: vec![bad.into()],
            ..opts("x")
        };
        let err = ClaudeCodeAdapter::args(&o).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{bad:?}: {err:?}");
    }
}

#[test]
fn agents_start_without_the_users_claude_setup() {
    let argv = ClaudeCodeAdapter::args(&LaunchOptions::default()).unwrap();
    for flag in [
        "--setting-sources=",
        "--settings={\"disableAllHooks\":true,\"autoMemoryEnabled\":false}",
        "--strict-mcp-config",
    ] {
        assert!(argv.iter().any(|a| a == flag), "{flag} missing: {argv:?}");
    }
    assert!(
        !argv.iter().any(|a| a.starts_with("--mcp-config")),
        "agents get no MCP server: {argv:?}"
    );
}
