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

const UNFIXED: &str = r#"
name = "unfixed"
work = "build"
max_fix_rounds = 0

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

const GUARDED: &str = r#"
name = "guarded"
work = "build"

[[step]]
id = "build"
agent = "builder"
prompt = "Build it."

[[step]]
id = "review"
agent = "reviewer"
after = ["build"]
prompt = "Review it."
when = { paths = ["**/migrations/**"] }

[[step]]
id = "land"
agent = "lander"
after = ["review"]
prompt = "Land it."
"#;

fn asking(name: &str, when: &str, steps: usize) -> String {
    let mut source = format!(
        "name = \"{name}\"\nwork = \"build\"\n\n[[step]]\nid = \"build\"\nagent = \"builder\"\nprompt = \"Build it.\"\n"
    );
    for n in 1..=steps {
        source.push_str(&format!(
            "\n[[step]]\nid = \"s{n}\"\nagent = \"planner\"\nafter = [\"build\"]\nprompt = \"Look.\"\nwhen = {when}\n"
        ));
    }
    source
}

const ADVISER_STUB: &str = r#"#!/bin/sh
if [ -t 0 ]; then exec sleep 120; fi
dir='@DIR@'
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin-$n"
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *ANSWER_YES*) said='YES, it touches auth' ;;
  *ANSWER_NO*) said='NO. docs only' ;;
  *) said='hard to say, honestly' ;;
esac
echo '{"type":"result","subtype":"success","is_error":false,"result":"'"$said"'","usage":{"input_tokens":10,"output_tokens":3}}'
cat >> "$dir/stdin-$n"
"#;

fn adviser_dir(s: &Sched) -> std::path::PathBuf {
    s.root.path().join("adviser-calls")
}

fn install_adviser_stub(s: &Sched) {
    use std::os::unix::fs::PermissionsExt;
    let dir = adviser_dir(s);
    std::fs::create_dir_all(&dir).unwrap();
    let script = ADVISER_STUB.replace("@DIR@", &dir.to_string_lossy());
    std::fs::write(&s.claude, script).unwrap();
    std::fs::set_permissions(&s.claude, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn adviser_calls(s: &Sched) -> usize {
    std::fs::read_to_string(adviser_dir(s).join("count"))
        .map(|text| text.trim().parse().unwrap())
        .unwrap_or(0)
}

fn adviser_was_told(s: &Sched, n: usize) -> String {
    std::fs::read_to_string(adviser_dir(s).join(format!("stdin-{n}"))).unwrap()
}

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
    for kind in ["planner", "builder", "tester", "reviewer", "lander"] {
        let source = format!("name = \"{kind}\"\nadapter = \"shell\"\n{SLEEPING}\n");
        s.store().save_agent_type(&source).await.unwrap();
    }
    for source in [STANDARD, CHECKED, UNFIXED, GUARDED] {
        s.store().save_workflow(source).await.unwrap();
    }
    install_adviser_stub(&s);
    s
}

async fn save(s: &Sched, source: &str) {
    s.store().save_workflow(source).await.unwrap();
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

    s.set_state("feat-land", TaskState::Merged).await;
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
    run_pipeline_to_a_finished_tester(s, "checked", report).await;
}

async fn run_pipeline_to_a_finished_tester(s: &Sched, pipeline: &str, report: Option<&str>) {
    start(s, "chk", pipeline).await;
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
    let fail = Some("Ran it.\n\nVERDICT: FAIL\n");
    run_pipeline_to_a_finished_tester(&s, "unfixed", fail).await;
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
async fn a_verdict_with_trailing_text_clears_the_step() {
    let s = setup().await;
    let said = "Ran it.\n\nVERDICT: PASS for all three criteria, with one note\n";
    run_to_a_finished_tester(&s, Some(said)).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert_eq!(names(&s).await, ["chk-build", "chk-land", "chk-test"]);
    assert!(waiting(&s).is_empty());
}

async fn finish_again(s: &Sched, name: &str, report: &str) {
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(name).await, TaskState::Working);
    tester_reports(s, name, report).await;
    s.end(name).await;
}

#[tokio::test]
async fn a_report_without_a_verdict_is_asked_for_one_in_the_same_task() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("Looks fine to me.\n")).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert!(waiting(&s).is_empty(), "{:?}", waiting(&s));
    let handed = s.store().handoffs("chk-test").await.unwrap();
    assert_eq!(handed.len(), 1);
    assert_eq!(handed[0].to, "tester");
    assert!(
        handed[0]
            .reason
            .contains("end with exactly one line: VERDICT: PASS or VERDICT: FAIL"),
        "{:?}",
        handed[0]
    );
    assert!(
        handed[0].reason.contains("Looks fine to me."),
        "{:?}",
        handed[0]
    );
    assert_eq!(names(&s).await, ["chk-build", "chk-test"]);

    finish_again(&s, "chk-test", "VERDICT: PASS\n").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert_eq!(names(&s).await, ["chk-build", "chk-land", "chk-test"]);
    assert_eq!(s.store().handoffs("chk-test").await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_verdict_step_still_without_a_verdict_line_needs_the_user() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("Looks fine to me.\n")).await;
    run_once(s.ctx()).await.unwrap();
    finish_again(&s, "chk-test", "Still fine.\n").await;
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    assert_eq!(names(&s).await, ["chk-build", "chk-test"]);
    assert_eq!(s.store().handoffs("chk-test").await.unwrap().len(), 1);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("no VERDICT"), "{told:?}");
}

