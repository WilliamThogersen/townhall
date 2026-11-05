//! Starting ready tasks: `queued` to `working`, one agent per task, under `agentsAtOnce`.

use std::path::{Path, PathBuf};

use crate::agent::{self, Agent, LaunchOptions};
use crate::cmd::run;
use crate::context::Context;
use crate::model::{AgentEvent, EntryBody, Task, TaskState};
use crate::{clock, ids, Error, Result, Workspace};

use super::{agent_type, brief, plan, transition, Pass, TaskLaunch};

/// An `agent` `failed` entry's text stays short: a start error can quote a whole command.
/// `Error::Command` puts the reason first and the command last, so the cut loses the
/// command, never the reason.
const MAX_FAILED_TEXT: usize = 300;

/// Every agent of every registered repo, by repo name. A repo whose agents cannot be listed
/// is kept in `unlisted` with the reason, so none of its tasks start on a guess.
#[derive(Debug, Default)]
struct Fleet {
    agents: Vec<(String, Agent)>,
    unlisted: Vec<(String, String)>,
}

/// What to do with one ready task.
enum Step {
    Adopt,
    Start,
    /// A task handed back to another agent type: its worktree is kept, a new agent runs in it.
    Resume,
    Wait,
    Skip(String),
    /// Another agent holds the name (another repo, or another case): its repo and name.
    Clash(String, String),
}

impl Fleet {
    /// The task's own agent: its exact name, in its repo.
    fn agent_of(&self, task: &Task) -> Option<&Agent> {
        self.agents
            .iter()
            .find(|(repo, a)| repo.eq_ignore_ascii_case(&task.repo) && a.name == task.name)
            .map(|(_, a)| a)
    }

    /// Any other agent whose name is the task's ignoring case: worktree folders and
    /// branches cannot tell them apart on a case-insensitive disk.
    fn clash(&self, task: &Task) -> Option<(String, String)> {
        self.agents
            .iter()
            .find(|(_, a)| a.name.eq_ignore_ascii_case(&task.name))
            .map(|(repo, a)| (repo.clone(), a.name.clone()))
    }

    /// Task agents that hold a slot: `working` or `needsYou`, with a live session. An
    /// interactive claude stays alive after its work, so an `ended` task frees its slot.
    fn active(&self, tasks: &[Task]) -> usize {
        tasks
            .iter()
            .filter(|t| matches!(t.state, TaskState::Working | TaskState::NeedsYou))
            .filter(|t| self.agent_of(t).is_some_and(|a| a.alive))
            .count()
    }
}

async fn survey(ctx: &Context) -> Result<Fleet> {
    let mut fleet = Fleet::default();
    for repo in ctx.repos().await? {
        // A repo whose folder is gone has no agents here; a start there fails with the reason.
        let Ok(ws) = ctx.workspace(&repo) else {
            continue;
        };
        match agent::list(&ws).await {
            Ok(found) => fleet
                .agents
                .extend(found.into_iter().map(|a| (repo.name.clone(), a))),
            Err(e) => fleet.unlisted.push((repo.name.clone(), e.to_string())),
        }
    }
    Ok(fleet)
}

fn decide(fleet: &Fleet, task: &Task, free: usize, handed_back: bool) -> Step {
    let same_repo = |repo: &str| repo.eq_ignore_ascii_case(&task.repo);
    if let Some((_, why)) = fleet.unlisted.iter().find(|(r, _)| same_repo(r)) {
        return Step::Skip(format!(
            "`{}` waits: repo `{}`: {why}",
            task.name, task.repo
        ));
    }
    if fleet.agent_of(task).is_some() {
        return match (handed_back, free) {
            (false, _) => Step::Adopt,
            (true, 0) => Step::Wait,
            (true, _) => Step::Resume,
        };
    }
    match fleet.clash(task) {
        Some((repo, name)) => Step::Clash(repo, name),
        None if free > 0 => Step::Start,
        None => Step::Wait,
    }
}

/// Starts every ready task that has a slot, oldest first, and adopts a queued task whose
/// agent already exists (a start cut short by a restart) instead of starting a second one.
pub(super) async fn start_ready(ctx: &Context, tasks: &[Task]) -> Result<Pass> {
    let ready = plan::ready(tasks);
    if ready.is_empty() {
        return Ok(Pass::default());
    }
    let limit = ctx.settings().await?.agents_at_once;
    let fleet = survey(ctx).await?;
    let mut free = plan::free_slots(limit, fleet.active(tasks));
    let mut pass = Pass::default();
    for task in ready {
        let handed_back = !ctx.store()?.handoffs(&task.name).await?.is_empty();
        let done = act(ctx, task, decide(&fleet, task, free, handed_back)).await?;
        let alive = fleet.agent_of(task).is_some_and(|a| a.alive);
        if matches!(done, Done::Started(_)) || (matches!(done, Done::Adopted(_)) && alive) {
            free = free.saturating_sub(1);
        }
        pass = done.into_pass(pass, &task.name);
    }
    Ok(pass)
}

