use std::time::Duration;

use serde_json::json;
use townhall_core::agent::{self, LaunchOptions, ShellAdapter};
use townhall_core::ids::is_session_id;
use townhall_core::model::{
    AddRepo, AgentEvent, EntryBody, PermissionMode, ServerEvent, SettingKey, Task, TaskState,
};
use townhall_core::scheduler::{self, run_once};

mod scheduler_support;
use scheduler_support::{commit, git, unique, Sched};

async fn last_entry(s: &Sched) -> EntryBody {
    let entries = s.store().entries(&s.conversation).await.unwrap();
    entries.last().unwrap().body.clone()
}

fn names(agents: &[agent::Agent]) -> Vec<String> {
    let mut all: Vec<String> = agents.iter().map(|a| a.name.clone()).collect();
    all.sort();
    all
}

#[tokio::test]
async fn a_task_starts_in_its_own_worktree_with_the_settings_permission_mode() {
    let s = Sched::new().await;
    let file = s
        .store()
        .save_attachment(&s.conversation, "shot.png", "image/png", vec![1, 2])
        .await
        .unwrap();
    let name = unique("first");
    let new = townhall_core::model::NewTask {
        attachments: vec![file.id.clone()],
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let skip = json!("bypassPermissions");
    s.store()
        .set_setting(SettingKey::PermissionMode, skip)
        .await
        .unwrap();

    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Working);
    let agents = s.agents().await;
    assert_eq!(names(&agents), [name.as_str()]);
    assert!(agents[0].alive);
    assert_eq!(agents[0].branch, format!("townhall/{name}"));
    let head = git(&s.worktree(&name), &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(head, format!("townhall/{name}"));

    let (_, path) = s
        .store()
        .attachment(&s.conversation, &file.id)
        .await
        .unwrap();
    let args = s.args(&name).await;
    let lines: Vec<&str> = args.lines().collect();
    assert_eq!(lines[0], "--dangerously-skip-permissions");
    assert_eq!(lines[1], "--session-id");
    assert!(is_session_id(lines[2]), "{args}");
    let prompt = lines[3..].join("\n");
    assert_eq!(
        prompt,
        format!(
            "the brief for {name}\n\nFiles attached to this task:\n- {}",
            path.display()
        )
    );
    let launch = s.store().task_launch(&name).await.unwrap().unwrap();
    assert_eq!(launch.session_id.as_deref(), Some(lines[2]));
    assert_eq!(
        launch.start_commit,
        git(&s.repo, &["rev-parse", "main"]).await
    );

    let again = run_once(s.ctx()).await.unwrap();
    assert!(again.started.is_empty() && again.adopted.is_empty());
    assert_eq!(s.agents().await.len(), 1);
}

#[tokio::test]
async fn the_default_permission_mode_is_passed_as_a_flag() {
    let s = Sched::new().await;
    let name = unique("plain");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    let lines: Vec<&str> = args.lines().collect();
    let mode = PermissionMode::AcceptEdits.cli_value().unwrap();
    assert_eq!(lines[..2], ["--permission-mode", mode]);
    assert_eq!(lines[2], "--session-id");
    assert_eq!(lines[4], format!("the brief for {name}"));
    assert_eq!(lines.len(), 5);
}

#[tokio::test]
async fn after_chains_wait_until_every_task_before_finished() {
    let s = Sched::new().await;
    let (a, b, c) = (unique("a"), unique("b"), unique("c"));
    let (x, y) = (unique("x"), unique("y"));
    s.create(&a, &[]).await;
    s.create(&b, &[&a]).await;
    s.create(&c, &[&a, &b]).await;
    s.create(&x, &[]).await;
    s.set_state(&x, TaskState::Error).await;
    s.create(&y, &[&x]).await;

    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [a.as_str()]);
    for queued in [&b, &c, &y] {
        assert_eq!(s.state(queued).await, TaskState::Queued, "{queued}");
    }
    s.end(&a).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, [b.as_str()]);
    assert_eq!(s.state(&c).await, TaskState::Queued);
    s.set_state(&b, TaskState::Merged).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, [c.as_str()]);
    assert_eq!(
        s.state(&y).await,
        TaskState::Queued,
        "an after task in error holds it"
    );
}

#[tokio::test]
async fn agents_at_once_caps_live_task_agents_oldest_first() {
    let s = Sched::new().await;
    s.store()
        .set_setting(SettingKey::AgentsAtOnce, json!(2))
        .await
        .unwrap();
    let names_in_order: Vec<String> = (0..3).map(|i| unique(&format!("cap{i}"))).collect();
    for name in &names_in_order {
        s.create(name, &[]).await;
    }
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, names_in_order[..2]);
    assert_eq!(s.state(&names_in_order[2]).await, TaskState::Queued);
    assert!(run_once(s.ctx()).await.unwrap().started.is_empty());

    s.end(&names_in_order[0]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, names_in_order[2..]);
    let alive = s.agents().await.iter().filter(|a| a.alive).count();
    assert_eq!(alive, 2);
}

#[tokio::test]
async fn a_start_that_fails_sets_error_with_the_reason() {
    let s = Sched::new().await;
    let name = unique("broken");
    let new = townhall_core::model::NewTask {
        base_branch: "no-such-branch".into(),
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Error);
    match last_entry(&s).await {
        EntryBody::Agent { agent, event, text } => {
            assert_eq!((agent.as_str(), event), (name.as_str(), AgentEvent::Failed));
            assert!(text.contains("no-such-branch"), "{text}");
        }
        other => panic!("expected an agent entry, got {other:?}"),
    }
    assert!(s.agents().await.is_empty());
    let entries = s.store().entries(&s.conversation).await.unwrap().len();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(
        s.store().entries(&s.conversation).await.unwrap().len(),
        entries
    );
}

