//! The agent watcher (unit 2.5): polls every agent (`fleet::list`) every [`INTERVAL`] and,
//! when a state changes, publishes `agentUpdated`, moves the agent's task and appends its
//! `needsYou` or `agent` entry. The state it acted on is remembered in the store, so a
//! restart repeats neither entries nor events. An exit (error or ended) counts once seen
//! on two polls in a row: a task's worktree exists a moment before its session does.
//! Claude's folder trust dialog in townhall's own worktrees is answered once per session
//! (`trust.rs`).

mod event;
mod sync;
mod trust;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::context::Context;
use crate::home::{repo_key, WORKTREES_DIR};
use crate::model::{AgentInfo, AgentState, ServerEvent, Task};
use crate::{fleet, runner, tmux, Error, Result, Workspace};
use sync::Change;

pub const INTERVAL: Duration = Duration::from_millis(1500);
/// Polls in a row an agent must be missing before its task counts as removed.
const MISSED_POLLS: u32 = 2;

/// Starts the watcher on the runtime; `Background::stop` aborts it.
pub fn start(ctx: &Context) -> Option<JoinHandle<()>> {
    let ctx = ctx.clone();
    Some(tokio::spawn(async move {
        let mut watcher = Watcher::new(ctx).await;
        let mut tick = tokio::time::interval(INTERVAL);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            watcher.poll().await;
        }
    }))
}

/// An agent across repos: names match ignoring case, like the store's.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    repo: String,
    agent: String,
}

impl Key {
    fn of(repo: &str, agent: &str) -> Self {
        Self {
            repo: repo.to_lowercase(),
            agent: agent.to_lowercase(),
        }
    }
}

/// The state last acted on, with the names as listed.
#[derive(Debug, Clone)]
struct Seen {
    repo: String,
    agent: String,
    state: AgentState,
}

pub struct Watcher {
    ctx: Context,
    seen: HashMap<Key, Seen>,
    /// Exits seen once, waiting for the next poll to confirm them.
    settling: HashMap<Key, AgentState>,
    missing: HashMap<Key, u32>,
    /// `<home>/worktrees`: only agents below it get their trust dialog answered.
    worktrees: PathBuf,
    /// Sessions (pane id and start) whose trust dialog was answered, never twice.
    trusted: HashSet<(Key, String)>,
    last_warning: Option<String>,
}

fn terminal(state: &AgentState) -> bool {
    matches!(state, AgentState::Error { .. } | AgentState::Ended)
}

impl Watcher {
    /// A watcher that remembers what the store remembers.
    pub async fn new(ctx: Context) -> Self {
        let worktrees = ctx.home().root().join(WORKTREES_DIR);
        let mut watcher = Self {
            ctx,
            seen: HashMap::new(),
            settling: HashMap::new(),
            missing: HashMap::new(),
            worktrees,
            trusted: HashSet::new(),
            last_warning: None,
        };
        let stored = match watcher.ctx.store() {
            Ok(store) => store.seen_agent_states().await,
            Err(e) => Err(e),
        };
        match stored {
            Ok(rows) => {
                watcher.seen = rows
                    .into_iter()
                    .map(|(repo, agent, state)| {
                        (Key::of(&repo, &agent), Seen { repo, agent, state })
                    })
                    .collect();
            }
            Err(e) => watcher.warn(&format!("starts without its memory: {e}")),
        }
        watcher
    }

    /// One round over every agent. Failures are logged and retried on the next round.
    pub async fn poll(&mut self) {
        let agents = match fleet::list(&self.ctx).await {
            Ok(agents) => agents,
            Err(e) => return self.warn(&format!("cannot list agents: {e}")),
        };
        let tasks = self.tasks().await;
        let listed: HashSet<Key> = agents.iter().map(|a| Key::of(&a.repo, &a.name)).collect();
        for info in &agents {
            self.observe(info, &tasks).await;
        }
        self.sweep(&listed, &tasks).await;
    }

