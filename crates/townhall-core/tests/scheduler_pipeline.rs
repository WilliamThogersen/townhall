use std::path::Path;
use std::sync::OnceLock;

use serde_json::json;
use tempfile::TempDir;
use townhall_core::mcp::create_task_as_plugin;
use townhall_core::model::{CreateTaskInput, EntryBody, SettingKey, Task, TaskState};
use townhall_core::scheduler::run_once;
use townhall_core::steps::{project_dir_for, CLAUDE_PROJECTS_ENV};
use townhall_core::store::{RUN_FINISHED, RUN_NEEDS_YOU, RUN_RUNNING};

mod scheduler_support;
use scheduler_support::Sched;

const SLEEPING: &str = "command = \"exec sleep 120\"";

const STANDARD: &str = r#"
name = "standard"
work = "build"

[[step]]
id = "plan"
agent = "planner"
prompt = "Plan this:\n\n{{input}}"

[[step]]
id = "build"
agent = "builder"
after = ["plan"]
prompt = "Build it."

[[step]]
id = "land"
agent = "lander"
after = ["build"]
prompt = "Land it."
"#;

const CHECKED: &str = r#"
name = "checked"
work = "build"

[[step]]
id = "build"
agent = "builder"
prompt = "Build it."

[[step]]
id = "test"
agent = "tester"
after = ["build"]
prompt = "Test it."

[[step]]
id = "land"
agent = "lander"
after = ["test"]
prompt = "Land it."
"#;

static PROJECTS: OnceLock<TempDir> = OnceLock::new();

fn projects() -> &'static Path {
    PROJECTS
        .get_or_init(|| {
            let dir = TempDir::new().unwrap();
            std::env::set_var(CLAUDE_PROJECTS_ENV, dir.path());
            dir
        })
        .path()
}

async fn setup() -> Sched {
    projects();
    let s = Sched::new().await;
    for kind in ["planner", "builder", "tester", "lander"] {
        let source = format!("name = \"{kind}\"\nadapter = \"shell\"\n{SLEEPING}\n");
        s.store().save_agent_type(&source).await.unwrap();
    }
    for source in [STANDARD, CHECKED] {
        s.store().save_workflow(source).await.unwrap();
    }
    s
}

async fn start(s: &Sched, run: &str, pipeline: &str) -> Task {
    let input = CreateTaskInput {
        name: run.into(),
        title: format!("run {run}"),
        prompt: "add a login page".into(),
        repo: Some(s.repo_name.clone()),
        pipeline: Some(pipeline.into()),
        ..CreateTaskInput::default()
    };
    create_task_as_plugin(s.ctx(), &s.conversation, "test", input)
        .await
        .unwrap()
}

async fn names(s: &Sched) -> Vec<String> {
    let mut all: Vec<String> = s
        .store()
        .all_tasks()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    all.sort();
    all
}

async fn task(s: &Sched, name: &str) -> Task {
    s.store().find_task(name).await.unwrap().unwrap()
}

async fn events(s: &Sched) -> Vec<String> {
    let entries = s.store().entries(&s.conversation).await.unwrap();
    entries
        .into_iter()
        .filter_map(|e| match e.body {
            EntryBody::Event { text } => Some(text),
            _ => None,
        })
        .collect()
}

fn waiting(s: &Sched) -> Vec<String> {
    s.ctx().notes().waiting(&s.conversation)
}

async fn run_state(s: &Sched, run: &str) -> String {
    s.store().run(run).await.unwrap().unwrap().state
}

#[tokio::test]
async fn plan_build_land_run_in_order() {
    let s = setup().await;
    let first = start(&s, "feat", "standard").await;
    assert_eq!(first.name, "feat-plan");
    assert_eq!(names(&s).await, ["feat-plan"]);
    assert!(first.prompt.contains("Plan this:\n\nadd a login page"));
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-plan"]);
    assert_eq!(s.state("feat-plan").await, TaskState::Working);

    s.end("feat-plan").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-build", "feat-plan"]);
    let build = task(&s, "feat-build").await;
    assert_eq!(build.after, ["feat-plan"]);
    assert_eq!(
        (build.base_branch.as_str(), build.pr_base.as_deref()),
        ("main", Some("main"))
    );
    assert_eq!(build.agent, "builder");
    assert_eq!(build.state, TaskState::Working);

    s.end("feat-build").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-build", "feat-land", "feat-plan"]);
    let land = task(&s, "feat-land").await;
    assert_eq!(land.after, ["feat-build"]);
    assert_eq!(land.base_branch, "townhall/feat-build");
    assert_eq!(land.pr_base.as_deref(), Some("main"));
    assert_eq!(run_state(&s, "feat").await, RUN_RUNNING);
    assert!(waiting(&s).is_empty());

    s.end("feat-land").await;
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "feat").await, RUN_FINISHED);
    assert_eq!(waiting(&s).len(), 1, "{:?}", waiting(&s));
    let said = events(&s).await;
    let started: Vec<&String> = said.iter().filter(|e| e.contains("started")).collect();
    assert_eq!(started.len(), 3, "{said:?}");
}

#[tokio::test]
async fn running_the_driver_twice_creates_no_duplicate() {
    let s = setup().await;
    start(&s, "feat", "standard").await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(names(&s).await, ["feat-plan"]);
    s.end("feat-plan").await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(names(&s).await, ["feat-build", "feat-plan"]);
    assert_eq!(events(&s).await.len(), 2, "{:?}", events(&s).await);
}

