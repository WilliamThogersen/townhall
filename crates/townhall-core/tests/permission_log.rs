use townhall_core::model::{NewPermissionDecision, PermissionVerdict};

mod common;
use common::store_env;

fn decision(agent_type: &str, pattern: &str, verdict: PermissionVerdict) -> NewPermissionDecision {
    NewPermissionDecision {
        task: "fix-login".into(),
        agent_type: agent_type.into(),
        command: format!(
            "Bash: {}",
            pattern.trim_start_matches("Bash(").trim_end_matches(" *)")
        ),
        pattern: pattern.into(),
        verdict,
        reason: "read-only".into(),
    }
}

#[tokio::test]
async fn every_decision_is_logged_newest_first() {
    let env = store_env();
    let store = &env.store;
    let first = decision("builder", "Bash(git status *)", PermissionVerdict::Allow);
    store.record_permission_decision(first).await.unwrap();
    let second = decision("builder", "Bash(rm *)", PermissionVerdict::HardDeny);
    store.record_permission_decision(second).await.unwrap();
    let log = store.permission_decisions(10).await.unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].verdict, PermissionVerdict::HardDeny);
    assert_eq!(log[1].verdict, PermissionVerdict::Allow);
    assert_eq!(log[1].agent_type, "builder");
    assert_eq!(log[1].command, "Bash: git status");
    assert_eq!(log[1].reason, "read-only");
    assert!(!log[1].created_at.is_empty());
    assert_eq!(store.permission_decisions(1).await.unwrap().len(), 1);
}

#[tokio::test]
async fn three_approvals_of_one_pattern_for_one_type_make_a_suggestion() {
    let env = store_env();
    let store = &env.store;
    for _ in 0..2 {
        let d = decision("builder", "Bash(git status *)", PermissionVerdict::Allow);
        store.record_permission_decision(d).await.unwrap();
    }
    assert!(store.permission_suggestions().await.unwrap().is_empty());
    let third = decision("builder", "Bash(git status *)", PermissionVerdict::Allow);
    store.record_permission_decision(third).await.unwrap();
    let other_type = decision("tester", "Bash(git status *)", PermissionVerdict::Allow);
    store.record_permission_decision(other_type).await.unwrap();
    let denied = decision("tester", "Bash(git status *)", PermissionVerdict::Deny);
    store.record_permission_decision(denied).await.unwrap();
    let suggestions = store.permission_suggestions().await.unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].agent_type, "builder");
    assert_eq!(suggestions[0].tool, "Bash(git status *)");
    assert_eq!(suggestions[0].approvals, 3);
}

#[tokio::test]
async fn a_pattern_the_type_already_has_is_not_suggested_and_blank_patterns_never_are() {
    let env = store_env();
    let store = &env.store;
    let tool = "Bash(git status *)";
    let source = format!("name = \"builder\"\nadapter = \"claude\"\ntools = [\"{tool}\"]\n");
    store.save_agent_type(&source).await.unwrap();
    for _ in 0..3 {
        let blank = decision("builder", "", PermissionVerdict::Allow);
        store.record_permission_decision(blank).await.unwrap();
        let known = decision("builder", tool, PermissionVerdict::Allow);
        store.record_permission_decision(known).await.unwrap();
    }
    assert!(store.permission_suggestions().await.unwrap().is_empty());
}
