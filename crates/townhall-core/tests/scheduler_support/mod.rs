#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tempfile::TempDir;
use townhall_core::model::{NewTask, Task, TaskState};
use townhall_core::store::Store;
use townhall_core::{agent, Context, ContextOptions, Home, Workspace};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

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
        let conversation = ctx.store().unwrap().create_conversation().await.unwrap().id;
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

fn open(ws: &Workspace, claude: &str) -> Context {
    let options = ContextOptions {
        claude_bin: claude.to_string(),
        ..ContextOptions::default()
    };
    Context::open(ws.clone(), options).unwrap()
}

impl Drop for Sched {
    fn drop(&mut self) {
        let prefix = format!("townhall-{}-", self.ws.key());
        let listed = std::process::Command::new("tmux")
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
                let killed = std::process::Command::new("tmux")
                    .args(["kill-session", "-t", &target])
                    .output();
                if let Err(e) = killed {
                    eprintln!("warn: test session {session} not killed: {e}");
                }
            }
        }
    }
}
