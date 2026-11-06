//! The agent watcher against real tmux agents: task states, thread entries and
//! `agentUpdated` events, once each, also after a restart. Transcripts come from a temp
//! projects folder (`TOWNHALL_CLAUDE_PROJECTS`), never `~/.claude`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;
use townhall_core::agent::watch::Watcher;
use townhall_core::agent::{self, state, LaunchOptions, ShellAdapter};
use townhall_core::model::{
    AgentEvent, AgentInfo, AgentState, EntryBody, NewTask, ServerEvent, Task, TaskState,
};
use townhall_core::{clock, steps, tmux, Context, ContextOptions, Home, Workspace};

mod common;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
static PROJECTS: OnceLock<PathBuf> = OnceLock::new();

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

/// Every test in this binary reads transcripts from one temp folder.
fn projects() -> &'static Path {
    PROJECTS.get_or_init(|| {
        let dir = tempfile::Builder::new()
            .prefix("townhall-watch-projects")
            .tempdir()
            .unwrap()
            .keep();
        std::env::set_var(state::PROJECTS_ENV, &dir);
        dir
    })
}

/// Kills the sessions it names when dropped.
struct Sessions(Vec<String>);

impl Drop for Sessions {
    fn drop(&mut self) {
        for session in &self.0 {
            let target = format!("={session}");
            let _ = common::tmux()
                .args(["kill-session", "-t", &target])
                .output();
        }
    }
}

struct Env {
    repo: TempDir,
    home: TempDir,
    ctx: Option<Context>,
    repo_name: String,
    conversation: String,
    sessions: Sessions,
}

fn open(repo: &Path, home: &Path) -> Context {
    let ws = Workspace::new(Home::at(home.join(".townhall")), repo).unwrap();
    let options = ContextOptions {
        notify_delay: Duration::from_secs(3600),
        ..ContextOptions::default()
    };
    Context::open(ws, options).unwrap()
}

async fn env() -> Env {
    projects();
    let repo = common::init_repo().await;
    let home = TempDir::new().unwrap();
    let ctx = open(repo.path(), home.path());
    let repo_name = ctx.ensure_startup_repo().await.unwrap();
    let conversation = ctx.store().unwrap().create_conversation().await.unwrap().id;
    Env {
        repo,
        home,
        ctx: Some(ctx),
        repo_name,
        conversation,
        sessions: Sessions(vec![]),
    }
}

impl Env {
    fn ctx(&self) -> &Context {
        self.ctx.as_ref().unwrap()
    }

    /// Closes the store and opens it again, as a daemon restart does.
    fn restart(&mut self) {
        drop(self.ctx.take());
        self.ctx = Some(open(self.repo.path(), self.home.path()));
    }

    async fn task(&self, name: &str, state: TaskState) -> Task {
        let store = self.ctx().store().unwrap();
        let new = NewTask {
            name: name.into(),
            title: format!("do {name}"),
            prompt: "the brief".into(),
            repo: self.repo_name.clone(),
            base_branch: "main".into(),
            after: vec![],
            attachments: vec![],
            agent: "builder".into(),
            pr_base: None,
        };
        store.create_task(&self.conversation, new).await.unwrap();
        self.set_state(name, state).await
    }

    async fn set_state(&self, name: &str, state: TaskState) -> Task {
        let store = self.ctx().store().unwrap();
        store
            .update_task(name, move |t| Ok(Task { state, ..t }))
            .await
            .unwrap()
    }

    async fn agent(&mut self, command: &str) -> String {
        let name = unique("w");
        let ws = self.ctx().startup().clone();
        self.sessions.0.push(tmux::session_name(ws.key(), &name));
        let options = LaunchOptions {
            command: Some(command.to_string()),
            ..LaunchOptions::default()
        };
        agent::start(&ws, &name, "main", &ShellAdapter, &options)
            .await
            .unwrap();
        name
    }

    async fn state_of(&self, name: &str) -> TaskState {
        let task = self.ctx().store().unwrap().find_task(name).await.unwrap();
        task.unwrap().state
    }

    /// The `agent` and `needsYou` entries of the conversation.
    async fn agent_entries(&self) -> Vec<EntryBody> {
        let entries = self.ctx().store().unwrap().entries(&self.conversation);
        entries
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.body)
            .filter(|b| matches!(b, EntryBody::Agent { .. } | EntryBody::NeedsYou { .. }))
            .collect()
    }

    /// Polls until the task is in `want`, failing after a few seconds.
    async fn poll_until(&self, watcher: &mut Watcher, name: &str, want: TaskState) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            watcher.poll().await;
            let now = self.state_of(name).await;
            if now == want {
                return;
            }
            assert!(Instant::now() < deadline, "task `{name}` stayed {now:?}");
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    async fn polls(&self, watcher: &mut Watcher, count: usize) {
        for _ in 0..count {
            watcher.poll().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// The `agentUpdated` events for `name` received so far.
fn updates(rx: &mut Receiver<ServerEvent>, name: &str) -> Vec<AgentInfo> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|e| match e {
            ServerEvent::AgentUpdated { agent } if agent.name == name => Some(agent),
            _ => None,
        })
        .collect()
}

