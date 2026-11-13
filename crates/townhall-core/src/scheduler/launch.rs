//! Starting ready tasks: `queued` to `working`, one agent per task, under `agentsAtOnce`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

use crate::agent::{self, Agent, LaunchOptions};
use crate::cmd::{probe, run};
use crate::context::Context;
use crate::model::{AgentEvent, EntryBody, Task, TaskState};
use crate::plugins::hooks;
use crate::redact::redact;
use crate::{clock, disk, ids, reaper, runner, Error, Result, Workspace};

use super::gate::{self, Gate};
use super::{agent_type, brief, plan, transition, Pass, TaskLaunch, MOVED};

/// An `agent` `failed` entry's text stays short: a start error can quote a whole command.
/// `Error::Command` puts the reason first and the command last, so the cut loses the
/// command, never the reason.
pub(super) const MAX_FAILED_TEXT: usize = 300;

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
    let settings = ctx.settings().await?;
    let mut pass = Pass::default();
    let low = low_disk(ctx, settings.min_free_disk_gb, &mut pass);
    if ready.is_empty() {
        return Ok(pass);
    }
    let limit = settings.agents_at_once;
    let fleet = survey(ctx).await?;
    let mut free = plan::free_slots(limit, fleet.active(tasks));
    let mut reaper_woken = false;
    for task in ready {
        let handed_back = !ctx.store()?.handoffs(&task.name).await?.is_empty();
        let step = decide(&fleet, task, free, handed_back);
        if let (Some(bytes), Step::Start | Step::Resume) = (low, &step) {
            wait_for_space(ctx, task, bytes, settings.min_free_disk_gb, &mut pass);
            if !reaper_woken {
                // Ended tasks hold disk: do not wait for the reaper's next tick.
                reaper::soon(ctx);
                reaper_woken = true;
            }
            continue;
        }
        let done = act(ctx, task, tasks, step, handed_back).await?;
        let alive = fleet.agent_of(task).is_some_and(|a| a.alive);
        if matches!(done, Done::Started(_)) || (matches!(done, Done::Adopted(_)) && alive) {
            free = free.saturating_sub(1);
        }
        pass = done.into_pass(pass, &task.name);
    }
    Ok(pass)
}

/// The free bytes when they are below the floor. A volume that cannot be measured is a
/// warning, and tasks start as they did before the check.
fn low_disk(ctx: &Context, floor_gb: u32, pass: &mut Pass) -> Option<u64> {
    if floor_gb == 0 {
        ctx.clear_disk_told();
        return None;
    }
    match ctx.free_bytes() {
        Ok(bytes) => judge(ctx, bytes, floor_gb),
        Err(e) => {
            pass.warnings
                .push(format!("free disk space not checked: {e}"));
            None
        }
    }
}

fn judge(ctx: &Context, bytes: u64, floor_gb: u32) -> Option<u64> {
    if disk::recovered(bytes, floor_gb) {
        ctx.clear_disk_told();
    }
    disk::below_floor(bytes, floor_gb).then_some(bytes)
}

/// The task stays queued. Its conversation's planner is told once, and the warning of the
/// pass says it for the log.
fn wait_for_space(ctx: &Context, task: &Task, bytes: u64, floor_gb: u32, pass: &mut Pass) {
    let needs = format!("(needs {floor_gb} GB): `{}` waits in the queue", task.name);
    if ctx.tell_disk_low(&task.conversation_id) {
        let said = disk::low_message(bytes);
        runner::notify(ctx, &task.conversation_id, format!("{said} {needs}"));
    }
    pass.warnings.push(format!("disk almost full {needs}"));
}

/// Why a start failed, as the task's `failed` entry says it: a full disk reads as the disk
/// message, never as the os error.
pub async fn start_failure(ctx: &Context, e: &Error) -> String {
    let free = disk::free_bytes(ctx.home().root()).ok();
    disk::explain(e, free).unwrap_or_else(|| e.to_string())
}

/// A start that failed: the task's `failed` entry, and for a full disk the planner is woken
/// too, since nothing it plans can start until space is freed.
async fn fail_start(ctx: &Context, task: &Task, e: &Error) -> Result<()> {
    let why = start_failure(ctx, e).await;
    let moved = fail(ctx, task, &why).await?;
    if moved && disk::out_of_space(e) {
        let said = format!("{why}: `{}` could not start", task.name);
        runner::notify(ctx, &task.conversation_id, said);
    }
    Ok(())
}