async fn restart(s: &mut Sched) {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    s.restart().await;
}

#[tokio::test]
async fn a_restart_mid_run_creates_no_duplicates() {
    let mut s = setup().await;
    start(&s, "feat", "standard").await;
    run_once(s.ctx()).await.unwrap();
    restart(&mut s).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-plan"]);
    s.end("feat-plan").await;
    run_once(s.ctx()).await.unwrap();
    restart(&mut s).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-build", "feat-plan"]);
}

#[tokio::test]
async fn a_claim_cut_short_before_the_task_exists_makes_exactly_one_task() {
    let s = setup().await;
    start(&s, "feat", "standard").await;
    run_once(s.ctx()).await.unwrap();
    s.end("feat-plan").await;
    assert!(s.store().claim_run_step("feat", "build", 1).await.unwrap());
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-build", "feat-plan"]);
    let steps = s.store().run_steps("feat").await.unwrap();
    let build = steps.iter().find(|r| r.step_id == "build").unwrap();
    assert_eq!(build.task.as_deref(), Some("feat-build"));
}

#[tokio::test]
async fn a_task_made_before_its_row_was_saved_is_adopted() {
    let s = setup().await;
    start(&s, "feat", "standard").await;
    run_once(s.ctx()).await.unwrap();
    s.end("feat-plan").await;
    assert!(s.store().claim_run_step("feat", "build", 1).await.unwrap());
    let new = townhall_core::model::NewTask {
        agent: "builder".into(),
        ..s.new_task("feat-build", &["feat-plan"])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["feat-build", "feat-plan"]);
    let steps = s.store().run_steps("feat").await.unwrap();
    let build = steps.iter().find(|r| r.step_id == "build").unwrap();
    assert_eq!(build.task.as_deref(), Some("feat-build"));
    assert_eq!(build.state, "started");
}

fn write_report(s: &Sched, name: &str, session: &str, text: &str) {
    let dir = project_dir_for(projects(), &s.worktree(name));
    std::fs::create_dir_all(&dir).unwrap();
    let line = json!({"type": "assistant", "timestamp": "2026-11-04T10:01:00.000Z",
        "message": {"id": "end", "role": "assistant", "stop_reason": "end_turn",
                    "content": [{"type": "text", "text": text}]}});
    std::fs::write(dir.join(format!("{session}.jsonl")), format!("{line}\n")).unwrap();
}

async fn tester_reports(s: &Sched, name: &str, text: &str) {
    let mut launch = s.store().task_launch(name).await.unwrap().unwrap();
    let session = townhall_core::ids::new_session_id();
    launch.session_id = Some(session.clone());
    s.store().record_launch(launch).await.unwrap();
    write_report(s, name, &session, text);
}

async fn run_to_a_finished_tester(s: &Sched, report: Option<&str>) {
    start(s, "chk", "checked").await;
    run_once(s.ctx()).await.unwrap();
    s.end("chk-build").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state("chk-test").await, TaskState::Working);
    if let Some(text) = report {
        tester_reports(s, "chk-test", text).await;
    }
    s.end("chk-test").await;
}

#[tokio::test]
async fn a_fail_stops_the_run_with_exactly_one_notify() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("Ran it.\n\nVERDICT: FAIL\n")).await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    assert_eq!(names(&s).await, ["chk-build", "chk-test"]);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(
        told[0].contains("chk-test") && told[0].contains("FAIL"),
        "{told:?}"
    );
}

#[tokio::test]
async fn a_pass_lets_the_run_go_on() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("Ran it.\n\nVERDICT: PASS\n")).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert_eq!(names(&s).await, ["chk-build", "chk-land", "chk-test"]);
    assert!(waiting(&s).is_empty());
}

#[tokio::test]
async fn a_verdict_step_with_no_verdict_line_needs_the_user() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("Looks fine to me.\n")).await;
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    assert_eq!(names(&s).await, ["chk-build", "chk-test"]);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("no VERDICT"), "{told:?}");
}

#[tokio::test]
async fn a_verdict_step_whose_branch_moved_needs_the_user() {
    let s = setup().await;
    start(&s, "chk", "checked").await;
    run_once(s.ctx()).await.unwrap();
    s.end("chk-build").await;
    run_once(s.ctx()).await.unwrap();
    tester_reports(&s, "chk-test", "VERDICT: PASS\n").await;
    scheduler_support::commit(&s.worktree("chk-test"), "tester committed").await;
    s.end("chk-test").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("moved"), "{told:?}");
}

#[tokio::test]
async fn reflect_skips_run_tasks() {
    let s = setup().await;
    let on = s
        .store()
        .set_setting(SettingKey::ReflectAfterTasks, json!(true));
    on.await.unwrap();
    start(&s, "feat", "standard").await;
    run_once(s.ctx()).await.unwrap();
    s.end("feat-plan").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.reflected.is_empty(), "{pass:?}");
    assert!(s.store().handoffs("feat-plan").await.unwrap().is_empty());
}

#[tokio::test]
async fn an_unknown_pipeline_is_refused_and_starts_nothing() {
    let s = setup().await;
    let input = CreateTaskInput {
        name: "feat".into(),
        title: "run".into(),
        prompt: "x".into(),
        repo: Some(s.repo_name.clone()),
        pipeline: Some("nope".into()),
        ..CreateTaskInput::default()
    };
    let err = create_task_as_plugin(s.ctx(), &s.conversation, "test", input)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");
    assert!(names(&s).await.is_empty());
    assert!(s.store().run("feat").await.unwrap().is_none());
}
