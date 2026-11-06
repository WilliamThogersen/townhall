#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tempfile::TempDir;
use townhall_core::agent::{LaunchOptions, ShellAdapter};
use townhall_core::model::{
    AddRepo, AgentEvent, EntryBody, NewTask, Repo, SettingKey, Task, TaskState,
};
use townhall_core::store::Store;
use townhall_core::{agent, Context, ContextOptions, Home, Workspace};

#[path = "../common/tmux_server.rs"]
mod tmux_server;
use tmux_server::{private_tmux, tmux};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The type `new_task` runs as: claude with nothing but the settings, the argv from before
/// agent types.
pub const PLAIN_TYPE: &str = "name = \"plain\"\nadapter = \"claude\"\n";

pub fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

pub async fn git(dir: &Path, args: &[&str]) -> String {
    townhall_core::cmd::run("git", args, Some(dir))
        .await
        .unwrap()
}

pub async fn commit(dir: &Path, message: &str) {
    let args = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        message,
    ];
    git(dir, &args).await;
}

/// A fake `claude` that writes its arguments next to the test (one per line, named after the
/// worktree folder, which is the task name), then sleeps inside its tmux session.
fn fake_claude(dir: &Path, args_dir: &Path) -> String {
    let path = dir.join("fake-claude");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'/\"$(basename \"$(pwd)\")\".args\nexec sleep 120\n",
        args_dir.display()
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// A temp repo (folder `th23-sched-...`, so its tmux sessions cannot clash with real ones),
/// a temp townhall home, a fake claude and one conversation. Kills its tmux sessions on drop.
pub struct Sched {
    pub root: TempDir,
    pub repo: PathBuf,
    pub repo_name: String,
    pub ws: Workspace,
    pub args_dir: PathBuf,
    pub claude: String,
    pub ctx: Option<Context>,
    pub conversation: String,
}

impl Sched {
    pub async fn new() -> Self {
        private_tmux();
        let root = TempDir::new().unwrap();
        let repo_name = unique("th23-sched");
        let repo = root.path().join(&repo_name);
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]).await;
        commit(&repo, "init").await;
        let repo = repo.canonicalize().unwrap();
        let args_dir = root.path().join("args");
        std::fs::create_dir_all(&args_dir).unwrap();
        let claude = fake_claude(root.path(), &args_dir);
        let ws = Workspace::new(Home::at(root.path().join(".townhall")), &repo).unwrap();
        let ctx = open(&ws, &claude);
        ctx.repos().await.unwrap();
        let store = ctx.store().unwrap();
        store.save_agent_type(PLAIN_TYPE).await.unwrap();
        let floor_off = store.set_setting(SettingKey::MinFreeDiskGb, serde_json::json!(0));
        floor_off.await.unwrap();
        let conversation = store.create_conversation().await.unwrap().id;
        Self {
            root,
            repo,
            repo_name,
            ws,
            args_dir,
            claude,
            ctx: Some(ctx),
            conversation,
        }
    }

    pub fn ctx(&self) -> &Context {
        self.ctx.as_ref().unwrap()
    }

    pub fn store(&self) -> &Store {
        self.ctx().store().unwrap()
    }

    /// Drops the context (and the store lock) and opens a fresh one: a daemon restart.
    pub async fn restart(&mut self) {
        self.ctx = None;
        let ctx = open(&self.ws, &self.claude);
        ctx.repos().await.unwrap();
        self.ctx = Some(ctx);
    }

    pub fn new_task(&self, name: &str, after: &[&str]) -> NewTask {
        NewTask {
            name: name.into(),
            title: format!("do {name}"),
            prompt: format!("the brief for {name}"),
            repo: self.repo_name.clone(),
            base_branch: "main".into(),
            after: after.iter().map(|a| a.to_string()).collect(),
            attachments: vec![],
            agent: "plain".into(),
            pr_base: None,
        }
    }

    pub async fn create(&self, name: &str, after: &[&str]) -> Task {
        let new = self.new_task(name, after);
        self.store()
            .create_task(&self.conversation, new)
            .await
            .unwrap()
    }

    pub async fn state(&self, name: &str) -> TaskState {
        self.store().find_task(name).await.unwrap().unwrap().state
    }

    pub async fn set_state(&self, name: &str, state: TaskState) {
        let change = move |t: Task| Ok(Task { state, ..t });
        self.store().update_task(name, change).await.unwrap();
    }

    /// What the agent watcher (2.5) does when an agent exits: its session is gone, `ended`.
    pub async fn end(&self, name: &str) {
        townhall_core::tmux::kill(self.ws.key(), name)
            .await
            .unwrap();
        self.set_state(name, TaskState::Ended).await;
    }

    pub async fn agents(&self) -> Vec<agent::Agent> {
        agent::list(&self.ws).await.unwrap()
    }

    pub fn worktree(&self, name: &str) -> PathBuf {
        self.ws.worktrees_dir().join(name)
    }

    /// The brief file the scheduler writes for `name`.
    pub fn brief(&self, name: &str) -> PathBuf {
        townhall_core::scheduler::brief_file(self.ws.home(), name)
    }

    pub async fn last_entry(&self) -> EntryBody {
        let entries = self.store().entries(&self.conversation).await.unwrap();
        entries.last().unwrap().body.clone()
    }

    /// The text of the last entry, which must be `× <name> ...`.
    pub async fn failed_text(&self, name: &str) -> String {
        match self.last_entry().await {
            EntryBody::Agent { agent, event, text } => {
                assert_eq!((agent.as_str(), event), (name, AgentEvent::Failed));
                text
            }
            other => panic!("expected a failed agent entry, got {other:?}"),
        }
    }

    pub async fn agent_names(&self) -> Vec<String> {
        let mut all: Vec<String> = self.agents().await.into_iter().map(|a| a.name).collect();
        all.sort();
        all
    }

    /// A second registered repo with one commit on `main`.
    pub async fn other_repo(&self) -> (Repo, Workspace) {
        let path = self.root.path().join(unique("th23-other"));
        std::fs::create_dir_all(&path).unwrap();
        git(&path, &["init", "-q", "-b", "main"]).await;
        commit(&path, "init").await;
        let add = AddRepo {
            path: path.to_string_lossy().into_owned(),
            ..AddRepo::default()
        };
        let repo = self.ctx().registry().unwrap().add(&add).await.unwrap();
        let ws = self.ctx().workspace(&repo).unwrap();
        (repo, ws)
    }

    /// The fake claude's arguments, once it has written them.
    pub async fn args(&self, name: &str) -> String {
        let path = self.args_dir.join(format!("{name}.args"));
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if !text.is_empty() {
                    return text;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the fake claude for {name} never ran");
    }
}

