use std::path::Path;

use serde_json::json;
use townhall_core::ids::is_session_id;
use townhall_core::model::{NewTask, SettingKey, TaskState};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{git, sleeper, unique, Sched};

fn brief_order(brief: &Path) -> String {
    format!(
        "read your task brief at {} and do it. it is the full task.",
        brief.display()
    )
}

#[tokio::test]
async fn a_task_starts_in_its_own_worktree_from_its_base() {
    let s = Sched::new().await;
    let name = unique("first");
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Working);
    let agents = s.agents().await;
    assert_eq!(s.agent_names().await, [name.as_str()]);
    assert!(agents[0].alive);
    assert_eq!(agents[0].branch, format!("townhall/{name}"));
    let head = git(&s.worktree(&name), &["rev-parse", "--abbrev-ref", "HEAD"]).await;
    assert_eq!(head, format!("townhall/{name}"));
    let main = git(&s.repo, &["rev-parse", "main"]).await;
    let launch = s.store().task_launch(&name).await.unwrap().unwrap();
    assert_eq!(launch.start_commit, main);

    let again = run_once(s.ctx()).await.unwrap();
    assert!(again.started.is_empty() && again.adopted.is_empty());
    assert_eq!(s.agents().await.len(), 1);
}

#[tokio::test]
async fn the_agent_reads_its_brief_with_the_settings_permission_mode() {
    let s = Sched::new().await;
    let name = unique("plain");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    let lines: Vec<&str> = args.lines().collect();
    let brief = s.brief(&name);
    assert!(brief.is_absolute() && brief.starts_with(s.ws.home().root()));
    assert!(!brief.starts_with(&s.repo) && !brief.starts_with(s.worktree(&name)));
    assert_eq!(lines[0], "--permission-mode=acceptEdits");
    assert!(
        lines[1].starts_with("--append-system-prompt=you can hand back to: builder."),
        "{args}"
    );
    let id = lines[2].strip_prefix("--session-id=").unwrap();
    assert!(is_session_id(id), "{args}");
    let dir = format!("--add-dir={}", brief.parent().unwrap().display());
    let order = brief_order(&brief);
    let mcp = format!(
        "--mcp-config={}",
        townhall_core::mcp::task_config_json(s.ctx().base_url(), &name)
    );
    let rest: Vec<&str> = [dir.as_str()]
        .into_iter()
        .chain(townhall_core::agent::AGENT_ISOLATION)
        .chain([mcp.as_str(), "--", order.as_str()])
        .collect();
    assert_eq!(
        lines[3],
        "--allowedTools=mcp__townhall__hand_back,mcp__townhall__open_pr,mcp__townhall__add_artifact,mcp__townhall__list_artifacts"
    );
    assert_eq!(lines[4..], rest);
    let text = std::fs::read_to_string(&brief).unwrap();
    assert_eq!(text, format!("# do {name}\n\nthe brief for {name}\n"));
    let launch = s.store().task_launch(&name).await.unwrap().unwrap();
    assert_eq!(launch.session_id.as_deref(), Some(id));
}