const LOGIN_ERROR: &str = "Please run /login · API Error: 403 {\"error\":\"forbidden\"}";

async fn let_the_retry_delay_pass(s: &Sched, step: &str) {
    let row = step_row(s, "chk", step).await;
    let note = row.note.unwrap();
    assert!(note.starts_with("retry at "), "{note}");
    let update = townhall_core::store::StepUpdate {
        note: Some("retry at 0: waited"),
        ..townhall_core::store::StepUpdate::state("started")
    };
    s.store()
        .set_run_step("chk", step, row.round, update)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_api_error_is_retried_once_after_a_delay_and_not_called_a_missing_verdict() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some(LOGIN_ERROR)).await;
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert!(s.store().handoffs("chk-test").await.unwrap().is_empty());
    assert!(waiting(&s).is_empty(), "{:?}", waiting(&s));

    let_the_retry_delay_pass(&s, "test").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.store().handoffs("chk-test").await.unwrap().len(), 1);
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);

    finish_again(&s, "chk-test", "VERDICT: PASS\n").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["chk-build", "chk-land", "chk-test"]);
    assert!(waiting(&s).is_empty(), "{:?}", waiting(&s));
}

#[tokio::test]
async fn an_api_error_that_comes_back_needs_the_user_with_the_real_error() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some(LOGIN_ERROR)).await;
    run_once(s.ctx()).await.unwrap();
    let_the_retry_delay_pass(&s, "test").await;
    run_once(s.ctx()).await.unwrap();
    finish_again(&s, "chk-test", LOGIN_ERROR).await;
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    assert_eq!(s.store().handoffs("chk-test").await.unwrap().len(), 1);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(
        told[0].contains("claude login expired: run /login"),
        "{told:?}"
    );
    assert!(told[0].contains("API Error: 403"), "{told:?}");
    assert!(!told[0].contains("no VERDICT"), "{told:?}");
}

#[tokio::test]
async fn a_tester_that_never_ran_is_retried_like_an_api_error() {
    let s = setup().await;
    run_to_a_finished_tester(&s, None).await;
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    let_the_retry_delay_pass(&s, "test").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.store().handoffs("chk-test").await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_land_step_that_ended_without_a_merge_does_not_finish_the_run() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("VERDICT: PASS\n")).await;
    run_once(s.ctx()).await.unwrap();
    s.end("chk-land").await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_ne!(run_state(&s, "chk").await, RUN_FINISHED);
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
}

#[tokio::test]
async fn a_land_step_that_hit_an_api_error_needs_the_user_after_one_retry() {
    let s = setup().await;
    run_to_a_finished_tester(&s, Some("VERDICT: PASS\n")).await;
    run_once(s.ctx()).await.unwrap();
    tester_reports(&s, "chk-land", LOGIN_ERROR).await;
    s.end("chk-land").await;
    run_once(s.ctx()).await.unwrap();
    let_the_retry_delay_pass(&s, "land").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.store().handoffs("chk-land").await.unwrap().len(), 1);
    finish_again(&s, "chk-land", LOGIN_ERROR).await;
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("claude login expired"), "{told:?}");
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

async fn build_touching(s: &Sched, run: &str, file: &str) {
    build_touching_in(s, run, "guarded", file).await;
}

async fn build_touching_in(s: &Sched, run: &str, pipeline: &str, file: &str) {
    start(s, run, pipeline).await;
    run_once(s.ctx()).await.unwrap();
    let build = format!("{run}-build");
    let path = s.worktree(&build).join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "x\n").unwrap();
    scheduler_support::git(&s.worktree(&build), &["add", "-A"]).await;
    scheduler_support::commit(&s.worktree(&build), "change").await;
    s.end(&build).await;
    run_once(s.ctx()).await.unwrap();
}