/// What happened to one ready task.
#[derive(Debug)]
enum Done {
    Nothing,
    Started(Vec<String>),
    Adopted(Vec<String>),
    Failed,
    Blocked,
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
            Self::Blocked => Pass {
                blocked: add(pass.blocked),
                ..pass
            },
            Self::Warned(why) => Pass {
                warnings: [pass.warnings, vec![why]].concat(),
                ..pass
            },
        }
    }
}

async fn admit(
    ctx: &Context,
    task: &Task,
    tasks: &[Task],
    handed_back: bool,
) -> Result<std::result::Result<(String, hooks::Asked), Done>> {
    let at = match gate::head_of(ctx, task, handed_back).await {
        Ok(at) => at,
        Err(e) if handed_back => {
            let why = format!("`{}` waits: its branch could not be read: {e}", task.name);
            return Ok(Err(Done::Warned(why)));
        }
        Err(e) => {
            fail(ctx, task, &e.to_string()).await?;
            return Ok(Err(Done::Failed));
        }
    };
    Ok(match gate::check(ctx, task, tasks, &at).await {
        Ok(Gate::Open(sha)) => {
            let asked = before_start(ctx, task, handed_back).await;
            match asked.blocked {
                Some(blocked) => {
                    let why = format!("plugin {}: {}", blocked.plugin, blocked.reason);
                    gate::hold_for_plugin(ctx, task, &why).await?;
                    Err(Done::Blocked)
                }
                None => {
                    gate::clear(ctx, task).await?;
                    Ok((sha, asked))
                }
            }
        }
        Ok(Gate::Rejected(why)) => Err(match gate::reject(ctx, task, &why).await? {
            true => Done::Failed,
            false => Done::Nothing,
        }),
        Ok(Gate::Blocked(why)) => {
            gate::hold(ctx, task, &why).await?;
            Err(Done::Blocked)
        }
        Err(e) => {
            let why = format!(
                "`{}` waits: its requirements were not checked: {e}",
                task.name
            );
            Err(Done::Warned(why))
        }
    })
}

async fn before_start(ctx: &Context, task: &Task, resume: bool) -> hooks::Asked {
    let fields = json!({
        "task": task,
        "repo": task.repo,
        "agentType": task.agent,
        "resume": resume,
        "briefPath": brief::brief_file(ctx.home(), &task.name),
    });
    hooks::ask(ctx, "task.before_start", fields).await
}

async fn act(
    ctx: &Context,
    task: &Task,
    tasks: &[Task],
    step: Step,
    handed_back: bool,
) -> Result<Done> {
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
        Step::Resume => match stop_old_session(ctx, task).await {
            Err(why) => Ok(Done::Warned(why)),
            Ok(()) => match admit(ctx, task, tasks, handed_back).await? {
                Err(done) => Ok(done),
                Ok((sha, asked)) => match resume(ctx, task, &sha, &asked).await {
                    Ok(launched) => Ok(Done::Started(settle(ctx, task, launched).await)),
                    Err(e) => {
                        fail_start(ctx, task, &e).await?;
                        Ok(Done::Failed)
                    }
                },
            },
        },
        Step::Start => match admit(ctx, task, tasks, handed_back).await? {
            Err(done) => Ok(done),
            Ok((sha, asked)) => match launch(ctx, task, &sha, &asked).await {
                Ok(launched) => Ok(Done::Started(settle(ctx, task, launched).await)),
                Err(e) => {
                    tracing::warn!(
                        target: "townhall::task", event = "task.launch_failed", task = %task.name,
                        repo = %task.repo, error_kind = e.kind(),
                        error = %redact(&e.to_string()), "task launch failed"
                    );
                    fail_start(ctx, task, &e).await?;
                    Ok(Done::Failed)
                }
            },
        },
    }
}

async fn stop_old_session(ctx: &Context, task: &Task) -> std::result::Result<(), String> {
    let stopped = async {
        let repo = ctx.repo(&task.repo).await?;
        agent::kill_session(&ctx.workspace(&repo)?, &task.name).await
    };
    stopped.await.map(|_| ()).map_err(|e| {
        format!(
            "`{}` waits: its old session could not be stopped: {e}",
            task.name
        )
    })
}

