use std::path::Path;

use super::*;
use crate::plugins::marketplace;
use crate::Error;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn entry_with(extra: &str) -> Entry {
    let text = format!(
        r#"{{"name":"m","owner":{{"name":"o"}},"plugins":[{{"name":"p","source":"./p"{extra}}}]}}"#
    );
    marketplace::parse(&text).unwrap().plugins.remove(0)
}

fn entry() -> Entry {
    entry_with("")
}

fn manifest(permissions: &str, rest: &str) -> String {
    format!(r#"{{"name":"p","version":"1.2.3","apiVersion":1,"permissions":{permissions}{rest}}}"#)
}

fn plugin(manifest: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".townhall-plugin/plugin.json", manifest);
    dir
}

fn refused(root: &Path) -> String {
    validate(root, &entry()).unwrap_err().to_string()
}

#[test]
fn the_permission_list_is_sorted_and_hashed() {
    let dir = plugin(&manifest(
        r#"{"api":["tasks:read","storage"],"hooks":["task.merged"],"prompts":["starters"],"env":["ACME_TOKEN"],"ui":["page"]}"#,
        "",
    ));
    let checked = validate(dir.path(), &entry()).unwrap();
    assert_eq!(checked.version.as_deref(), Some("1.2.3"));
    assert_eq!(
        checked.permissions,
        [
            "api:storage",
            "api:tasks:read",
            "env:ACME_TOKEN",
            "hook:task.merged",
            "prompt:starters",
            "ui:page",
        ]
    );
    assert!(checked.hash.starts_with("sha256:"), "{}", checked.hash);
    assert_eq!(checked.hash.len(), "sha256:".len() + 64);
}

#[test]
fn the_hash_ignores_the_order_it_was_declared_in_and_changes_with_any_addition() {
    let one = plugin(&manifest(r#"{"api":["tasks:read","storage"]}"#, ""));
    let two = plugin(&manifest(r#"{"api":["storage","tasks:read"]}"#, ""));
    let more = plugin(&manifest(
        r#"{"api":["storage","tasks:read","events"]}"#,
        "",
    ));
    let hash = |d: &tempfile::TempDir| validate(d.path(), &entry()).unwrap().hash;
    assert_eq!(hash(&one), hash(&two));
    assert_ne!(hash(&one), hash(&more));
}

#[test]
fn added_names_only_what_the_old_list_lacks() {
    let old = ["api:a".to_string(), "api:b".to_string()];
    let new = ["api:b".to_string(), "api:c".to_string()];
    assert_eq!(added(&old, &new), ["api:c"]);
    assert!(added(&old, &old).is_empty());
    assert!(added(&old, &old[..1]).is_empty());
}

#[test]
fn a_townhall_hook_for_an_event_that_was_not_declared_is_refused() {
    let dir = plugin(&manifest(
        r#"{"hooks":["task.merged"]}"#,
        r#","contributes":{"hooks":"townhall-hooks.json"}"#,
    ));
    write(
        dir.path(),
        "townhall-hooks.json",
        r#"{"hooks":{"task.merged":[{"command":"bin/a"}],"task.failed":[{"command":"bin/b"}]}}"#,
    );
    let err = refused(dir.path());
    assert!(err.contains("task.failed"), "{err}");
    assert!(err.contains("permissions.hooks"), "{err}");
}

#[test]
fn declared_hooks_add_their_commands_to_the_list() {
    let dir = plugin(&manifest(
        r#"{"hooks":["task.merged"]}"#,
        r#","contributes":{"hooks":"townhall-hooks.json"}"#,
    ));
    write(
        dir.path(),
        "townhall-hooks.json",
        r#"{"hooks":{"task.merged":[{"command":"bin/a","timeout":5}]}}"#,
    );
    let checked = validate(dir.path(), &entry()).unwrap();
    assert!(checked
        .permissions
        .contains(&"hook:task.merged".to_string()));
    assert!(
        checked
            .permissions
            .contains(&"runs.hook:task.merged:bin/a".to_string()),
        "{:?}",
        checked.permissions
    );
}

#[test]
fn a_claude_hook_event_or_mcp_server_that_was_not_declared_is_refused() {
    let hooks = r#"{"hooks":{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"x"}]}]}}"#;
    let dir = plugin(&manifest(r#"{"claude":{"hooks":{"events":["Stop"]}}}"#, ""));
    write(dir.path(), "hooks/hooks.json", hooks);
    let err = refused(dir.path());
    assert!(err.contains("PostToolUse"), "{err}");

    let dir = plugin(&manifest(
        r#"{"claude":{"mcp":{"servers":{"notes":["add_note"]}}}}"#,
        "",
    ));
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers":{"notes":{"command":"n"},"extra":{"command":"e"}}}"#,
    );
    let err = refused(dir.path());
    assert!(err.contains("extra"), "{err}");
    assert!(err.contains("permissions.claude.mcp.servers"), "{err}");
}

#[test]
fn declared_claude_parts_are_listed_with_their_commands() {
    let dir = plugin(&manifest(
        r#"{"claude":{"hooks":{"events":["PostToolUse"]},"mcp":{"servers":{"notes":["add_note"]}},"planner":true}}"#,
        "",
    ));
    write(
        dir.path(),
        "hooks/hooks.json",
        r#"{"hooks":{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"fmt.sh"}]}]}}"#,
    );
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers":{"notes":{"command":"node","args":["srv.js","--x"]}}}"#,
    );
    let got = validate(dir.path(), &entry()).unwrap().permissions;
    for want in [
        "claude.hooks:PostToolUse",
        "claude.mcp:notes:add_note",
        "claude.planner",
        "runs.hook:PostToolUse:fmt.sh",
        "runs.mcp:notes:node srv.js --x",
    ] {
        assert!(got.contains(&want.to_string()), "{want} in {got:?}");
    }
}

#[test]
fn a_plain_claude_plugin_gets_every_hook_event_and_mcp_server_it_has() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"p","version":"0.4.0"}"#,
    );
    write(
        dir.path(),
        "hooks/hooks.json",
        r#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"gate"}]}]}}"#,
    );
    write(
        dir.path(),
        ".mcp.json",
        r#"{"mcpServers":{"search":{"url":"https://mcp.example/x"}}}"#,
    );
    let checked = validate(dir.path(), &entry()).unwrap();
    assert_eq!(checked.version.as_deref(), Some("0.4.0"));
    for want in [
        "claude.hooks:PreToolUse",
        "claude.mcp:search:*",
        "connects.mcp:search:https://mcp.example/x",
    ] {
        assert!(
            checked.permissions.contains(&want.to_string()),
            "{want} in {:?}",
            checked.permissions
        );
    }
}

#[test]
fn an_entry_without_any_manifest_has_an_empty_list_and_the_entry_version() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "README.md", "hi");
    let checked = validate(dir.path(), &entry_with(r#","version":"2.0.0""#)).unwrap();
    assert!(checked.permissions.is_empty());
    assert_eq!(checked.version.as_deref(), Some("2.0.0"));
    assert!(checked.hash.starts_with("sha256:"));
}

#[test]
fn a_wrapped_entry_supplies_the_permissions_of_a_plain_plugin() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "README.md", "hi");
    let extra = r#","townhall":{"apiVersion":1,"permissions":{"api":["tasks:read"]}}"#;
    let checked = validate(dir.path(), &entry_with(extra)).unwrap();
    assert_eq!(checked.permissions, ["api:tasks:read"]);
}