fn failed(agent: &str, text: &str) -> EntryBody {
    EntryBody::Agent {
        agent: agent.into(),
        event: AgentEvent::Failed,
        text: text.into(),
    }
}

#[tokio::test]
async fn exit_three_fails_its_task_once_even_after_a_restart() {
    let mut env = env().await;
    let name = env.agent("sh -c 'sleep 0.3; exit 3'").await;
    env.task(&name, TaskState::Working).await;
    let mut rx = env.ctx().bus().subscribe();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::Error).await;
    env.polls(&mut watcher, 3).await;
    let once = vec![failed(&name, "exited with code 3")];
    assert_eq!(env.agent_entries().await, once);
    assert!(env.ctx().notes().is_waiting(&env.conversation));
    let seen = updates(&mut rx, &name);
    let last = seen.last().expect("agentUpdated was published");
    assert_eq!(last.state, AgentState::Error { exit_code: Some(3) });

    drop(watcher);
    env.restart();
    let mut rx = env.ctx().bus().subscribe();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.polls(&mut watcher, 3).await;
    assert_eq!(env.agent_entries().await, once);
    assert_eq!(env.state_of(&name).await, TaskState::Error);
    assert!(
        updates(&mut rx, &name).is_empty(),
        "no repeat after a restart"
    );
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn exit_zero_ends_its_task_once() {
    let mut env = env().await;
    let name = env.agent("true").await;
    env.task(&name, TaskState::Working).await;
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::Ended).await;
    env.polls(&mut watcher, 2).await;
    let ended = EntryBody::Agent {
        agent: name.clone(),
        event: AgentEvent::Ended,
        text: "ended".into(),
    };
    assert_eq!(env.agent_entries().await, vec![ended]);
    assert!(
        env.ctx().notes().is_waiting(&env.conversation),
        "the planner is told it ended"
    );
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_question_needs_you_once_then_working_again() {
    let mut env = env().await;
    let name = env
        .agent("sh -c 'printf \"continue? [y/n] \"; read a; echo got-$a; sleep 30'")
        .await;
    env.task(&name, TaskState::Working).await;
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::NeedsYou)
        .await;
    env.polls(&mut watcher, 2).await;
    drop(watcher);
    env.restart();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.polls(&mut watcher, 2).await;
    let asked = EntryBody::NeedsYou {
        agent: name.clone(),
        question: "continue? [y/n]".into(),
        choices: vec!["y".into(), "n".into()],
    };
    assert_eq!(env.agent_entries().await, vec![asked.clone()]);

    agent::send(env.ctx().startup(), &name, "y").await.unwrap();
    env.poll_until(&mut watcher, &name, TaskState::Working)
        .await;
    assert_eq!(env.agent_entries().await, vec![asked]);
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn an_answered_y_n_prompt_works_again_with_one_entry() {
    let mut env = env().await;
    let name = env
        .agent("sh -c 'printf \"go on? (y/n) \"; read x; sleep 600'")
        .await;
    env.task(&name, TaskState::Working).await;
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::NeedsYou)
        .await;
    assert!(env.ctx().notes().is_waiting(&env.conversation));
    agent::send(env.ctx().startup(), &name, "y").await.unwrap();
    env.poll_until(&mut watcher, &name, TaskState::Working)
        .await;
    env.polls(&mut watcher, 3).await;
    assert_eq!(env.state_of(&name).await, TaskState::Working);
    let asked = EntryBody::NeedsYou {
        agent: name.clone(),
        question: "go on? (y/n)".into(),
        choices: vec!["y".into(), "n".into()],
    };
    assert_eq!(env.agent_entries().await, vec![asked]);
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn queued_and_merged_tasks_are_left_alone() {
    let mut env = env().await;
    let asking = env.agent("sh -c 'printf \"go? \"; read a; sleep 30'").await;
    let failing = env.agent("sh -c 'exit 3'").await;
    env.task(&asking, TaskState::Queued).await;
    env.task(&failing, TaskState::Merged).await;
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.polls(&mut watcher, 6).await;
    assert_eq!(env.state_of(&asking).await, TaskState::Queued);
    assert_eq!(env.state_of(&failing).await, TaskState::Merged);
    assert!(env.agent_entries().await.is_empty());
    assert!(!env.ctx().notes().is_waiting(&env.conversation));

    env.set_state(&asking, TaskState::Working).await;
    env.poll_until(&mut watcher, &asking, TaskState::NeedsYou)
        .await;
    assert_eq!(env.agent_entries().await.len(), 1);
    assert!(env.ctx().notes().is_waiting(&env.conversation));
    for name in [&asking, &failing] {
        agent::stop(env.ctx().startup(), name).await.unwrap();
    }
}