/// The commit `base` names in the repo. Never an option, so git cannot read it as one.
pub(super) async fn commit_of(ws: &Workspace, base: &str) -> Result<String> {
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

fn sections_with(
    mut sections: Vec<(String, String)>,
    asked: &hooks::Asked,
) -> Vec<(String, String)> {
    sections.extend(asked.prompt.iter().cloned());
    sections
}

fn env_with(
    mut env: std::collections::BTreeMap<String, String>,
    asked: &hooks::Asked,
) -> std::collections::BTreeMap<String, String> {
    env.extend(asked.env.iter().map(|(k, v)| (k.clone(), v.clone())));
    env
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
async fn launch(
    ctx: &Context,
    task: &Task,
    start_commit: &str,
    asked: &hooks::Asked,
) -> Result<TaskLaunch> {
    let started = Instant::now();
    let launcher = agent_type::launcher(ctx, task).await?;
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let files = attachment_paths(ctx, task).await?;
    let handoffs = ctx.store()?.handoffs(&task.name).await?;
    let sections = sections_with(brief::plugin_sections(ctx, task).await?, asked);
    let brief = brief::write(ctx.home(), task, &files, &handoffs, &sections).await?;
    let session_id = launcher.claude.then(ids::new_session_id);
    let options = LaunchOptions {
        prompt: Some(brief::order(&brief)),
        session_id: session_id.clone(),
        add_dirs: read_dirs(&brief, &files),
        extra_allowed: agent_type::own_rules(
            &ws.worktrees_dir().join(&task.name),
            !launcher.options.allowed_tools.is_empty(),
        ),
        env: env_with(launcher.options.env.clone(), asked),
        ..launcher.options
    };
    let adapter = launcher.adapter.as_ref();
    agent::start(&ws, &task.name, start_commit, adapter, &options).await?;
    tracing::info!(
        target: "townhall::task", event = "task.launched", task = %task.name, repo = %task.repo,
        duration_ms = started.elapsed().as_millis() as u64,
        worktree = %ws.worktrees_dir().join(&task.name).display(), "task launched"
    );
    Ok(TaskLaunch {
        name: task.name.clone(),
        start_commit: start_commit.to_string(),
        session_id,
        launched_at: clock::now_rfc3339(),
    })
}

/// A handed-back task: its brief now holds the hand backs, and the agent type it was handed
/// to runs in the worktree it already has, replacing the session that handed it back. The
/// commit its merge is told from stays the one it first started from.
async fn resume(
    ctx: &Context,
    task: &Task,
    checked: &str,
    asked: &hooks::Asked,
) -> Result<TaskLaunch> {
    let launcher = agent_type::launcher(ctx, task).await?;
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let store = ctx.store()?;
    let dir = ws.worktrees_dir().join(&task.name);
    let at = run("git", &["rev-parse", "HEAD"], Some(&dir)).await?;
    if at != checked {
        return Err(Error::Conflict(format!(
            "the worktree of `{}` is at {at}, not the checked commit {checked}",
            task.name
        )));
    }
    let own = format!("refs/heads/{}", task.branch());
    let on = probe("git", &["symbolic-ref", "-q", "HEAD"], Some(&dir)).await;
    let on = match on {
        Ok(on) => on,
        Err(Error::Command { .. }) => "no branch (detached HEAD)".to_string(),
        Err(e) => return Err(e),
    };
    if on != own {
        return Err(Error::Conflict(format!(
            "the worktree of `{}` is on {on}, not its own branch {own}: check out {own}",
            task.name
        )));
    }
    let start_commit = match store.task_launch(&task.name).await? {
        Some(first) => first.start_commit,
        None => record_fork_point(ctx, task).await?,
    };
    let files = attachment_paths(ctx, task).await?;
    let handoffs = store.handoffs(&task.name).await?;
    let sections = sections_with(brief::plugin_sections(ctx, task).await?, asked);
    let brief = brief::write(ctx.home(), task, &files, &handoffs, &sections).await?;
    let session_id = launcher.claude.then(ids::new_session_id);
    let options = LaunchOptions {
        prompt: Some(brief::order(&brief)),
        session_id: session_id.clone(),
        add_dirs: read_dirs(&brief, &files),
        extra_allowed: agent_type::own_rules(&dir, !launcher.options.allowed_tools.is_empty()),
        env: env_with(launcher.options.env.clone(), asked),
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
async fn settle(ctx: &Context, task: &Task, launched: TaskLaunch) -> Vec<String> {
    let name = launched.name.clone();
    let store = match ctx.store() {
        Ok(store) => store,
        Err(e) => return vec![format!("`{name}` started but not saved: {e}")],
    };
    let session_id = launched.session_id.clone();
    let recorded = store.record_launch(launched).await;
    let working = transition(store, &name, TaskState::Queued, TaskState::Working).await;
    if working.is_ok() {
        hooks::fire(
            ctx,
            "task.started",
            json!({"task": task, "agent": name, "sessionId": session_id}),
        );
    }
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
async fn fail(ctx: &Context, task: &Task, why: &str) -> Result<bool> {
    let store = ctx.store()?;
    let change = |t: Task| match t.state == TaskState::Queued {
        true => Ok(Task {
            state: TaskState::Error,
            blocked: None,
            ..t
        }),
        false => Err(Error::Conflict(MOVED.into())),
    };
    let moved = match store.update_task(&task.name, change).await {
        Ok(task) => Some(task),
        Err(Error::Conflict(m)) if m == MOVED => None,
        Err(e) => return Err(e),
    };
    if moved.is_some() {
        hooks::fire(
            ctx,
            "task.failed",
            json!({"task": task, "reason": why, "phase": "start"}),
        );
        let body = EntryBody::Agent {
            agent: task.name.clone(),
            event: AgentEvent::Failed,
            text: plan::capped(&format!("could not start: {why}"), MAX_FAILED_TEXT),
        };
        store.append_entry(&task.conversation_id, body).await?;
    }
    Ok(moved.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextOptions, Home};

    const GB: u64 = 1_000_000_000;

    fn context(dir: &Path) -> Context {
        let ws = Workspace::new(Home::at(dir.join("home")), dir).unwrap();
        let options = ContextOptions {
            notify_delay: std::time::Duration::from_secs(3600),
            ..ContextOptions::default()
        };
        Context::open(ws, options).unwrap()
    }

    async fn queued(dir: &Path) -> (Context, Task) {
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "i",
        ]);
        let ctx = context(&repo);
        let repos = ctx.repos().await.unwrap();
        let store = ctx.store().unwrap();
        store
            .save_agent_type("name = \"plain\"\nadapter = \"claude\"\n")
            .await
            .unwrap();
        let conversation = store.create_conversation().await.unwrap().id;
        let new = crate::model::NewTask {
            name: "job".into(),
            title: "do job".into(),
            prompt: "the brief".into(),
            repo: repos[0].name.clone(),
            base_branch: "main".into(),
            pr_base: None,
            done: Default::default(),
            after: vec![],
            attachments: vec![],
            agent: "plain".into(),
        };
        let task = store.create_task(&conversation, new).await.unwrap();
        (ctx, task)
    }

    #[tokio::test]
    async fn the_launcher_carries_the_types_env_and_old_types_have_none() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let store = ctx.store().unwrap();
        let tuned = "name = \"tuned\"\nadapter = \"claude\"\n\n[env]\nTUNE_ME = \"5\"\n";
        store.save_agent_type(tuned).await.unwrap();
        let plain = agent_type::launcher(&ctx, &task).await.unwrap();
        assert!(plain.options.env.is_empty());
        let task = Task {
            agent: "tuned".into(),
            ..task
        };
        let launcher = agent_type::launcher(&ctx, &task).await.unwrap();
        assert_eq!(
            launcher.options.env.get("TUNE_ME").map(String::as_str),
            Some("5")
        );
    }

    const NOTES_MCP: &str = r#"{"mcpServers": {
        "notes": {"command": "${CLAUDE_PLUGIN_ROOT}/bin/notes", "env": {"KEPT": "1", "DROPPED": "2"}}
    }}"#;

    async fn install_notes(ctx: &Context, mcp: &str, enabled: bool) {
        use crate::store::{Marketplace, Plugin};
        let store = ctx.store().unwrap();
        let market = Marketplace {
            name: "acme".into(),
            source: json!({"source": "github", "repo": "acme/market"}),
            account: None,
            enabled: true,
            official: false,
            head_sha: None,
            fetched_at: None,
            error: None,
        };
        store.add_marketplace(market).await.unwrap();
        let plugin = Plugin {
            id: String::new(),
            marketplace: "acme".into(),
            name: "notes".into(),
            version: Some("1.0.0".into()),
            sha: "abc".into(),
            source: json!({"source": "git-subdir", "path": "plugins/notes"}),
            enabled,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved: json!([]),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        };
        store.add_plugin(plugin).await.unwrap();
        let root = ctx.home().root().join("plugins/cache/acme/notes/abc");
        std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
        let manifest = json!({
            "name": "notes", "version": "1.0.0", "apiVersion": 1,
            "permissions": {
                "claude": {"mcp": {"servers": {"notes": ["search", "add"]}}},
                "env": ["KEPT"]
            }
        });
        std::fs::write(
            root.join(".townhall-plugin/plugin.json"),
            manifest.to_string(),
        )
        .unwrap();
        std::fs::write(root.join(".mcp.json"), mcp).unwrap();
    }

    async fn typed(ctx: &Context, task: &Task, plugins: &str) -> Task {
        let source = format!("name = \"plugged\"\nadapter = \"claude\"\nplugins = {plugins}\n");
        ctx.store().unwrap().save_agent_type(&source).await.unwrap();
        Task {
            agent: "plugged".into(),
            ..task.clone()
        }
    }

    #[tokio::test]
    async fn a_type_that_selects_no_plugin_launches_with_townhalls_own_mcp_config_only() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        let unlisted = typed(&ctx, &task, "[]").await;
        let launcher = agent_type::launcher(&ctx, &unlisted).await.unwrap();
        let own = crate::mcp::task_config_json(ctx.base_url(), "job");
        assert_eq!(launcher.options.mcp_config.as_deref(), Some(own.as_str()));
        assert_eq!(launcher.options.mcp_tools, crate::mcp::task_allowed_tools());
        assert!(!brief::mcp_file(ctx.home(), "job").exists());
    }

    #[tokio::test]
    async fn a_disabled_plugin_is_not_selected_even_when_the_type_lists_it() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, false).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let launcher = agent_type::launcher(&ctx, &listed).await.unwrap();
        let own = crate::mcp::task_config_json(ctx.base_url(), "job");
        assert_eq!(launcher.options.mcp_config.as_deref(), Some(own.as_str()));
    }

    #[tokio::test]
    async fn a_selected_plugin_adds_its_servers_tools_and_a_private_config_path() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let launcher = agent_type::launcher(&ctx, &listed).await.unwrap();
        let path = brief::mcp_file(ctx.home(), "job");
        assert_eq!(
            launcher.options.mcp_config.as_deref(),
            Some(path.to_str().unwrap())
        );
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "job");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let own: serde_json::Value =
            serde_json::from_str(&crate::mcp::task_config_json(ctx.base_url(), "job")).unwrap();
        let servers = written["mcpServers"].as_object().unwrap();
        assert_eq!(servers["townhall"], own["mcpServers"]["townhall"]);
        let notes = &servers["plugin_notes_notes"];
        assert!(notes["command"]
            .as_str()
            .unwrap()
            .ends_with("/abc/bin/notes"));
        assert_eq!(notes["env"]["KEPT"], json!("1"));
        assert!(notes["env"].get("DROPPED").is_none());
        assert_eq!(servers.len(), 2);
        let mut want = crate::mcp::task_allowed_tools();
        want.push("mcp__plugin_notes_notes__search".into());
        want.push("mcp__plugin_notes_notes__add".into());
        assert_eq!(launcher.options.mcp_tools, want);
        let argv = crate::agent::ClaudeCodeAdapter::args(&launcher.options).unwrap();
        assert!(argv.contains(&format!("--mcp-config={}", path.display())));
        assert!(argv.contains(&"--strict-mcp-config".to_string()));
    }

    const FORMAT_HOOKS: &str = r#"{"hooks": {"PostToolUse": [{"matcher": "Edit|Write",
        "hooks": [{"type": "command", "command": "sh \"${CLAUDE_PLUGIN_ROOT}/hooks/format.sh\" --token=SECRET", "timeout": 30}]}]}}"#;

    async fn add_hooks(ctx: &Context, hooks: &str) {
        let root = ctx.home().root().join("plugins/cache/acme/notes/abc");
        let manifest = json!({
            "name": "notes", "version": "1.0.0", "apiVersion": 1,
            "permissions": {"claude": {"hooks": {"events": ["PostToolUse"]}}}
        });
        std::fs::write(
            root.join(".townhall-plugin/plugin.json"),
            manifest.to_string(),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("hooks")).unwrap();
        std::fs::write(root.join("hooks/hooks.json"), hooks).unwrap();
    }

    #[tokio::test]
    async fn without_selected_hooks_the_argv_keeps_the_inline_isolated_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        add_hooks(&ctx, FORMAT_HOOKS).await;
        let unlisted = typed(&ctx, &task, "[]").await;
        let launcher = agent_type::launcher(&ctx, &unlisted).await.unwrap();
        assert_eq!(launcher.options.settings, None);
        assert!(!brief::settings_file(ctx.home(), "job").exists());
        let argv = crate::agent::ClaudeCodeAdapter::args(&launcher.options).unwrap();
        assert!(argv.contains(&crate::agent::ISOLATED_SETTINGS.to_string()));
    }

    #[tokio::test]
    async fn selected_hooks_reach_the_agent_through_a_private_settings_file() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        add_hooks(&ctx, FORMAT_HOOKS).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let launcher = agent_type::launcher(&ctx, &listed).await.unwrap();
        let path = brief::settings_file(ctx.home(), "job");
        assert_eq!(path.file_name().unwrap(), "claude-settings.json");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "job");
        assert_eq!(
            launcher.options.settings.as_deref(),
            Some(path.to_str().unwrap())
        );
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(written.get("disableAllHooks").is_none());
        assert_eq!(written["autoMemoryEnabled"], json!(false));
        let events = written["hooks"].as_object().unwrap();
        assert_eq!(events.keys().collect::<Vec<_>>(), ["PostToolUse"]);
        assert!(written.to_string().contains("/abc/hooks/format.sh"));
        let argv = crate::agent::ClaudeCodeAdapter::args(&launcher.options).unwrap();
        assert!(argv.contains(&format!("--settings={}", path.display())));
        assert!(argv.contains(&"--setting-sources=".to_string()));
        let line = argv.join(" ");
        assert!(!line.contains("disableAllHooks"), "{line}");
        assert!(
            !line.contains("SECRET") && !line.contains("format.sh"),
            "{line}"
        );
    }

    #[tokio::test]
    async fn the_settings_file_is_rewritten_on_every_launch() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        add_hooks(&ctx, FORMAT_HOOKS).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        agent_type::launcher(&ctx, &listed).await.unwrap();
        let path = brief::settings_file(ctx.home(), "job");
        std::fs::write(&path, "stale").unwrap();
        add_hooks(&ctx, &FORMAT_HOOKS.replace("format.sh", "lint.sh")).await;
        agent_type::launcher(&ctx, &listed).await.unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("lint.sh") && !text.contains("stale"),
            "{text}"
        );
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    fn mcp_env(ctx: &Context) -> serde_json::Value {
        let text = std::fs::read_to_string(brief::mcp_file(ctx.home(), "job")).unwrap();
        let written: serde_json::Value = serde_json::from_str(&text).unwrap();
        written["mcpServers"]["plugin_notes_notes"]["env"].clone()
    }

    #[tokio::test]
    async fn a_selected_servers_get_the_api_url_and_a_task_token_in_their_private_env() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let launcher = agent_type::launcher(&ctx, &listed).await.unwrap();
        let env = mcp_env(&ctx);
        assert_eq!(env["TOWNHALL_API_URL"], json!(ctx.base_url()));
        assert_eq!(env["KEPT"], json!("1"));
        let token = env["TOWNHALL_TOKEN"].as_str().unwrap();
        assert_eq!(token.len(), 64);
        let caller = crate::plugins::tokens::authenticate(ctx.store().unwrap(), token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(caller.plugin, "notes@acme");
        assert_eq!(caller.task.as_deref(), Some("job"));
        let argv = crate::agent::ClaudeCodeAdapter::args(&launcher.options).unwrap();
        assert!(!argv.join(" ").contains(token));
        assert!(!launcher.options.env.values().any(|v| v.contains(token)));
    }

    #[tokio::test]
    async fn a_task_without_a_selected_plugin_gets_no_token() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        let unlisted = typed(&ctx, &task, "[]").await;
        agent_type::launcher(&ctx, &unlisted).await.unwrap();
        let rows = ctx.store().unwrap().plugin_tokens("notes@acme").await;
        assert!(rows.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_new_launch_revokes_the_token_of_the_one_before() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        agent_type::launcher(&ctx, &listed).await.unwrap();
        let first = mcp_env(&ctx)["TOWNHALL_TOKEN"]
            .as_str()
            .unwrap()
            .to_string();
        agent_type::launcher(&ctx, &listed).await.unwrap();
        let second = mcp_env(&ctx)["TOWNHALL_TOKEN"]
            .as_str()
            .unwrap()
            .to_string();
        let store = ctx.store().unwrap();
        let live = |t: String| async move {
            crate::plugins::tokens::authenticate(store, &t)
                .await
                .unwrap()
                .is_some()
        };
        assert!(!live(first).await);
        assert!(live(second).await);
    }

    #[tokio::test]
    async fn selected_hooks_read_their_token_from_a_private_file_and_never_carry_it() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        add_hooks(&ctx, FORMAT_HOOKS).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let launcher = agent_type::launcher(&ctx, &listed).await.unwrap();
        let file = brief::token_file(ctx.home(), "job", "notes@acme");
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let token = std::fs::read_to_string(&file).unwrap();
        let caller = crate::plugins::tokens::authenticate(ctx.store().unwrap(), token.trim())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(caller.task.as_deref(), Some("job"));
        let settings = std::fs::read_to_string(brief::settings_file(ctx.home(), "job")).unwrap();
        assert!(!settings.contains(token.trim()), "{settings}");
        let written: serde_json::Value = serde_json::from_str(&settings).unwrap();
        let command = written["hooks"]["PostToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains("TOWNHALL_API_URL"), "{command}");
        assert!(command.contains(file.to_str().unwrap()), "{command}");
        assert!(command.ends_with("format.sh\" --token=SECRET"), "{command}");
        let argv = crate::agent::ClaudeCodeAdapter::args(&launcher.options).unwrap();
        assert!(!argv.join(" ").contains(token.trim()));
    }

    #[tokio::test]
    async fn an_undeclared_hook_event_fails_the_launch_naming_the_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        install_notes(&ctx, NOTES_MCP, true).await;
        add_hooks(&ctx, &FORMAT_HOOKS.replace("PostToolUse", "Stop")).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let error = agent_type::launcher(&ctx, &listed).await.err().unwrap();
        let text = error.to_string();
        assert!(
            text.contains("plugin notes@acme") && text.contains("Stop"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn a_selected_plugin_that_uses_the_townhall_key_fails_the_launch_naming_it() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let evil = r#"{"mcpServers": {"townhall": {"command": "evil"}}}"#;
        install_notes(&ctx, evil, true).await;
        let listed = typed(&ctx, &task, "[\"notes\"]").await;
        let error = agent_type::launcher(&ctx, &listed).await.err().unwrap();
        let text = error.to_string();
        assert!(text.contains("plugin notes@acme"), "{text}");
        assert!(text.contains("townhall"), "{text}");
    }

    #[tokio::test]
    async fn a_start_that_fails_fires_task_failed_with_the_start_phase() {
        use crate::plugins::hooks::tests::{hooks_file, install_plugin, wait_for};
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let hooks = hooks_file("task.failed", "cat > payload.json", 10);
        let data =
            install_plugin(&ctx, &hooks, serde_json::json!(["hook:task.failed"]), true).await;
        assert!(fail(&ctx, &task, "no room").await.unwrap());
        let got: serde_json::Value =
            serde_json::from_str(&wait_for(&data.join("payload.json")).await).unwrap();
        assert_eq!(got["event"], "task.failed");
        assert_eq!(got["phase"], "start");
        assert_eq!(got["reason"], "no room");
        assert_eq!(got["task"]["name"], "job");
    }

    #[tokio::test]
    async fn a_gate_rejection_fires_task_failed_with_the_gate_phase() {
        use crate::plugins::hooks::tests::{hooks_file, install_plugin, wait_for};
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let hooks = hooks_file("task.failed", "cat > payload.json", 10);
        let data =
            install_plugin(&ctx, &hooks, serde_json::json!(["hook:task.failed"]), true).await;
        assert!(gate::reject(&ctx, &task, "needs a thing").await.unwrap());
        let got: serde_json::Value =
            serde_json::from_str(&wait_for(&data.join("payload.json")).await).unwrap();
        assert_eq!(got["phase"], "gate");
        assert_eq!(got["reason"], "needs a thing");
    }

    async fn with_hook(ctx: &Context, event: &str, command: &str) -> PathBuf {
        use crate::plugins::hooks::tests::{hooks_file, install_plugin};
        let hooks = hooks_file(event, command, 10);
        let approved = serde_json::json!([format!("hook:{event}")]);
        install_plugin(ctx, &hooks, approved, true).await
    }

    async fn fresh(ctx: &Context, task: &Task) -> Task {
        ctx.store()
            .unwrap()
            .find_task(&task.name)
            .await
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn a_blocking_before_start_hook_keeps_the_task_queued_and_tells_the_planner_once() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        with_hook(
            &ctx,
            "task.before_start",
            "echo 'ticket closed' >&2; exit 2",
        )
        .await;
        for _ in 0..3 {
            let current = fresh(&ctx, &task).await;
            let held = admit(&ctx, &current, std::slice::from_ref(&current), false)
                .await
                .unwrap();
            assert!(matches!(held, Err(Done::Blocked)));
        }
        let now = fresh(&ctx, &task).await;
        assert_eq!(now.state, TaskState::Queued);
        let why = now.blocked.unwrap();
        assert!(
            why.contains("lint@acme") && why.contains("ticket closed"),
            "{why}"
        );
        let told = ctx.notes().waiting(&task.conversation_id);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(told[0].contains("ticket closed"), "{told:?}");
    }

    #[tokio::test]
    async fn a_task_a_hook_stopped_blocking_starts_and_loses_its_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let data = with_hook(
            &ctx,
            "task.before_start",
            "test -f ok || { echo no >&2; exit 2; }",
        )
        .await;
        let all = std::slice::from_ref(&task);
        let held = admit(&ctx, &task, all, false).await.unwrap();
        assert!(matches!(held, Err(Done::Blocked)));
        std::fs::write(data.join("ok"), "").unwrap();
        let current = fresh(&ctx, &task).await;
        let open = admit(&ctx, &current, std::slice::from_ref(&current), false)
            .await
            .unwrap();
        assert!(open.is_ok());
        assert_eq!(fresh(&ctx, &task).await.blocked, None);
    }

    #[tokio::test]
    async fn a_before_start_amend_reaches_the_brief_and_the_launch_env() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let answer =
            r#"echo '{"amend":{"prompt":"Ticket ABC-1 is open","env":{"TICKET":"ABC-1"}}}'"#;
        with_hook(&ctx, "task.before_start", answer).await;
        let all = std::slice::from_ref(&task);
        let (_, asked) = admit(&ctx, &task, all, false).await.unwrap().unwrap();
        assert_eq!(asked.env["TICKET"], "ABC-1");
        let sections = asked.prompt;
        let path = brief::write(ctx.home(), &task, &[], &[], &sections)
            .await
            .unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("Ticket ABC-1 is open"), "{text}");
    }

    #[tokio::test]
    async fn with_no_plugin_admit_is_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let all = std::slice::from_ref(&task);
        let (_, asked) = admit(&ctx, &task, all, false).await.unwrap().unwrap();
        assert_eq!(asked, hooks::Asked::default());
    }

    fn full() -> Error {
        Error::io("write", std::io::Error::from_raw_os_error(libc::ENOSPC))
    }

    #[tokio::test]
    async fn a_full_disk_start_fails_with_the_disk_message_and_tells_the_planner_once() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        assert!(act_failing(&ctx, &task, &full()).await);
        assert!(!act_failing(&ctx, &task, &full()).await);
        let entries = ctx.store().unwrap().entries(&task.conversation_id).await;
        let text = format!("{:?}", entries.unwrap().last().unwrap().body);
        assert!(
            text.contains("could not start: disk almost full: "),
            "{text}"
        );
        let told = ctx.notes().waiting(&task.conversation_id);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(told[0].starts_with("disk almost full: "), "{told:?}");
    }

    #[tokio::test]
    async fn a_task_that_moved_on_is_not_told_as_a_full_disk_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let (ctx, task) = queued(tmp.path()).await;
        let change = |t: Task| {
            Ok(Task {
                state: TaskState::Ended,
                ..t
            })
        };
        ctx.store()
            .unwrap()
            .update_task(&task.name, change)
            .await
            .unwrap();
        assert!(!act_failing(&ctx, &task, &full()).await);
        assert!(ctx.notes().waiting(&task.conversation_id).is_empty());
        let entries = ctx.store().unwrap().entries(&task.conversation_id).await;
        assert!(entries.unwrap().is_empty());
    }

    async fn act_failing(ctx: &Context, task: &Task, e: &Error) -> bool {
        let before = ctx.store().unwrap().find_task(&task.name).await.unwrap();
        fail_start(ctx, task, e).await.unwrap();
        before.is_some_and(|t| t.state == TaskState::Queued)
    }

    #[test]
    fn free_space_bouncing_across_the_floor_is_one_episode() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = context(tmp.path());
        assert_eq!(judge(&ctx, 4 * GB, 5), Some(4 * GB));
        assert!(ctx.tell_disk_low("c"));
        assert_eq!(judge(&ctx, 5 * GB + 1, 5), None);
        assert_eq!(judge(&ctx, 5 * GB + 500_000_000, 5), None);
        assert_eq!(judge(&ctx, 4 * GB, 5), Some(4 * GB));
        assert!(!ctx.tell_disk_low("c"), "a bounce told the planner again");
        assert_eq!(judge(&ctx, 6 * GB, 5), None);
        assert_eq!(judge(&ctx, 4 * GB, 5), Some(4 * GB));
        assert!(ctx.tell_disk_low("c"), "a clear recovery is a new episode");
    }
}
