use std::time::{Duration, Instant};

use townhall_core::model::{NewTask, TaskState};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{unique, Sched};

async fn create_as(s: &Sched, name: &str, agent: &str) {
    let new = NewTask {
        agent: agent.into(),
        ..s.new_task(name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

#[tokio::test]
async fn a_builder_task_runs_claude_with_the_types_flags() {
    let s = Sched::new().await;
    let name = unique("built");
    create_as(&s, &name, "builder").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    let lines: Vec<&str> = args.lines().collect();
    assert_eq!(
        lines[..2],
        ["--model=sonnet", "--permission-mode=acceptEdits"]
    );
    assert!(
        lines[2].starts_with("--append-system-prompt=You implement the task"),
        "{args}"
    );
    assert!(args.contains("Write a failing test first") && args.contains("\n--session-id="));
    let brief = s.brief(&name);
    let dir = format!("--add-dir={}", brief.parent().unwrap().display());
    let end = lines.iter().position(|l| *l == "--").unwrap();
    let isolation = end - 1 - townhall_core::agent::AGENT_ISOLATION.len();
    assert_eq!(
        lines[isolation..end - 1],
        townhall_core::agent::AGENT_ISOLATION
    );
    assert!(lines[end - 1].starts_with("--mcp-config="), "{args}");
    assert_eq!(lines[isolation - 1], dir);
    assert!(lines[end + 1].contains(&brief.display().to_string()));
    assert!(!args.contains("--tools="), "{args}");
    assert!(
        args.contains("--allowedTools=mcp__townhall__hand_back"),
        "{args}"
    );
}

#[tokio::test]
async fn a_task_runs_in_its_types_mode_and_accept_edits_when_it_sets_none() {
    let s = Sched::new().await;
    let source =
        |name: &str, mode: &str| format!("name = \"{name}\"\nadapter = \"claude\"\n{mode}");
    for (type_name, mode) in [
        ("modeless", ""),
        ("planner", "permission_mode = \"plan\"\n"),
        ("skipper", "permission_mode = \"bypassPermissions\"\n"),
    ] {
        s.store()
            .save_agent_type(&source(type_name, mode))
            .await
            .unwrap();
    }
    for (task, type_name, want) in [
        ("modeless", "modeless", "--permission-mode=acceptEdits\n"),
        ("planned", "planner", "--permission-mode=plan\n"),
        ("skipped", "skipper", "--dangerously-skip-permissions\n"),
    ] {
        let name = unique(task);
        create_as(&s, &name, type_name).await;
        run_once(s.ctx()).await.unwrap();
        let args = s.args(&name).await;
        assert!(args.contains(want), "{type_name}: {args}");
        if type_name != "skipper" {
            assert!(!args.contains("dangerously"), "{args}");
        }
    }
}

#[tokio::test]
async fn a_shell_type_runs_its_command() {
    let s = Sched::new().await;
    let source = "name = \"echoer\"\nadapter = \"shell\"\ncommand = \"echo ran > shell-ran.txt; exec sleep 30\"\n";
    s.store().save_agent_type(source).await.unwrap();
    let name = unique("shelled");
    create_as(&s, &name, "Echoer").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [name.as_str()], "{pass:?}");
    let agents = s.agents().await;
    assert_eq!(agents[0].kind.as_deref(), Some("shell"));
    let ran = s.worktree(&name).join("shell-ran.txt");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ran.exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "ran\n");
    let launch = s.store().task_launch(&name).await.unwrap().unwrap();
    assert!(launch.session_id.is_none());
}

#[tokio::test]
async fn a_task_whose_type_was_deleted_fails_with_the_reason() {
    let s = Sched::new().await;
    s.store()
        .save_agent_type("name = \"gone\"\nadapter = \"claude\"\n")
        .await
        .unwrap();
    let name = unique("orphan");
    create_as(&s, &name, "gone").await;
    s.store().delete_agent_type("gone").await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [name.as_str()], "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Error);
    let text = s.failed_text(&name).await;
    assert!(
        text.contains("agent type `gone`") && text.contains("deleted"),
        "{text}"
    );
    assert!(s.agents().await.is_empty());
}