/// An agent that is no task's: `sleep 120` in a worktree of `ws`.
pub async fn sleeper(ws: &Workspace, name: &str) {
    let options = LaunchOptions {
        command: Some("sleep 120".into()),
        ..LaunchOptions::default()
    };
    agent::start(ws, name, "main", &ShellAdapter, &options)
        .await
        .unwrap();
}

/// Kills every tmux session of the repo `ws`, and only those.
pub fn kill_sessions(ws: &Workspace) {
    let prefix = format!("townhall-{}-", ws.key());
    let listed = tmux()
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();
    let out = match listed {
        Ok(out) => out,
        Err(e) => {
            eprintln!("warn: test sessions not listed: {e}");
            return;
        }
    };
    for session in String::from_utf8_lossy(&out.stdout).lines() {
        if session.starts_with(&prefix) {
            let target = format!("={session}");
            let killed = tmux().args(["kill-session", "-t", &target]).output();
            if let Err(e) = killed {
                eprintln!("warn: test session {session} not killed: {e}");
            }
        }
    }
}

fn open(ws: &Workspace, claude: &str) -> Context {
    let options = ContextOptions {
        claude_bin: claude.to_string(),
        notify_delay: Duration::from_secs(3600),
        ..ContextOptions::default()
    };
    Context::open(ws.clone(), options).unwrap()
}

impl Drop for Sched {
    fn drop(&mut self) {
        kill_sessions(&self.ws);
    }
}
