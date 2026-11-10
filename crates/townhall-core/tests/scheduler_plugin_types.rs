use townhall_core::model::{NewTask, TaskState};
use townhall_core::scheduler::run_once;
use townhall_core::store::Plugin;

mod scheduler_support;
use scheduler_support::{unique, Sched};
mod plugin_support;
use plugin_support::{install, WRITER};

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