    async fn tasks(&mut self) -> HashMap<String, Task> {
        let all = match self.ctx.store() {
            Ok(store) => store.all_tasks().await,
            Err(e) => Err(e),
        };
        match all {
            Ok(tasks) => tasks
                .into_iter()
                .map(|t| (t.name.to_lowercase(), t))
                .collect(),
            Err(e) => {
                self.warn(&format!("cannot read tasks: {e}"));
                HashMap::new()
            }
        }
    }

    async fn observe(&mut self, info: &AgentInfo, tasks: &HashMap<String, Task>) {
        let key = Key::of(&info.repo, &info.name);
        self.missing.remove(&key);
        let before = self.seen.get(&key).map(|s| s.state.clone());
        let changed = before.as_ref() != Some(&info.state);
        let task = tasks
            .get(&key.agent)
            .filter(|t| t.repo.eq_ignore_ascii_case(&info.repo));
        if changed && self.trust(info, &key, task).await {
            return;
        }
        if changed && terminal(&info.state) {
            let first = self.settling.insert(key.clone(), info.state.clone());
            if first.as_ref() != Some(&info.state) {
                return;
            }
        }
        if let Some(task) = task {
            if !self
                .apply(task, sync::plan(task, &info.state, before.as_ref()))
                .await
            {
                return;
            }
        }
        self.settling.remove(&key);
        if changed {
            if let Some(task) = task {
                self.record_tokens(info, task).await;
            }
            let event = ServerEvent::AgentUpdated {
                agent: info.clone(),
            };
            self.ctx.bus().publish(event);
            self.remember(key, info).await;
        }
    }

    /// Answers the trust dialog of an agent in townhall's worktrees, once per session and
    /// only when the dialog is not the state acted on already. True when it answered: this
    /// poll's state is not acted on, and the next poll shows whether the dialog went away.
    async fn trust(&mut self, info: &AgentInfo, key: &Key, task: Option<&Task>) -> bool {
        if !trust::answers(info, &self.worktrees) {
            return false;
        }
        let ws = match self.workspace_of(&info.repo).await {
            Ok(ws) => ws,
            Err(e) => {
                self.warn(&format!(
                    "cannot trust the worktree of `{}`: {e}",
                    info.name
                ));
                return false;
            }
        };
        let session = match tmux::pane(ws.key(), &info.name).await {
            Ok(Some(pane)) => format!("{}@{}", pane.id, pane.created),
            Ok(None) => return false,
            Err(e) => {
                self.warn(&format!("cannot find the session of `{}`: {e}", info.name));
                return false;
            }
        };
        if !self.trusted.insert((key.clone(), session)) {
            return false;
        }
        if let Err(e) = trust::answer(&ws, &info.name).await {
            self.warn(&format!(
                "cannot trust the worktree of `{}`: {e}",
                info.name
            ));
            return false;
        }
        if let Some(task) = task {
            let entry = Some(trust::entry(&task.name));
            self.apply(task, Some(Change { state: None, entry })).await;
        }
        true
    }

    async fn record_tokens(&mut self, info: &AgentInfo, task: &Task) {
        let found = async {
            let ws = self.workspace_of(&info.repo).await?;
            let listed = crate::worktree::list(&ws).await?;
            let Some(wt) = listed.into_iter().find(|wt| wt.name == info.name) else {
                return Ok(0);
            };
            let projects = crate::steps::claude_projects_dir()?;
            crate::steps::tokens_for_agent(&projects, &wt.path).await
        }
        .await;
        let saved = match (found, self.ctx.store()) {
            (Ok(0), _) => Ok(()),
            (Ok(tokens), Ok(store)) => store.set_task_tokens(&task.name, tokens).await,
            (Err(e), _) | (_, Err(e)) => Err(e),
        };
        if let Err(e) = saved {
            self.warn(&format!("cannot count the tokens of `{}`: {e}", task.name));
        }
    }

    async fn workspace_of(&self, repo: &str) -> Result<Workspace> {
        let repo = self.ctx.repo(repo).await?;
        self.ctx.workspace(&repo)
    }

