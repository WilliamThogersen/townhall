use serde_json::json;
use townhall_core::model::NewTask;
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{unique, Sched};
mod plugin_support;
use plugin_support::install_plain;

const SKILL: &str = "---\nname: guide\ndescription: d\n---\nuse ${CLAUDE_PLUGIN_ROOT}/scripts/x.sh";
const PATH_SOURCE: &str = r#"{"source":"path","path":"./"}"#;

async fn create_as(s: &Sched, name: &str, agent: &str) {
    let new = NewTask {
        agent: agent.into(),
        ..s.new_task(name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

async fn save_type(s: &Sched, name: &str, plugins: &str) {
    let source = format!("name = \"{name}\"\nadapter = \"claude\"\n{plugins}");
    s.store().save_agent_type(&source).await.unwrap();
}

async fn skills_plugin(s: &Sched, enabled: bool) {
    install_plain(
        s.store(),
        "web",
        "acme",
        &[
            (
                ".claude-plugin/plugin.json",
                r#"{"name":"web","version":"1.0.0"}"#,
            ),
            ("skills/guide/SKILL.md", SKILL),
            ("skills/guide/references/a.md", "ref"),
            ("commands/deploy.md", "deploy"),
            ("scripts/x.sh", "#!/bin/sh\n"),
        ],
        serde_json::from_str(PATH_SOURCE).unwrap(),
        enabled,
    )
    .await;
}

fn plugin_dirs(args: &str) -> Vec<String> {
    args.lines()
        .filter_map(|l| l.strip_prefix("--plugin-dir="))
        .map(String::from)
        .collect()
}

#[tokio::test]
async fn a_type_that_lists_a_skills_plugin_launches_with_its_folder() {
    let s = Sched::new().await;
    skills_plugin(&s, true).await;
    save_type(&s, "webby", "plugins = [\"web@acme\"]\n").await;
    let name = unique("skilled");
    create_as(&s, &name, "webby").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");
    let args = s.args(&name).await;
    let dirs = plugin_dirs(&args);
    assert_eq!(dirs.len(), 1, "{args}");
    let dir = std::path::PathBuf::from(&dirs[0]);
    assert!(dir.starts_with(s.brief(&name).parent().unwrap().join("plugins")));
    assert!(dir.join("skills/guide/SKILL.md").is_file());
    assert!(dir.join("skills/guide/references/a.md").is_file());
    assert!(dir.join("commands/deploy.md").is_file());
    assert!(dir.join("scripts/x.sh").exists());
    let manifest = std::fs::read_to_string(dir.join(".claude-plugin/plugin.json")).unwrap();
    assert!(manifest.contains("\"name\":\"web\""), "{manifest}");
    assert!(args.contains("--setting-sources=\n"), "{args}");
    assert!(args.contains("--strict-mcp-config\n"), "{args}");
    assert!(args.contains("disableAllHooks"), "{args}");
}

#[tokio::test]
async fn a_type_without_plugins_gets_the_same_flags_as_before() {
    let s = Sched::new().await;
    skills_plugin(&s, true).await;
    let name = unique("plain");
    create_as(&s, &name, "builder").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    assert!(!args.contains("plugin"), "{args}");
    assert!(!s.brief(&name).parent().unwrap().join("plugins").exists());
}

#[tokio::test]
async fn a_disabled_skills_plugin_adds_nothing() {
    let s = Sched::new().await;
    skills_plugin(&s, false).await;
    save_type(&s, "webby", "plugins = [\"web@acme\"]\n").await;
    let name = unique("off");
    create_as(&s, &name, "webby").await;
    run_once(s.ctx()).await.unwrap();
    assert!(plugin_dirs(&s.args(&name).await).is_empty());
}

#[tokio::test]
async fn the_staged_folder_leaves_out_what_townhall_injects_itself() {
    let s = Sched::new().await;
    install_plain(
        s.store(),
        "web",
        "acme",
        &[
            (".claude-plugin/plugin.json", r#"{"name":"web"}"#),
            ("skills/guide/SKILL.md", SKILL),
            ("hooks/hooks.json", r#"{"hooks":{}}"#),
            (".mcp.json", r#"{"mcpServers":{}}"#),
            ("agents/helper.md", "helper"),
            ("README.md", "hi"),
        ],
        serde_json::from_str(PATH_SOURCE).unwrap(),
        true,
    )
    .await;
    save_type(&s, "webby", "plugins = [\"web@acme\"]\n").await;
    let name = unique("lean");
    create_as(&s, &name, "webby").await;
    run_once(s.ctx()).await.unwrap();
    let dirs = plugin_dirs(&s.args(&name).await);
    let dir = std::path::PathBuf::from(&dirs[0]);
    assert!(dir.join("skills/guide/SKILL.md").is_file());
    assert!(dir.join("README.md").exists());
    for left_out in ["hooks", ".mcp.json", "agents"] {
        assert!(!dir.join(left_out).exists(), "{left_out}");
    }
    let manifest = std::fs::read_to_string(dir.join(".claude-plugin/plugin.json")).unwrap();
    assert!(
        !manifest.contains("hooks") && !manifest.contains("mcp"),
        "{manifest}"
    );
}

#[tokio::test]
async fn a_strict_false_entry_stages_only_the_skill_folders_it_names() {
    let s = Sched::new().await;
    install_plain(
        s.store(),
        "pack",
        "acme",
        &[
            ("local-ai/SKILL.md", SKILL),
            ("other/SKILL.md", SKILL),
            ("cmds/go.md", "go"),
        ],
        json!({"source": "path", "path": "./", "skills": ["./local-ai"], "commands": ["./cmds"]}),
        true,
    )
    .await;
    save_type(&s, "packer", "plugins = [\"pack\"]\n").await;
    let name = unique("named");
    create_as(&s, &name, "packer").await;
    run_once(s.ctx()).await.unwrap();
    let dirs = plugin_dirs(&s.args(&name).await);
    let dir = std::path::PathBuf::from(&dirs[0]);
    assert!(dir.join("local-ai/SKILL.md").is_file());
    assert!(dir.join("cmds/go.md").is_file());
    let manifest = std::fs::read_to_string(dir.join(".claude-plugin/plugin.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(parsed["skills"], json!(["./local-ai"]));
    assert_eq!(parsed["commands"], json!(["./cmds"]));
}

#[tokio::test]
async fn a_plugin_with_nothing_to_stage_adds_no_flag() {
    let s = Sched::new().await;
    install_plain(
        s.store(),
        "bare",
        "acme",
        &[("README.md", "hi")],
        serde_json::from_str(PATH_SOURCE).unwrap(),
        true,
    )
    .await;
    save_type(&s, "bare-type", "plugins = [\"bare\"]\n").await;
    let name = unique("bare");
    create_as(&s, &name, "bare-type").await;
    run_once(s.ctx()).await.unwrap();
    assert!(plugin_dirs(&s.args(&name).await).is_empty());
}