#[test]
fn a_plugin_for_a_newer_api_cannot_be_installed() {
    let dir = plugin(r#"{"name":"p","version":"1.0.0","apiVersion":2}"#);
    let err = refused(dir.path());
    assert!(
        err.contains("needs plugin api 2, this townhall serves 1"),
        "{err}"
    );
}

#[test]
fn a_typo_in_the_manifest_is_an_error_and_not_a_silent_grant() {
    let dir = plugin(&manifest(r#"{"hookz":["task.merged"]}"#, ""));
    let err = refused(dir.path());
    assert!(err.contains("hookz"), "{err}");
}

#[test]
fn every_contributed_file_must_exist_inside_the_plugin() {
    let dir = plugin(&manifest(
        r#"{}"#,
        r#","contributes":{"theme":"theme.css"}"#,
    ));
    let err = refused(dir.path());
    assert!(err.contains("theme.css"), "{err}");
    assert!(err.contains("does not exist"), "{err}");
    write(dir.path(), "theme.css", ":root { --th-bg: #000; }");
    assert!(validate(dir.path(), &entry()).is_ok());
}

#[test]
fn a_plugin_agent_type_with_bypass_permissions_cannot_be_installed() {
    let dir = plugin(&manifest(
        r#"{}"#,
        r#","contributes":{"agentTypes":["agent-types/skip.toml"]}"#,
    ));
    let skip = "name = \"skip\"\nadapter = \"claude\"\npermission_mode = \"bypassPermissions\"\n";
    write(dir.path(), "agent-types/skip.toml", skip);
    let err = refused(dir.path());
    assert!(err.contains("agent-types/skip.toml"), "{err}");
    assert!(err.contains("bypassPermissions"), "{err}");
    let plain = "name = \"skip\"\nadapter = \"claude\"\npermission_mode = \"plan\"\n";
    write(dir.path(), "agent-types/skip.toml", plain);
    assert!(validate(dir.path(), &entry()).is_ok());
    write(dir.path(), "agent-types/skip.toml", "not toml at all");
    assert!(validate(dir.path(), &entry()).is_ok());
}

#[test]
fn a_contributed_file_that_is_a_symlink_out_of_the_plugin_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "secret.css", "x");
    let dir = plugin(&manifest(
        r#"{}"#,
        r#","contributes":{"theme":"theme.css"}"#,
    ));
    std::os::unix::fs::symlink(
        outside.path().join("secret.css"),
        dir.path().join("theme.css"),
    )
    .unwrap();
    let err = refused(dir.path());
    assert!(err.contains("outside"), "{err}");
}

#[test]
fn any_symlink_out_of_the_plugin_is_refused_even_when_nothing_names_it() {
    let outside = tempfile::tempdir().unwrap();
    let dir = plugin(&manifest(r#"{}"#, ""));
    std::os::unix::fs::symlink(outside.path(), dir.path().join("docs")).unwrap();
    let err = refused(dir.path());
    assert!(err.contains("outside"), "{err}");
}

#[test]
fn a_dangling_symlink_is_refused_and_one_that_stays_inside_is_fine() {
    let dir = plugin(&manifest(r#"{}"#, ""));
    write(dir.path(), "real.txt", "x");
    std::os::unix::fs::symlink("real.txt", dir.path().join("alias.txt")).unwrap();
    assert!(validate(dir.path(), &entry()).is_ok());
    std::os::unix::fs::symlink("missing.txt", dir.path().join("gone.txt")).unwrap();
    let err = refused(dir.path());
    assert!(err.contains("gone.txt"), "{err}");
}

#[test]
fn the_size_caps_hold_for_a_file_the_whole_plugin_and_the_manifest() {
    let dir = plugin(&manifest(
        r#"{}"#,
        r#","contributes":{"theme":"theme.css"}"#,
    ));
    write(dir.path(), "theme.css", &"a".repeat(100));
    let small = Limits {
        file: 50,
        total: 10_000,
    };
    let err = validate_with(dir.path(), &entry(), &small).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");

    let tight = Limits {
        file: 1000,
        total: 150,
    };
    let err = validate_with(dir.path(), &entry(), &tight).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");

    let big = plugin(&format!(
        r#"{{"name":"p","version":"1.0.0","apiVersion":1,"description":"{}"}}"#,
        "a".repeat(70 * 1024)
    ));
    let err = validate(big.path(), &entry()).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");
}

#[test]
fn a_hook_file_that_is_not_json_is_named_in_the_error() {
    let dir = plugin(&manifest(
        r#"{"hooks":["task.merged"]}"#,
        r#","contributes":{"hooks":"townhall-hooks.json"}"#,
    ));
    write(dir.path(), "townhall-hooks.json", "{ nope");
    let err = refused(dir.path());
    assert!(err.contains("townhall-hooks.json"), "{err}");
}

fn declarative(rest: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = plugin(&manifest(
        r#"{"prompts":["starters","brief","planner"]}"#,
        rest,
    ));
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    dir
}

#[test]
fn the_official_theme_and_starters_pass_install_validation() {
    let theme = declarative(
        r#","contributes":{"theme":"theme.css"}"#,
        &[("theme.css", include_str!("../theme/slate.css"))],
    );
    assert!(validate(theme.path(), &entry()).is_ok());
    let starters = declarative(
        r#","contributes":{"prompts":{"starters":"prompts/starters.json"}}"#,
        &[(
            "prompts/starters.json",
            include_str!("../prompts/starters.json"),
        )],
    );
    assert!(validate(starters.path(), &entry()).is_ok());
}

#[test]
fn a_theme_the_strict_parser_refuses_is_an_install_error() {
    for (css, needle) in [
        ("@import \"x.css\";", "@import"),
        ("body { --th-bg: #000; }", "selector"),
        (":root { color: red; }", "--th-"),
        (":root { --th-bg: url(x); }", "url("),
    ] {
        let dir = declarative(
            r#","contributes":{"theme":"theme.css"}"#,
            &[("theme.css", css)],
        );
        let err = refused(dir.path());
        assert!(
            err.contains("contributes.theme") && err.contains(needle),
            "{css}: {err}"
        );
    }
}

#[test]
fn starters_prompts_and_settings_that_do_not_validate_are_install_errors() {
    let long = "x".repeat(8001);
    let cases = [
        (
            r#","contributes":{"prompts":{"starters":"s.json"}}"#,
            "s.json",
            "{}",
            "starters",
        ),
        (
            r#","contributes":{"prompts":{"brief":"b.md"}}"#,
            "b.md",
            "  ",
            "brief",
        ),
        (
            r#","contributes":{"prompts":{"planner":"p.md"}}"#,
            "p.md",
            long.as_str(),
            "planner",
        ),
        (
            r#","contributes":{"settings":"settings.schema.json"}"#,
            "settings.schema.json",
            r#"{"fields":[{"key":"a","label":"a","type":"boolean","default":"no"}]}"#,
            "settings",
        ),
    ];
    for (rest, file, text, needle) in cases {
        let dir = declarative(rest, &[(file, text)]);
        let err = refused(dir.path());
        assert!(err.contains(needle), "{file}: {err}");
    }
}

#[test]
fn a_valid_brief_planner_and_schema_are_accepted() {
    let dir = declarative(
        r#","contributes":{"prompts":{"brief":"b.md","planner":"p.md"},"settings":"s.json"}"#,
        &[
            ("b.md", "write a changelog line"),
            ("p.md", "prefer small tasks"),
            (
                "s.json",
                r#"{"fields":[{"key":"a","label":"a","type":"boolean","default":true}]}"#,
            ),
        ],
    );
    assert!(validate(dir.path(), &entry()).is_ok());
}

const SKILL: &str = "---\nname: x\ndescription: d\n---\nbody";

#[test]
fn a_skills_only_plain_plugin_installs_and_lists_its_skills_and_commands() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"p","version":"1.0.0","description":"d","keywords":["web"]}"#,
    );
    write(dir.path(), "skills/guide/SKILL.md", SKILL);
    write(dir.path(), "skills/guide/references/a.md", "ref");
    write(dir.path(), "skills/other/SKILL.md", SKILL);
    write(dir.path(), "skills/notes/readme.md", "not a skill");
    write(dir.path(), "commands/deploy.md", "run it");
    let checked = validate(dir.path(), &entry()).unwrap();
    assert_eq!(
        checked.permissions,
        [
            "claude.command:deploy",
            "claude.skill:guide",
            "claude.skill:other"
        ]
    );
    assert!(checked.declared.skills.is_empty());
}

#[test]
fn a_strict_false_entry_names_its_skill_folders() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "local-ai/SKILL.md", SKILL);
    write(dir.path(), "pack/one/SKILL.md", SKILL);
    write(dir.path(), "pack/two/SKILL.md", SKILL);
    write(dir.path(), "cmds/go.md", "go");
    let extra = r#","strict":false,"skills":["./local-ai","./pack"],"commands":["./cmds"]"#;
    let checked = validate(dir.path(), &entry_with(extra)).unwrap();
    assert_eq!(
        checked.permissions,
        [
            "claude.command:go",
            "claude.skill:local-ai",
            "claude.skill:one",
            "claude.skill:two"
        ]
    );
    assert_eq!(checked.declared.skills, ["./local-ai", "./pack"]);
    assert_eq!(checked.declared.commands, ["./cmds"]);
}