#[tokio::test]
async fn an_agent_without_a_task_is_only_published_when_it_changes() {
    let mut env = env().await;
    let name = env.agent("sleep 30").await;
    let mut rx = env.ctx().bus().subscribe();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.polls(&mut watcher, 3).await;
    let seen = updates(&mut rx, &name);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(
        (seen[0].state.clone(), seen[0].repo.as_str()),
        (AgentState::Working, env.repo_name.as_str())
    );
    tmux::kill(env.ctx().startup().key(), &name).await.unwrap();
    env.polls(&mut watcher, 3).await;
    let ended = updates(&mut rx, &name);
    assert_eq!(ended.len(), 1, "{ended:?}");
    assert_eq!(ended[0].state, AgentState::Ended);
    assert!(env.agent_entries().await.is_empty());
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_removed_agent_ends_its_working_task() {
    let mut env = env().await;
    let name = env.agent("sleep 30").await;
    env.task(&name, TaskState::Working).await;
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.polls(&mut watcher, 2).await;
    agent::stop(env.ctx().startup(), &name).await.unwrap();
    env.poll_until(&mut watcher, &name, TaskState::Ended).await;
    let ended = EntryBody::Agent {
        agent: name.clone(),
        event: AgentEvent::Ended,
        text: "the agent was removed".into(),
    };
    assert_eq!(env.agent_entries().await, vec![ended]);
}

#[tokio::test]
async fn a_transcript_question_needs_you_with_numbered_choices() {
    let mut env = env().await;
    let name = env.agent("sleep 30").await;
    env.task(&name, TaskState::Working).await;
    let listed = agent::list(env.ctx().startup()).await.unwrap();
    let worktree = &listed.iter().find(|a| a.name == name).unwrap().worktree;
    let dir = steps::project_dir_for(projects(), worktree);
    std::fs::create_dir_all(&dir).unwrap();
    let input = json!({"questions": [{"question": "Which db?", "options": [{"label": "postgres"}, {"label": "sqlite"}]}]});
    let line = json!({"type": "assistant", "message": {"id": "m1", "stop_reason": "tool_use",
        "content": [{"type": "tool_use", "id": "t1", "name": "AskUserQuestion", "input": input}]}});
    std::fs::write(dir.join("s.jsonl"), format!("{line}\n")).unwrap();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::NeedsYou)
        .await;
    let asked = EntryBody::NeedsYou {
        agent: name.clone(),
        question: "Which db?\n1. postgres\n2. sqlite".into(),
        choices: vec!["1".into(), "2".into()],
    };
    assert_eq!(env.agent_entries().await, vec![asked]);
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_finished_idle_claude_ends_its_task_and_a_new_prompt_resumes_it() {
    let mut env = env().await;
    let name = env.agent("sleep 30").await;
    env.task(&name, TaskState::Working).await;
    let listed = agent::list(env.ctx().startup()).await.unwrap();
    let worktree = &listed.iter().find(|a| a.name == name).unwrap().worktree;
    let dir = steps::project_dir_for(projects(), worktree);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.jsonl");
    let at = clock::from_unix_millis(clock::unix_millis(SystemTime::now()) - 60_000);
    let done = json!({"type": "assistant", "timestamp": at, "message": {"id": "m1",
        "stop_reason": "end_turn", "content": [{"type": "text", "text": "Done, tests pass."}]}});
    std::fs::write(&path, format!("{done}\n")).unwrap();
    let mut watcher = Watcher::new(env.ctx().clone()).await;
    env.poll_until(&mut watcher, &name, TaskState::Ended).await;
    let ended = EntryBody::Agent {
        agent: name.clone(),
        event: AgentEvent::Ended,
        text: "ended".into(),
    };
    assert_eq!(env.agent_entries().await, vec![ended.clone()]);

    let prompt = json!({"type": "user", "message": {"content": "also update the docs"}});
    std::fs::write(&path, format!("{done}\n{prompt}\n")).unwrap();
    env.poll_until(&mut watcher, &name, TaskState::Working)
        .await;
    assert_eq!(env.agent_entries().await, vec![ended]);
    agent::stop(env.ctx().startup(), &name).await.unwrap();
}
