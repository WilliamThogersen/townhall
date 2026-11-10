use townhall_core::model::{NewTask, TaskState};
use townhall_core::scheduler::run_once;
use townhall_core::store::Plugin;

mod scheduler_support;
use scheduler_support::{unique, Sched};
mod plugin_support;
use plugin_support::{everything, install, install_files, SCHEMA, STARTERS, THEME, WRITER};

async fn create_as(s: &Sched, name: &str, agent: &str) {
    let new = NewTask {
        agent: agent.into(),
        ..s.new_task(name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

#[tokio::test]
async fn a_task_of_a_plugin_type_starts_with_the_types_flags() {
    let s = Sched::new().await;
    install(
        s.store(),
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        true,
    )
    .await;
    let name = unique("noted");
    create_as(&s, &name, "notes-writer").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");
    let args = s.args(&name).await;
    assert!(
        args.starts_with("--model=haiku\n--permission-mode=plan\n"),
        "{args}"
    );
    assert!(args.contains("You write release notes."), "{args}");
}

async fn install_with_brief(s: &Sched, enabled: bool) {
    let mut manifest = everything("notes");
    manifest["contributes"]["agentTypes"] = serde_json::json!(["agent-types/writer.toml"]);
    install_files(
        s.store(),
        "notes",
        "acme",
        manifest,
        &[
            ("agent-types/writer.toml", WRITER),
            ("prompts/starters.json", STARTERS),
            ("prompts/brief.md", "always write a changelog line\n"),
            ("prompts/planner.md", "prefer small tasks"),
            ("settings.schema.json", SCHEMA),
            ("theme.css", THEME),
        ],
        enabled,
    )
    .await;
}

fn plain_brief(name: &str) -> String {
    format!("# do {name}\n\nthe brief for {name}\n")
}

#[tokio::test]
async fn a_task_whose_type_selects_a_plugin_gets_its_brief_section() {
    let s = Sched::new().await;
    install_with_brief(&s, true).await;
    let name = unique("sectioned");
    create_as(&s, &name, "notes-writer").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");
    let text = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert_eq!(
        text,
        format!(
            "{}\n## From plugin notes@acme\n\nalways write a changelog line\n",
            plain_brief(&name)
        )
    );
}

#[tokio::test]
async fn a_task_of_a_type_that_does_not_select_the_plugin_keeps_its_brief_byte_equal() {
    let s = Sched::new().await;
    install_with_brief(&s, true).await;
    let name = unique("plain");
    let new = s.new_task(&name, &[]);
    s.store().create_task(&s.conversation, new).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");
    let text = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert_eq!(text, plain_brief(&name));
}

#[tokio::test]
async fn with_no_plugin_installed_the_brief_is_the_plain_one() {
    let s = Sched::new().await;
    let name = unique("bare");
    let new = s.new_task(&name, &[]);
    s.store().create_task(&s.conversation, new).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    let text = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert_eq!(text, plain_brief(&name));
}

#[tokio::test]
async fn a_disabled_plugin_makes_its_types_task_fail_naming_the_plugin() {
    let s = Sched::new().await;
    let plugin = install(
        s.store(),
        "notes",
        "acme",
        &[("writer", WRITER.into())],
        true,
    )
    .await;
    let name = unique("orphaned");
    create_as(&s, &name, "notes-writer").await;
    s.store()
        .update_plugin(Plugin {
            enabled: false,
            ..plugin
        })
        .await
        .unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [name.as_str()], "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Error);
    let text = s.failed_text(&name).await;
    assert!(
        text.contains("agent type `notes-writer`")
            && text.contains("notes@acme")
            && text.contains("disabled"),
        "{text}"
    );
    assert!(s.agents().await.is_empty());
}