#[test]
fn a_skill_path_that_is_not_in_the_plugin_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "README.md", "hi");
    let extra = r#","strict":false,"skills":["./missing"]"#;
    let err = validate(dir.path(), &entry_with(extra))
        .unwrap_err()
        .to_string();
    assert!(err.contains("skills") && err.contains("missing"), "{err}");
}

#[test]
fn plugin_json_may_name_skill_and_command_paths() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".claude-plugin/plugin.json",
        r#"{"name":"p","skills":"./s","commands":["./c"]}"#,
    );
    write(dir.path(), "s/a/SKILL.md", SKILL);
    write(dir.path(), "c/k.md", "k");
    let checked = validate(dir.path(), &entry()).unwrap();
    assert_eq!(checked.permissions, ["claude.command:k", "claude.skill:a"]);
    assert_eq!(checked.declared.skills, ["./s"]);
    assert_eq!(checked.declared.commands, ["./c"]);
}

#[test]
fn a_new_skill_changes_the_hash_so_an_update_needs_approval() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "skills/a/SKILL.md", SKILL);
    let before = validate(dir.path(), &entry()).unwrap();
    write(dir.path(), "skills/b/SKILL.md", SKILL);
    let after = validate(dir.path(), &entry()).unwrap();
    assert_ne!(before.hash, after.hash);
    assert_eq!(
        added(&before.permissions, &after.permissions),
        ["claude.skill:b"]
    );
}

#[test]
fn a_plugin_without_skills_keeps_an_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "README.md", "hi");
    let checked = validate(dir.path(), &entry()).unwrap();
    assert!(checked.permissions.is_empty());
}