/// What happened to one ready task.
enum Done {
    Nothing,
    Started(Vec<String>),
    Adopted(Vec<String>),
    Failed,
    Warned(String),
}

impl Done {
    fn into_pass(self, pass: Pass, name: &str) -> Pass {
        let add = |list: Vec<String>| [list, vec![name.to_string()]].concat();
        match self {
            Self::Nothing => pass,
            Self::Started(warnings) => Pass {
                started: add(pass.started),
                warnings: [pass.warnings, warnings].concat(),
                ..pass
            },
            Self::Adopted(warnings) => Pass {
                adopted: add(pass.adopted),
                warnings: [pass.warnings, warnings].concat(),
                ..pass
            },
            Self::Failed => Pass {
                failed: add(pass.failed),
                ..pass
            },
            Self::Warned(why) => Pass {
                warnings: [pass.warnings, vec![why]].concat(),
                ..pass
            },
        }
    }
}

async fn act(ctx: &Context, task: &Task, step: Step) -> Result<Done> {
    let name = &task.name;
    match step {
        Step::Wait => Ok(Done::Nothing),
        Step::Skip(why) => Ok(Done::Warned(why)),
        Step::Adopt => Ok(match adopt(ctx, task).await {
            Ok(warnings) => Done::Adopted(warnings),
            Err(e) => Done::Warned(format!("`{name}` not adopted: {e}")),
        }),
        Step::Clash(repo, held) => {
            let why = format!("an agent named `{held}` already exists in repo `{repo}`");
            fail(ctx, task, &why).await?;
            Ok(Done::Failed)
        }
        Step::Resume => match resume(ctx, task).await {
            Ok(launched) => Ok(Done::Started(settle(ctx, launched).await)),
            Err(e) => {
                fail(ctx, task, &e.to_string()).await?;
                Ok(Done::Failed)
            }
        },
        Step::Start => match launch(ctx, task).await {
            Ok(launched) => Ok(Done::Started(settle(ctx, launched).await)),
            Err(e) => {
                fail(ctx, task, &e.to_string()).await?;
                Ok(Done::Failed)
            }
        },
    }
}

/// The commit `base` names in the repo. Never an option, so git cannot read it as one.
async fn commit_of(ws: &Workspace, base: &str) -> Result<String> {
    if base.trim().is_empty() || base.starts_with('-') {
        return Err(Error::InvalidInput(format!("invalid base branch `{base}`")));
    }
    let spec = format!("{base}^{{commit}}");
    let args = ["rev-parse", "--verify", "--quiet", spec.as_str()];
    run("git", &args, Some(ws.repo()))
        .await
        .map_err(|e| match e {
            Error::Command { .. } => Error::InvalidInput(format!(
                "base branch `{base}` is not a commit in {}",
                ws.repo().display()
            )),
            other => other,
        })
}

async fn attachment_paths(ctx: &Context, task: &Task) -> Result<Vec<PathBuf>> {
    let store = ctx.store()?;
    let mut files = vec![];
    for id in &task.attachments {
        files.push(store.attachment(&task.conversation_id, id).await?.1);
    }
    Ok(files)
}

/// The folders the agent reads without asking: its brief's, and its conversation's
/// `files` folder (each attachment is `files/<id>/<name>`) when it has attachments.
fn read_dirs(brief: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    let attached = files.iter().filter_map(|f| f.parent()?.parent());
    brief
        .parent()
        .into_iter()
        .chain(attached)
        .fold(vec![], |acc: Vec<PathBuf>, dir| {
            match acc.iter().any(|d| d == dir) {
                true => acc,
                false => [acc, vec![dir.to_path_buf()]].concat(),
            }
        })
}

