//! The watcher answers claude's folder trust dialog for an agent in townhall's own
//! worktrees with `up`, then `enter`, once per session, and no other prompt. Fake panes
//! print the screen and write each line they read to a keys file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use townhall_core::agent::watch::Watcher;
use townhall_core::agent::{self, shell_quote, LaunchOptions, ShellAdapter};
use townhall_core::model::{AgentEvent, EntryBody, NewTask, Task, TaskState};
use townhall_core::{tmux, Context, ContextOptions, Home, Workspace};

mod common;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const TRUST: &str = " Quick safety check: Is this a project you created or one you trust?\n\n   \
                     Yes, I trust this folder\n \u{276f} No, exit\n\n Enter to confirm\n";
const PERMISSION: &str = " Bash command\n\n Do you want to proceed?\n \u{276f} 1. Yes\n   2. No\n";

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
    _repo: TempDir,
    _home: TempDir,
    ctx: Context,
    repo_name: String,
    conversation: String,
    keys: TempDir,
    sessions: Sessions,
}

async fn env() -> Env {
    let repo = common::init_repo().await;
    let home = TempDir::new().unwrap();
    let ws = Workspace::new(Home::at(home.path().join(".townhall")), repo.path()).unwrap();
    let ctx = Context::open(ws, ContextOptions::default()).unwrap();
    let repo_name = ctx.ensure_startup_repo().await.unwrap();
    let conversation = ctx.store().unwrap().create_conversation().await.unwrap().id;
    Env {
        _repo: repo,
        _home: home,
        ctx,
        repo_name,
        conversation,
        keys: TempDir::new().unwrap(),
        sessions: Sessions(vec![]),
    }
}

impl Env {
    fn keys_file(&self, name: &str) -> PathBuf {
        self.keys.path().join(name)
    }

    /// A shell agent with a working task that prints `screen`, then reads lines into its
    /// keys file; `again` prints the screen again after each line.
    async fn agent(&mut self, screen: &str, again: bool) -> String {
        let name = format!(
            "t-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let keys = shell_quote(&self.keys_file(&name).to_string_lossy());
        let show = format!("printf '%s' {}", shell_quote(screen));
        let after = if again {
            show.clone()
        } else {
            "printf '\\033[2J\\033[Hwelcome\\n'".into()
        };
        let script = format!(
            "stty -echo; {show}; while IFS= read -r k; do printf '%s\\n' \"$k\" >> {keys}; {after}; done"
        );
        let ws = self.ctx.startup().clone();
        self.sessions.0.push(tmux::session_name(ws.key(), &name));
        let options = LaunchOptions {
            command: Some(format!("sh -c {}", shell_quote(&script))),
            ..LaunchOptions::default()
        };
        agent::start(&ws, &name, "main", &ShellAdapter, &options)
            .await
            .unwrap();
        self.task(&name).await;
        name
    }

    async fn task(&self, name: &str) {
        let store = self.ctx.store().unwrap();
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
            done: Default::default(),
        };
        store.create_task(&self.conversation, new).await.unwrap();
        let working = move |t: Task| {
            Ok(Task {
                state: TaskState::Working,
                ..t
            })
        };
        store.update_task(name, working).await.unwrap();
    }

    async fn state_of(&self, name: &str) -> TaskState {
        let task = self.ctx.store().unwrap().find_task(name).await.unwrap();
        task.unwrap().state
    }

    async fn agent_entries(&self) -> Vec<EntryBody> {
        let entries = self.ctx.store().unwrap().entries(&self.conversation).await;
        entries
            .unwrap()
            .into_iter()
            .map(|e| e.body)
            .filter(|b| matches!(b, EntryBody::Agent { .. } | EntryBody::NeedsYou { .. }))
            .collect()
    }

    async fn reached(&self, name: &str, until: &Until) -> bool {
        match until {
            Until::Keys => self.keys_file(name).exists(),
            Until::State(want) => self.state_of(name).await == *want,
        }
    }

    /// Polls until `name` reached `until`, failing after a few seconds, then 4 more times.
    async fn poll_until(&self, watcher: &mut Watcher, name: &str, until: Until) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.reached(name, &until).await {
            assert!(Instant::now() < deadline, "the watcher never got there");
            watcher.poll().await;
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        for _ in 0..4 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            watcher.poll().await;
        }
    }
}

enum Until {
    Keys,
    State(TaskState),
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|text| text.lines().map(String::from).collect())
        .unwrap_or_default()
}

fn is_up(line: &str) -> bool {
    line == "\u{1b}[A" || line == "\u{1b}OA"
}

fn trusted(agent: &str) -> EntryBody {
    EntryBody::Agent {
        agent: agent.into(),
        event: AgentEvent::Sent,
        text: "trusted the worktree".into(),
    }
}

#[tokio::test]
async fn the_trust_dialog_of_a_townhall_worktree_gets_up_then_enter_and_one_entry() {
    let mut env = env().await;
    let name = env.agent(TRUST, false).await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    env.poll_until(&mut watcher, &name, Until::Keys).await;
    let lines = read_lines(&env.keys_file(&name));
    assert_eq!(lines.len(), 1, "one line, ended by enter: {lines:?}");
    assert!(is_up(&lines[0]), "up came before enter: {lines:?}");
    assert_eq!(env.agent_entries().await, vec![trusted(&name)]);
    assert_eq!(env.state_of(&name).await, TaskState::Working);
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_trust_dialog_still_there_after_one_answer_is_left_waiting() {
    let mut env = env().await;
    let name = env.agent(TRUST, true).await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    let needs_you = Until::State(TaskState::NeedsYou);
    env.poll_until(&mut watcher, &name, needs_you).await;
    assert_eq!(env.state_of(&name).await, TaskState::NeedsYou);
    let lines = read_lines(&env.keys_file(&name));
    assert_eq!(lines.len(), 1, "answered once only: {lines:?}");
    let asked = EntryBody::NeedsYou {
        agent: name.clone(),
        question: "claude asks whether to trust this worktree".into(),
        choices: vec!["yes, trust it".into(), "no, exit".into()],
        advice: None,
    };
    assert_eq!(env.agent_entries().await, vec![trusted(&name), asked]);
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}

#[tokio::test]
async fn a_permission_prompt_is_never_answered() {
    let mut env = env().await;
    let store = env.ctx.store().unwrap();
    let off = townhall_core::model::SettingKey::AdviserPermissions;
    store.set_setting(off, false.into()).await.unwrap();
    let name = env.agent(PERMISSION, false).await;
    let mut watcher = Watcher::new(env.ctx.clone()).await;
    let needs_you = Until::State(TaskState::NeedsYou);
    env.poll_until(&mut watcher, &name, needs_you).await;
    assert_eq!(env.state_of(&name).await, TaskState::NeedsYou);
    assert!(!env.keys_file(&name).exists(), "no key reached the prompt");
    let entries = env.agent_entries().await;
    assert!(
        matches!(&entries[..], [EntryBody::NeedsYou { question, .. }] if question.starts_with("Do you want to proceed?")),
        "{entries:?}"
    );
    agent::stop(env.ctx.startup(), &name).await.unwrap();
}