#[tokio::test]
async fn attachments_are_listed_in_the_brief_and_readable() {
    let s = Sched::new().await;
    let file = s
        .store()
        .save_attachment(&s.conversation, "shot.png", "image/png", vec![1, 2])
        .await
        .unwrap();
    let name = unique("files");
    let new = NewTask {
        attachments: vec![file.id.clone()],
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let skip = json!("bypassPermissions");
    let store = s.store();
    store
        .set_setting(SettingKey::PermissionMode, skip)
        .await
        .unwrap();
    run_once(s.ctx()).await.unwrap();

    let (_, path) = store.attachment(&s.conversation, &file.id).await.unwrap();
    let files_dir = path.parent().unwrap().parent().unwrap();
    assert!(files_dir.ends_with("files"), "{files_dir:?}");
    let args = s.args(&name).await;
    let lines: Vec<&str> = args.lines().collect();
    assert_eq!(lines[0], "--dangerously-skip-permissions");
    let brief = s.brief(&name);
    let dirs = [
        format!("--add-dir={}", brief.parent().unwrap().display()),
        format!("--add-dir={}", files_dir.display()),
    ];
    assert_eq!(lines[4..6], dirs);
    let text = std::fs::read_to_string(&brief).unwrap();
    let listed = format!("\n\nFiles attached to this task:\n- {}\n", path.display());
    assert!(text.ends_with(&listed), "{text}");
}

#[tokio::test]
async fn long_and_hostile_prompts_start_from_the_brief() {
    let s = Sched::new().await;
    let (long, hostile) = (unique("long"), unique("hostile"));
    let big = "a long brief line\n".repeat(2_700);
    assert!(big.len() > 40_000);
    let new = NewTask {
        prompt: big.clone(),
        ..s.new_task(&long, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let new = NewTask {
        prompt: "--dangerously-skip-permissions".into(),
        ..s.new_task(&hostile, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [long.as_str(), hostile.as_str()], "{pass:?}");

    let text = std::fs::read_to_string(s.brief(&long)).unwrap();
    assert!(text.contains(&big));
    let args = s.args(&hostile).await;
    let lines: Vec<&str> = args.lines().collect();
    let end = lines.iter().position(|l| *l == "--").unwrap();
    assert!(!lines[..end].contains(&"--dangerously-skip-permissions"));
    assert_eq!(lines[end + 1..], [brief_order(&s.brief(&hostile))]);
    let brief = std::fs::read_to_string(s.brief(&hostile)).unwrap();
    assert!(
        brief.contains("\n--dangerously-skip-permissions\n"),
        "{brief}"
    );
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
    let held = s.state(&y).await;
    assert_eq!(held, TaskState::Queued, "an after task in error holds it");
}

#[tokio::test]
async fn agents_at_once_caps_active_task_agents_oldest_first() {
    let s = Sched::new().await;
    s.store()
        .set_setting(SettingKey::AgentsAtOnce, json!(2))
        .await
        .unwrap();
    let order: Vec<String> = (0..4).map(|i| unique(&format!("cap{i}"))).collect();
    for name in &order {
        s.create(name, &[]).await;
    }
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, order[..2]);
    s.set_state(&order[1], TaskState::NeedsYou).await;
    assert!(run_once(s.ctx()).await.unwrap().started.is_empty());

    s.end(&order[0]).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, order[2..3]);
    s.set_state(&order[2], TaskState::Ended).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(
        pass.started,
        order[3..],
        "an ended task's idle session frees its slot"
    );
    let alive = s.agents().await.iter().filter(|a| a.alive).count();
    assert_eq!(alive, 3);
}

#[tokio::test]
async fn an_agent_named_like_a_task_but_for_case_is_never_its_agent() {
    let s = Sched::new().await;
    s.store()
        .set_setting(SettingKey::AgentsAtOnce, json!(1))
        .await
        .unwrap();
    let busy = unique("busy");
    s.create(&busy, &[]).await;
    s.set_state(&busy, TaskState::Working).await;
    sleeper(&s.ws, &busy.to_uppercase()).await;
    let next = unique("next");
    s.create(&next, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [next.as_str()], "`{busy}` has no live agent");

    s.end(&next).await;
    let clash = unique("clash");
    sleeper(&s.ws, &clash.to_uppercase()).await;
    s.create(&clash, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.adopted.is_empty(), "{pass:?}");
    assert_eq!(pass.failed, [clash.as_str()]);
    let text = s.failed_text(&clash).await;
    assert!(text.contains(&clash.to_uppercase()), "{text}");
}

#[tokio::test]
async fn a_start_that_fails_sets_error_with_a_short_reason() {
    let s = Sched::new().await;
    let name = unique("broken");
    let new = NewTask {
        base_branch: "no-such-branch".into(),
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Error);
    let text = s.failed_text(&name).await;
    assert!(text.contains("no-such-branch"), "{text}");
    assert!(s.agents().await.is_empty());
    let entries = s.store().entries(&s.conversation).await.unwrap().len();
    run_once(s.ctx()).await.unwrap();
    let after = s.store().entries(&s.conversation).await.unwrap().len();
    assert_eq!(after, entries);

    let long = format!("x{}", "y".repeat(2_000));
    let name = unique("long-base");
    let new = NewTask {
        base_branch: long,
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    let text = s.failed_text(&name).await;
    assert!(text.chars().count() <= 300, "{}", text.len());
    assert!(text.starts_with("could not start: ") && text.ends_with("..."));
}

/// tmux refuses a command line this long; its reason must survive the 300 character cap.
#[tokio::test]
async fn a_start_tmux_refuses_keeps_the_reason_before_the_command() {
    let mut s = Sched::new().await;
    s.claude = format!("/{}", "c".repeat(20_000));
    s.restart().await;
    let name = unique("refused");
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [name.as_str()]);
    let text = s.failed_text(&name).await;
    assert!(
        text.starts_with("could not start: command too long (`tmux "),
        "{text}"
    );
    assert!(
        text.chars().count() <= 300 && text.ends_with("..."),
        "{text}"
    );
    assert!(s.agents().await.is_empty());
}

async fn set_floor(s: &Sched, gb: u32) {
    let put = s.store().set_setting(SettingKey::MinFreeDiskGb, json!(gb));
    put.await.unwrap();
}

#[tokio::test]
async fn a_task_waits_queued_while_the_disk_is_below_the_floor() {
    let s = Sched::new().await;
    set_floor(&s, 100_000).await;
    let name = unique("no-room");
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(
        pass.started.is_empty() && pass.failed.is_empty(),
        "{pass:?}"
    );
    assert_eq!(s.state(&name).await, TaskState::Queued);
    assert!(s.agents().await.is_empty());
    assert!(!s.worktree(&name).exists());
    let said = pass.warnings.join("; ");
    assert!(said.starts_with("disk almost full (needs "), "{said}");
    assert!(!said.contains(" GB free"), "{said}");

    let waiting = s.ctx().notes().waiting(&s.conversation);
    assert_eq!(waiting.len(), 1, "{waiting:?}");
    assert!(waiting[0].starts_with("disk almost full: "), "{waiting:?}");
    assert!(waiting[0].contains(&name), "{waiting:?}");
    run_once(s.ctx()).await.unwrap();
    let again = s.ctx().notes().waiting(&s.conversation);
    assert_eq!(
        again.len(),
        1,
        "one event for as long as it blocks: {again:?}"
    );

    set_floor(&s, 0).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Working);
}

#[tokio::test]
async fn a_later_low_disk_episode_tells_the_planner_again() {
    let s = Sched::new().await;
    set_floor(&s, 100_000).await;
    let first = unique("cancelled");
    s.create(&first, &[]).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.ctx().notes().waiting(&s.conversation).len(), 1);

    s.set_state(&first, TaskState::Ended).await;
    set_floor(&s, 0).await;
    run_once(s.ctx()).await.unwrap();

    set_floor(&s, 100_000).await;
    let second = unique("later");
    s.create(&second, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let waiting = s.ctx().notes().waiting(&s.conversation);
    assert!(waiting.iter().any(|w| w.contains(&second)), "{waiting:?}");
}

#[tokio::test]
async fn a_start_that_runs_out_of_space_says_the_disk_is_full() {
    let s = Sched::new().await;
    let name = unique("enospc");
    s.create(&name, &[]).await;
    let full = townhall_core::Error::io("write", std::io::Error::from_raw_os_error(28));
    let text = townhall_core::scheduler::start_failure(s.ctx(), &full).await;
    assert!(text.starts_with("disk almost full: "), "{text}");
    assert!(text.contains(" GB free"), "{text}");
    let other = townhall_core::Error::Conflict("a clash".into());
    assert_eq!(
        townhall_core::scheduler::start_failure(s.ctx(), &other).await,
        "a clash"
    );
}
