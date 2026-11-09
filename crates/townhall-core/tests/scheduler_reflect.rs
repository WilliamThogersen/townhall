use serde_json::json;
use townhall_core::definitions::PermissionMode;
use townhall_core::model::{EntryBody, SettingKey, TaskState};
use townhall_core::scheduler::reflect::{REFLECTOR, REFLECTOR_SOURCE};
use townhall_core::scheduler::run_once;
use townhall_core::store::Store;

mod scheduler_support;
use scheduler_support::{commit, unique, Sched};

async fn reflect_on(s: &Sched) {
    let on = s
        .store()
        .set_setting(SettingKey::ReflectAfterTasks, json!(true));
    on.await.unwrap();
}

/// A task that did some work and ended, as the agent watcher leaves it.
async fn ended_task(s: &Sched, prefix: &str) -> String {
    let name = unique(prefix);
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    s.args(&name).await;
    commit(&s.worktree(&name), "the work").await;
    s.end(&name).await;
    std::fs::remove_file(s.args_dir.join(format!("{name}.args"))).unwrap();
    name
}

#[tokio::test]
async fn an_ended_task_is_left_alone_while_the_setting_is_off() {
    let s = Sched::new().await;
    let name = ended_task(&s, "quiet").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Ended);
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
    assert!(s.store().agent_type(REFLECTOR).await.unwrap().is_none());
}

#[tokio::test]
async fn an_ended_task_is_reflected_on_once_in_its_own_worktree() {
    let s = Sched::new().await;
    reflect_on(&s).await;
    let name = ended_task(&s, "learn").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, std::slice::from_ref(&name), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Working);
    let task = s.store().find_task(&name).await.unwrap().unwrap();
    assert_eq!(task.agent, REFLECTOR);
    assert_eq!(s.agent_names().await, std::slice::from_ref(&name));
    let handoffs = s.store().handoffs(&name).await.unwrap();
    assert_eq!(handoffs.len(), 1);
    assert_eq!(
        (handoffs[0].from.as_str(), handoffs[0].to.as_str()),
        ("plain", REFLECTOR)
    );
    assert!(
        handoffs[0].summary.contains("the work"),
        "{}",
        handoffs[0].summary
    );
    let args = s.args(&name).await;
    assert!(args.contains("--model=haiku"), "{args}");
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert!(brief.contains("CLAUDE.md"), "{brief}");
    assert!(brief.contains("the work"), "{brief}");
    let entries = s.store().entries(&s.conversation).await.unwrap();
    assert!(entries.iter().any(|e| matches!(
        &e.body,
        EntryBody::Handoff { to, .. } if to == REFLECTOR
    )));

    s.end(&name).await;
    let again = run_once(s.ctx()).await.unwrap();
    assert!(again.started.is_empty(), "{again:?}");
    assert_eq!(s.state(&name).await, TaskState::Ended);
    assert_eq!(s.store().handoffs(&name).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_task_reflected_on_and_handed_back_is_not_reflected_on_again() {
    let s = Sched::new().await;
    reflect_on(&s).await;
    let name = ended_task(&s, "again").await;
    run_once(s.ctx()).await.unwrap();
    s.args(&name).await;
    s.store()
        .hand_back(&name, "plain", "the lint check fails", "")
        .await
        .unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, std::slice::from_ref(&name), "{pass:?}");
    s.end(&name).await;
    let later = run_once(s.ctx()).await.unwrap();
    assert!(
        later.started.is_empty() && later.reflected.is_empty(),
        "{later:?}"
    );
    assert_eq!(s.state(&name).await, TaskState::Ended);
    let to: Vec<String> = s
        .store()
        .handoffs(&name)
        .await
        .unwrap()
        .into_iter()
        .map(|h| h.to)
        .collect();
    assert_eq!(to, [REFLECTOR, "plain"]);
}

#[tokio::test]
async fn a_type_the_user_edited_is_kept() {
    let s = Sched::new().await;
    reflect_on(&s).await;
    let mine = REFLECTOR_SOURCE.replace("haiku", "sonnet");
    s.store().save_agent_type(&mine).await.unwrap();
    let name = ended_task(&s, "mine").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    assert!(args.contains("--model=sonnet"), "{args}");
}

#[test]
fn the_shipped_reflector_is_cheap_and_may_only_edit_the_memory_files() {
    let parsed = Store::check_agent_type(REFLECTOR_SOURCE).unwrap();
    let def = parsed.def;
    assert_eq!(def.name, REFLECTOR);
    assert_eq!(def.model.as_deref(), Some("haiku"));
    assert_eq!(def.permission_mode, Some(PermissionMode::Default));
    assert_eq!(def.can_hand_back_to, Some(vec![]));
    let editing: Vec<&String> = def
        .tools
        .iter()
        .filter(|t| t.starts_with("Edit") || t.starts_with("Write"))
        .collect();
    assert!(!editing.is_empty());
    for tool in editing {
        assert!(
            tool.ends_with("(/CLAUDE.md)") || tool.ends_with("(/docs/lessons.md)"),
            "{tool}"
        );
    }
    for tool in &def.tools {
        assert!(!tool.starts_with("Bash(git push"), "{tool}");
        assert!(!tool.contains("--force"), "{tool}");
    }
    let prompt = def.prompt.unwrap().to_lowercase();
    for rule in ["claude.md", "docs/lessons.md", "duplicate", "concise"] {
        assert!(prompt.contains(rule), "{rule}: {prompt}");
    }
    assert!(!REFLECTOR_SOURCE.contains('\u{2014}') && !REFLECTOR_SOURCE.contains('\u{2013}'));
}