/// Writes the brief, creates the worktree from the commit the base branch is at now
/// (recorded, so `merged` can be told later) and runs the task's agent type in its tmux
/// session: `claude` with the type's flags, a new session id and the order to read the
/// brief, or a `shell` type's command.
async fn launch(ctx: &Context, task: &Task) -> Result<TaskLaunch> {
    let launcher = agent_type::launcher(ctx, task).await?;
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let start_commit = commit_of(&ws, &task.base_branch).await?;
    let files = attachment_paths(ctx, task).await?;
    let handoffs = ctx.store()?.handoffs(&task.name).await?;
    let brief = brief::write(ctx.home(), task, &files, &handoffs).await?;
    let session_id = launcher.claude.then(ids::new_session_id);
    let options = LaunchOptions {
        prompt: Some(brief::order(&brief)),
        session_id: session_id.clone(),
        add_dirs: read_dirs(&brief, &files),
        ..launcher.options
    };
    let adapter = launcher.adapter.as_ref();
    agent::start(&ws, &task.name, &start_commit, adapter, &options).await?;
    Ok(TaskLaunch {
        name: task.name.clone(),
        start_commit,
        session_id,
        launched_at: clock::now_rfc3339(),
    })
}

/// A handed-back task: its brief now holds the hand backs, and the agent type it was handed
/// to runs in the worktree it already has, replacing the session that handed it back. The
/// commit its merge is told from stays the one it first started from.
async fn resume(ctx: &Context, task: &Task) -> Result<TaskLaunch> {
    let launcher = agent_type::launcher(ctx, task).await?;
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let store = ctx.store()?;
    let start_commit = match store.task_launch(&task.name).await? {
        Some(first) => first.start_commit,
        None => record_fork_point(ctx, task).await?,
    };
    let files = attachment_paths(ctx, task).await?;
    let handoffs = store.handoffs(&task.name).await?;
    let brief = brief::write(ctx.home(), task, &files, &handoffs).await?;
    let session_id = launcher.claude.then(ids::new_session_id);
    let options = LaunchOptions {
        prompt: Some(brief::order(&brief)),
        session_id: session_id.clone(),
        add_dirs: read_dirs(&brief, &files),
        ..launcher.options
    };
    agent::restart(&ws, &task.name, launcher.adapter.as_ref(), &options).await?;
    Ok(TaskLaunch {
        name: task.name.clone(),
        start_commit,
        session_id,
        launched_at: clock::now_rfc3339(),
    })
}

/// The agent runs: record its launch and set `working`. A failure here leaves the task
/// queued with its agent, so the next pass adopts it; it is reported, never retried blind.
async fn settle(ctx: &Context, launched: TaskLaunch) -> Vec<String> {
    let name = launched.name.clone();
    let store = match ctx.store() {
        Ok(store) => store,
        Err(e) => return vec![format!("`{name}` started but not saved: {e}")],
    };
    let recorded = store.record_launch(launched).await;
    let working = transition(store, &name, TaskState::Queued, TaskState::Working).await;
    [recorded.err(), working.err()]
        .into_iter()
        .flatten()
        .map(|e| format!("`{name}` started but not saved: {e}"))
        .collect()
}

/// A queued task whose agent exists already: `working`. When its launch was not recorded,
/// the commit its branch forked from is; failing that only costs `merged` (a warning).
async fn adopt(ctx: &Context, task: &Task) -> Result<Vec<String>> {
    let store = ctx.store()?;
    transition(store, &task.name, TaskState::Queued, TaskState::Working).await?;
    if store.task_launch(&task.name).await?.is_some() {
        return Ok(vec![]);
    }
    Ok(match record_fork_point(ctx, task).await {
        Ok(_) => vec![],
        Err(e) => vec![format!("`{}` adopted, merges unknown: {e}", task.name)],
    })
}

async fn record_fork_point(ctx: &Context, task: &Task) -> Result<String> {
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let base = commit_of(&ws, &task.base_branch).await?;
    let args = ["merge-base", base.as_str(), &task.branch()];
    let start_commit = run("git", &args, Some(ws.repo())).await?;
    let launch = TaskLaunch {
        name: task.name.clone(),
        start_commit: start_commit.clone(),
        session_id: None,
        launched_at: clock::now_rfc3339(),
    };
    ctx.store()?.record_launch(launch).await?;
    Ok(start_commit)
}

/// The one `error` the scheduler sets: the agent could not start. Adds `× <name>` with why.
async fn fail(ctx: &Context, task: &Task, why: &str) -> Result<()> {
    let store = ctx.store()?;
    let moved = transition(store, &task.name, TaskState::Queued, TaskState::Error).await?;
    if moved.is_some() {
        let body = EntryBody::Agent {
            agent: task.name.clone(),
            event: AgentEvent::Failed,
            text: plan::capped(&format!("could not start: {why}"), MAX_FAILED_TEXT),
        };
        store.append_entry(&task.conversation_id, body).await?;
    }
    Ok(())
}
