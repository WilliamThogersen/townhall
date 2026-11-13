use townhall_core::model::{NewTask, TaskState};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{unique, Sched};

const LONE: &str = "name = \"lone\"\nadapter = \"claude\"\ncan_hand_back_to = []\n";
const PICKY: &str = "name = \"picky\"\nadapter = \"claude\"\ncan_hand_back_to = [\"plain\"]\nprompt = \"be picky\"\n";
const SHELL: &str = "name = \"runner\"\nadapter = \"shell\"\ncommand = \"sleep 120\"\n";

async fn create_as(s: &Sched, name: &str, agent: &str) {
    let new = NewTask {
        agent: agent.into(),
        ..s.new_task(name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

fn forget_args(s: &Sched, name: &str) {
    std::fs::remove_file(s.args_dir.join(format!("{name}.args"))).unwrap();
}

#[tokio::test]
async fn a_claude_task_gets_hand_back_and_is_told_where_it_can_hand_back() {
    let s = Sched::new().await;
    s.store().save_agent_type(PICKY).await.unwrap();
    let name = unique("picky");
    create_as(&s, &name, "picky").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    let config = format!(
        "--mcp-config={}",
        townhall_core::mcp::task_config_json(s.ctx().base_url(), &name)
    );
    assert!(args.lines().any(|l| l == config), "{args}");
    assert!(args.contains("--strict-mcp-config"), "{args}");
    assert!(
        args.contains("--allowedTools=mcp__townhall__hand_back"),
        "{args}"
    );
    assert!(
        args.contains("be picky\n\nyou can hand back to: plain"),
        "{args}"
    );
}

#[tokio::test]
async fn a_type_that_cannot_hand_back_gets_no_mcp_server() {
    let s = Sched::new().await;
    s.store().save_agent_type(LONE).await.unwrap();
    let name = unique("lone");
    create_as(&s, &name, "lone").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    assert!(
        !args.contains("--mcp-config") && !args.contains("hand back"),
        "{args}"
    );
}

#[tokio::test]
async fn every_claude_type_allows_hand_back_without_a_prompt() {
    let s = Sched::new().await;
    s.store().save_agent_type(LONE).await.unwrap();
    s.store().save_agent_type(PICKY).await.unwrap();
    for agent in ["lone", "picky"] {
        let name = unique(agent);
        create_as(&s, &name, agent).await;
        run_once(s.ctx()).await.unwrap();
        let args = s.args(&name).await;
        assert!(args.contains("--allowedTools"), "{agent}: {args}");
        assert!(args.contains("mcp__townhall__hand_back"), "{agent}: {args}");
    }
}

#[tokio::test]
async fn hand_back_and_the_standard_commands_are_pre_allowed_and_the_mode_is_not_loosened() {
    let s = Sched::new().await;
    s.store().save_agent_type(PICKY).await.unwrap();
    let name = unique("picky");
    create_as(&s, &name, "picky").await;
    run_once(s.ctx()).await.unwrap();
    let args = s.args(&name).await;
    let allowed = args
        .lines()
        .find_map(|l| l.strip_prefix("--allowedTools="))
        .unwrap_or_else(|| panic!("{args}"));
    let rules: Vec<&str> = allowed.split(',').collect();
    for rule in [
        "mcp__townhall__hand_back",
        "Bash(git *)",
        "Bash(gh *)",
        "Bash(cargo *)",
        "Bash(npm *)",
    ] {
        assert!(rules.contains(&rule), "{rule}: {allowed}");
    }
    assert!(!args.contains("--dangerously-skip-permissions"), "{args}");
    assert!(
        !args.contains("--permission-mode=bypassPermissions"),
        "{args}"
    );
}

#[tokio::test]
async fn a_shell_type_gets_no_mcp_server() {
    let s = Sched::new().await;
    s.store().save_agent_type(SHELL).await.unwrap();
    let name = unique("runner");
    create_as(&s, &name, "runner").await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, [name]);
}

#[tokio::test]
async fn a_handed_back_task_restarts_as_the_target_type_in_the_same_worktree() {
    let s = Sched::new().await;
    s.store().save_agent_type(PICKY).await.unwrap();
    let name = unique("loop");
    create_as(&s, &name, "picky").await;
    run_once(s.ctx()).await.unwrap();
    s.args(&name).await;
    forget_args(&s, &name);
    s.store()
        .hand_back(&name, "plain", "the lint check fails", "opened the pr")
        .await
        .unwrap();
    assert_eq!(s.state(&name).await, TaskState::Queued);
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, std::slice::from_ref(&name), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Working);
    let args = s.args(&name).await;
    assert!(!args.contains("be picky"), "{args}");
    assert_eq!(s.agent_names().await, std::slice::from_ref(&name));
    assert!(s.agents().await[0].alive);
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert!(brief.contains(&format!("the brief for {name}")), "{brief}");
    assert!(brief.contains("handed back by picky to plain"), "{brief}");
    assert!(brief.contains("the lint check fails"), "{brief}");
    assert!(brief.contains("opened the pr"), "{brief}");
    let again = run_once(s.ctx()).await.unwrap();
    assert!(
        again.started.is_empty() && again.failed.is_empty(),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_handed_back_task_keeps_the_commit_its_merge_is_told_from() {
    let s = Sched::new().await;
    let name = unique("keep");
    create_as(&s, &name, "builder").await;
    run_once(s.ctx()).await.unwrap();
    let before = s.store().task_launch(&name).await.unwrap().unwrap();
    s.args(&name).await;
    s.store().hand_back(&name, "plain", "r", "").await.unwrap();
    run_once(s.ctx()).await.unwrap();
    let after = s.store().task_launch(&name).await.unwrap().unwrap();
    assert_eq!(after.start_commit, before.start_commit);
    assert_ne!(after.session_id, before.session_id);
}