#[tokio::test]
async fn a_paths_step_is_skipped_when_no_changed_file_matches() {
    let s = setup().await;
    build_touching(&s, "doc", "README.md").await;
    assert_eq!(names(&s).await, ["doc-build", "doc-land"]);
    let steps = s.store().run_steps("doc").await.unwrap();
    let review = steps.iter().find(|r| r.step_id == "review").unwrap();
    assert_eq!(review.state, "skipped");
    assert!(review.task.is_none());
    let why = review.note.clone().unwrap_or_default();
    assert!(why.contains("**/migrations/**"), "{why}");
    let said = events(&s).await;
    let skipped: Vec<&String> = said.iter().filter(|e| e.contains("skipped")).collect();
    assert_eq!(skipped.len(), 1, "{said:?}");
    assert!(skipped[0].contains("review"), "{said:?}");
    assert_eq!(task(&s, "doc-land").await.after, ["doc-build"]);
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(events(&s).await.len(), said.len());
    assert_eq!(run_state(&s, "doc").await, RUN_RUNNING);
}

#[tokio::test]
async fn a_paths_step_runs_when_a_changed_file_matches() {
    let s = setup().await;
    build_touching(&s, "db", "db/migrations/1.sql").await;
    assert_eq!(names(&s).await, ["db-build", "db-review"]);
    let steps = s.store().run_steps("db").await.unwrap();
    let review = steps.iter().find(|r| r.step_id == "review").unwrap();
    assert_eq!(review.state, "started");
    assert_eq!(task(&s, "db-review").await.base_branch, "townhall/db-build");
}

async fn first_fail(s: &Sched) {
    run_to_a_finished_tester(s, Some("Ran it.\n\nVERDICT: FAIL\n")).await;
    run_once(s.ctx()).await.unwrap();
}

#[tokio::test]
async fn a_fail_hands_the_build_back_and_a_pass_after_the_fix_lands() {
    let s = setup().await;
    first_fail(&s).await;
    assert_eq!(run_state(&s, "chk").await, RUN_RUNNING);
    assert!(waiting(&s).is_empty(), "{:?}", waiting(&s));
    let handed = s.store().handoffs("chk-build").await.unwrap();
    assert_eq!(handed.len(), 1);
    assert_eq!(handed[0].to, "builder");
    assert!(
        handed[0].reason.contains("VERDICT: FAIL"),
        "{:?}",
        handed[0]
    );
    assert_eq!(names(&s).await, ["chk-build", "chk-test"]);

    s.end("chk-build").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(names(&s).await, ["chk-build", "chk-test", "chk-test-r2"]);
    let again = task(&s, "chk-test-r2").await;
    assert_eq!(again.after, ["chk-build"]);
    assert_eq!(again.base_branch, "townhall/chk-build");
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(names(&s).await, ["chk-build", "chk-test", "chk-test-r2"]);

    tester_reports(&s, "chk-test-r2", "VERDICT: PASS\n").await;
    s.end("chk-test-r2").await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(task(&s, "chk-land").await.after, ["chk-test-r2"]);
    s.set_state("chk-land", TaskState::Merged).await;
    run_once(s.ctx()).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(run_state(&s, "chk").await, RUN_FINISHED);
    assert_eq!(waiting(&s).len(), 1, "{:?}", waiting(&s));
}

#[tokio::test]
async fn a_second_fail_needs_the_user_with_exactly_one_event() {
    let s = setup().await;
    first_fail(&s).await;
    s.end("chk-build").await;
    run_once(s.ctx()).await.unwrap();
    tester_reports(&s, "chk-test-r2", "VERDICT: FAIL\n").await;
    s.end("chk-test-r2").await;
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(run_state(&s, "chk").await, RUN_NEEDS_YOU);
    assert_eq!(s.store().handoffs("chk-build").await.unwrap().len(), 1);
    assert_eq!(names(&s).await, ["chk-build", "chk-test", "chk-test-r2"]);
    let told = waiting(&s);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("chk-test-r2"), "{told:?}");
}

async fn step_row(s: &Sched, run: &str, step: &str) -> townhall_core::store::RunStep {
    let steps = s.store().run_steps(run).await.unwrap();
    steps.into_iter().find(|r| r.step_id == step).unwrap()
}

async fn ask_run(s: &Sched, question: &str, file: &str) {
    let when = format!("{{ ask = \"{question}\" }}");
    save(s, &asking("asked", &when, 1)).await;
    build_touching_in(s, "ask", "asked", file).await;
}

#[tokio::test]
async fn a_yes_from_the_adviser_runs_the_step() {
    let s = setup().await;
    ask_run(&s, "ANSWER_YES: does it touch auth?", "src/auth/login.rs").await;
    assert_eq!(names(&s).await, ["ask-build", "ask-s1"]);
    assert_eq!(adviser_calls(&s), 1);
    let told = adviser_was_told(&s, 1);
    assert!(told.contains("does it touch auth?"), "{told}");
    assert!(told.contains("src/auth/login.rs"), "{told}");
    let row = step_row(&s, "ask", "s1").await;
    assert_eq!(row.state, "started");
    let note = row.note.unwrap_or_default();
    assert!(note.starts_with("adviser answered YES"), "{note}");
    assert!(note.contains("it touches auth"), "{note}");
    let said = events(&s).await;
    let started: Vec<&String> = said.iter().filter(|e| e.contains("`s1` started")).collect();
    assert!(started[0].contains("YES"), "{said:?}");
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(adviser_calls(&s), 1);
}