#[tokio::test]
async fn merged_is_detected_after_a_real_merge_and_never_merges_anything() {
    let s = Sched::new().await;
    let (done, idle) = (unique("done"), unique("idle"));
    s.create(&done, &[]).await;
    s.create(&idle, &[]).await;
    run_once(s.ctx()).await.unwrap();
    commit(&s.worktree(&done), "the work").await;
    s.end(&done).await;
    s.end(&idle).await;
    let main_before = git(&s.repo, &["rev-parse", "main"]).await;

    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.merged.is_empty());
    assert_eq!(s.state(&done).await, TaskState::Ended);
    assert_eq!(git(&s.repo, &["rev-parse", "main"]).await, main_before);

    let branch = format!("townhall/{done}");
    let merge = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "merge",
        "-q",
        "--no-ff",
        "-m",
        "merge it",
        &branch,
    ];
    git(&s.repo, &merge).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.merged, [done.as_str()]);
    assert_eq!(s.state(&done).await, TaskState::Merged);
    assert_eq!(
        s.state(&idle).await,
        TaskState::Ended,
        "a branch with no commits of its own is not merged"
    );
    match last_entry(&s).await {
        EntryBody::Agent { agent, event, text } => {
            assert_eq!((agent.as_str(), event), (done.as_str(), AgentEvent::Passed));
            assert!(text.contains("main"), "{text}");
        }
        other => panic!("expected an agent entry, got {other:?}"),
    }
    assert!(run_once(s.ctx()).await.unwrap().merged.is_empty());
    let branches = git(&s.repo, &["branch", "--list", &branch]).await;
    assert!(branches.contains(&branch), "the branch is kept");
}

#[tokio::test]
async fn a_restart_never_starts_a_second_agent() {
    let mut s = Sched::new().await;
    let (running, crashed) = (unique("running"), unique("crashed"));
    s.create(&running, &[]).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, [running.as_str()]);
    s.create(&crashed, &[]).await;
    let options = LaunchOptions {
        command: Some("sleep 120".into()),
        ..LaunchOptions::default()
    };
    agent::start(&s.ws, &crashed, "main", &ShellAdapter, &options)
        .await
        .unwrap();

    s.restart().await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    assert_eq!(pass.adopted, [crashed.as_str()]);
    assert_eq!(s.state(&running).await, TaskState::Working);
    assert_eq!(s.state(&crashed).await, TaskState::Working);
    let mut want = vec![running.clone(), crashed.clone()];
    want.sort();
    assert_eq!(names(&s.agents().await), want);
    let launch = s.store().task_launch(&crashed).await.unwrap().unwrap();
    assert_eq!(
        launch.start_commit,
        git(&s.repo, &["rev-parse", "main"]).await
    );

    s.restart().await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(
        pass.started.is_empty() && pass.adopted.is_empty(),
        "{pass:?}"
    );
    assert_eq!(s.agents().await.len(), 2);
}

#[tokio::test]
async fn the_loop_wakes_on_task_updates() {
    let s = Sched::new().await;
    let mut events = s.ctx().bus().subscribe();
    let handle = scheduler::start(s.ctx()).expect("the scheduler runs");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let name = unique("woken");
    s.create(&name, &[]).await;
    let working = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if let Ok(ServerEvent::TaskUpdated { task }) = events.recv().await {
                if task.name == name && task.state == TaskState::Working {
                    return task;
                }
            }
        }
    })
    .await;
    handle.abort();
    assert!(
        working.is_ok(),
        "the task did not start before the slow tick"
    );
}

#[tokio::test]
async fn a_name_another_repo_has_is_an_error_and_bad_bases_are_warned() {
    let s = Sched::new().await;
    let other_path = s.root.path().join(unique("th23-other"));
    std::fs::create_dir_all(&other_path).unwrap();
    git(&other_path, &["init", "-q", "-b", "main"]).await;
    commit(&other_path, "init").await;
    let add = AddRepo {
        path: other_path.to_string_lossy().into_owned(),
        ..AddRepo::default()
    };
    let other = s.ctx().registry().unwrap().add(&add).await.unwrap();
    let other_ws = s.ctx().workspace(&other).unwrap();
    let name = unique("taken");
    let options = LaunchOptions {
        command: Some("sleep 120".into()),
        ..LaunchOptions::default()
    };
    agent::start(&other_ws, &name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await;
    townhall_core::tmux::kill(other_ws.key(), &name)
        .await
        .unwrap();
    assert_eq!(pass.unwrap().failed, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Error);
    match last_entry(&s).await {
        EntryBody::Agent { text, .. } => assert!(text.contains(&other.name), "{text}"),
        other => panic!("expected an agent entry, got {other:?}"),
    }

    let ended = unique("ended");
    s.create(&ended, &[]).await;
    run_once(s.ctx()).await.unwrap();
    s.end(&ended).await;
    let change = |t: Task| {
        Ok(Task {
            base_branch: "-oops".into(),
            ..t
        })
    };
    s.store().update_task(&ended, change).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.merged.is_empty());
    assert!(
        pass.warnings.iter().any(|w| w.contains("-oops")),
        "{:?}",
        pass.warnings
    );
}