    /// Applies `change` unless the task moved meanwhile. False when it must be retried.
    async fn apply(&mut self, task: &Task, change: Option<Change>) -> bool {
        let Some(change) = change else {
            return true;
        };
        let store = match self.ctx.store() {
            Ok(store) => store.clone(),
            Err(e) => {
                self.warn(&format!("cannot update tasks: {e}"));
                return false;
            }
        };
        if let Some(state) = change.state {
            let (expected, name) = (task.state, task.name.clone());
            let moved = move |t: Task| {
                if t.state == expected {
                    Ok(Task { state, ..t })
                } else {
                    Err(Error::Conflict(format!("task `{name}` changed meanwhile")))
                }
            };
            match store.update_task(&task.name, moved).await {
                Ok(_) => {}
                Err(Error::Conflict(_)) => return false,
                Err(e) => {
                    self.warn(&format!("cannot update task `{}`: {e}", task.name));
                    return false;
                }
            }
        }
        if let Some(entry) = change.entry {
            if let Err(e) = store
                .append_entry(&task.conversation_id, entry.clone())
                .await
            {
                self.warn(&format!(
                    "cannot note `{}` in its conversation: {e}",
                    task.name
                ));
                return false;
            }
            if let Some(text) = event::for_entry(&self.ctx, task, &entry).await {
                runner::notify(&self.ctx, &task.conversation_id, text);
            }
        }
        true
    }

    /// Agents that were seen but are no longer listed: once missing for a few polls with no
    /// live session either, their task ends and they are forgotten.
    async fn sweep(&mut self, listed: &HashSet<Key>, tasks: &HashMap<String, Task>) {
        let gone: Vec<Key> = self
            .seen
            .keys()
            .filter(|k| !listed.contains(k))
            .cloned()
            .collect();
        for key in gone {
            let misses = self.missing.entry(key.clone()).or_insert(0);
            *misses += 1;
            if *misses < MISSED_POLLS {
                continue;
            }
            let Some(seen) = self.seen.get(&key).cloned() else {
                continue;
            };
            if self.still_running(&seen).await {
                continue;
            }
            let task = tasks
                .get(&key.agent)
                .filter(|t| t.repo.eq_ignore_ascii_case(&seen.repo));
            if let Some(task) = task {
                if !self.apply(task, sync::removed(task)).await {
                    continue;
                }
            }
            self.forget(&key, &seen).await;
        }
    }

    /// A live session of that name in its repo: only the listing failed.
    async fn still_running(&self, seen: &Seen) -> bool {
        let Ok(repo) = self.ctx.repo(&seen.repo).await else {
            return false;
        };
        let key = repo_key(Path::new(&repo.path));
        tmux::list(&key)
            .await
            .is_ok_and(|live| live.iter().any(|n| n.eq_ignore_ascii_case(&seen.agent)))
    }

    async fn remember(&mut self, key: Key, info: &AgentInfo) {
        let seen = Seen {
            repo: info.repo.clone(),
            agent: info.name.clone(),
            state: info.state.clone(),
        };
        self.seen.insert(key, seen);
        let saved = match self.ctx.store() {
            Ok(store) => {
                store
                    .see_agent_state(&info.repo, &info.name, &info.state)
                    .await
            }
            Err(e) => Err(e),
        };
        if let Err(e) = saved {
            self.warn(&format!("cannot remember `{}`: {e}", info.name));
        }
    }

    async fn forget(&mut self, key: &Key, seen: &Seen) {
        self.seen.remove(key);
        self.missing.remove(key);
        self.settling.remove(key);
        self.trusted.retain(|(k, _)| k != key);
        let forgotten = match self.ctx.store() {
            Ok(store) => store.forget_agent_state(&seen.repo, &seen.agent).await,
            Err(e) => Err(e),
        };
        if let Err(e) = forgotten {
            self.warn(&format!("cannot forget `{}`: {e}", seen.agent));
        }
    }

    /// Logs `message` unless it was the last one, so a failing poll does not flood the log.
    fn warn(&mut self, message: &str) {
        if self.last_warning.as_deref() != Some(message) {
            eprintln!("warn: agent watcher: {message}");
            self.last_warning = Some(message.to_string());
        }
    }
}