#[tokio::test]
async fn a_no_from_the_adviser_skips_the_step() {
    let s = setup().await;
    ask_run(&s, "ANSWER_NO: does it touch auth?", "README.md").await;
    assert_eq!(names(&s).await, ["ask-build"]);
    assert_eq!(adviser_calls(&s), 1);
    let row = step_row(&s, "ask", "s1").await;
    assert_eq!(row.state, "skipped");
    let note = row.note.unwrap_or_default();
    assert!(note.starts_with("adviser answered NO"), "{note}");
    let said = events(&s).await;
    let skipped: Vec<&String> = said.iter().filter(|e| e.contains("skipped")).collect();
    assert_eq!(skipped.len(), 1, "{said:?}");
    assert!(skipped[0].contains("NO"), "{said:?}");
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(adviser_calls(&s), 1);
}

#[tokio::test]
async fn an_answer_that_is_neither_yes_nor_no_counts_as_no() {
    let s = setup().await;
    ask_run(&s, "does it touch auth?", "src/auth/login.rs").await;
    assert_eq!(names(&s).await, ["ask-build"]);
    let row = step_row(&s, "ask", "s1").await;
    assert_eq!(row.state, "skipped");
    let note = row.note.unwrap_or_default();
    assert!(note.starts_with("adviser answered NO"), "{note}");
    assert!(note.contains("hard to say"), "{note}");
}

#[tokio::test]
async fn the_fourth_question_of_a_run_is_skipped_without_a_call() {
    let s = setup().await;
    save(&s, &asking("many", "{ ask = \"ANSWER_YES: ok?\" }", 4)).await;
    build_touching_in(&s, "many", "many", "src/a.rs").await;
    assert_eq!(adviser_calls(&s), 3);
    assert_eq!(
        names(&s).await,
        ["many-build", "many-s1", "many-s2", "many-s3"]
    );
    let row = step_row(&s, "many", "s4").await;
    assert_eq!(row.state, "skipped");
    assert_eq!(row.note.as_deref(), Some("adviser budget used"));
    for _ in 0..2 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(adviser_calls(&s), 3);
}

#[tokio::test]
async fn max_adviser_calls_sets_the_budget() {
    let s = setup().await;
    let source = asking("tight", "{ ask = \"ANSWER_YES: ok?\" }", 2)
        .replace("work =", "max_adviser_calls = 1\nwork =");
    save(&s, &source).await;
    build_touching_in(&s, "tight", "tight", "src/a.rs").await;
    assert_eq!(adviser_calls(&s), 1);
    assert_eq!(names(&s).await, ["tight-build", "tight-s1"]);
    let row = step_row(&s, "tight", "s2").await;
    assert_eq!(row.note.as_deref(), Some("adviser budget used"));
}

#[tokio::test]
async fn a_pipeline_without_ask_never_calls_the_adviser() {
    let s = setup().await;
    build_touching(&s, "db", "db/migrations/1.sql").await;
    build_touching(&s, "doc", "README.md").await;
    assert_eq!(adviser_calls(&s), 0);
}

#[tokio::test]
async fn a_step_asking_without_an_adviser_type_is_skipped_with_a_note() {
    let s = setup().await;
    s.store().delete_agent_type("adviser").await.unwrap();
    ask_run(&s, "ANSWER_YES: ok?", "src/a.rs").await;
    assert_eq!(adviser_calls(&s), 0);
    assert_eq!(names(&s).await, ["ask-build"]);
    let row = step_row(&s, "ask", "s1").await;
    assert_eq!(row.state, "skipped");
    let note = row.note.unwrap_or_default();
    assert!(note.contains("no adviser type"), "{note}");
}

#[tokio::test]
async fn ask_and_paths_must_both_hold_and_paths_is_checked_first() {
    let s = setup().await;
    let when = "{ paths = [\"**/migrations/**\"], ask = \"ANSWER_YES: ok?\" }";
    save(&s, &asking("both", when, 1)).await;
    build_touching_in(&s, "miss", "both", "README.md").await;
    assert_eq!(adviser_calls(&s), 0);
    let row = step_row(&s, "miss", "s1").await;
    assert_eq!(row.state, "skipped");
    assert!(row.note.unwrap_or_default().contains("**/migrations/**"));
    build_touching_in(&s, "hit", "both", "db/migrations/1.sql").await;
    assert_eq!(adviser_calls(&s), 1);
    assert_eq!(step_row(&s, "hit", "s1").await.state, "started");
}
