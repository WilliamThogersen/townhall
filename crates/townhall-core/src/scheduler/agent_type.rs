//! The agent type a task runs as (2.12): its adapter and launch options. The type's
//! permission mode only ever narrows the saved setting (`AgentDef::launch_options`).

use crate::agent::{AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};
use crate::context::Context;
use crate::definitions::AdapterKind;
use crate::mcp;
use crate::model::Task;
use crate::{Error, Result, Workspace};

/// The variable cargo reads for where to build.
const CARGO_TARGET_DIR_ENV: &str = "CARGO_TARGET_DIR";

async fn workspace_of(ctx: &Context, task: &Task) -> Result<Workspace> {
    ctx.workspace(&ctx.repo(&task.repo).await?)
}

/// How a task's agent starts, before the brief, session id and folders are added.
pub(super) struct Launcher {
    pub adapter: Box<dyn AgentAdapter>,
    pub options: LaunchOptions,
    /// Claude gets a session id; a shell command does not.
    pub claude: bool,
}

/// The task's type as saved now. A type deleted since the task was created fails the start.
pub(super) async fn launcher(ctx: &Context, task: &Task) -> Result<Launcher> {
    let store = ctx.store()?;
    let Some(found) = store.agent_type(&task.agent).await? else {
        let names: Vec<String> = store
            .agent_types()
            .await?
            .into_iter()
            .map(|t| t.def.name)
            .collect();
        return Err(Error::Conflict(format!(
            "agent type `{}` was deleted after the task was created: create the task again with one of: {}",
            task.agent,
            names.join(", ")
        )));
    };
    let mut options = found
        .def
        .launch_options(ctx.settings().await?.permission_mode);
    let names: Vec<String> = store
        .agent_types()
        .await?
        .into_iter()
        .map(|t| t.def.name)
        .collect();
    let targets = found.def.hand_back_targets(&found.def.name, &names);
    let (adapter, claude): (Box<dyn AgentAdapter>, bool) = match found.def.adapter {
        AdapterKind::Claude => (Box::new(ClaudeCodeAdapter::new(ctx.claude_bin())), true),
        AdapterKind::Shell => (Box::new(ShellAdapter), false),
    };
    if ctx.settings().await?.shared_cargo_target {
        options.env.push((
            CARGO_TARGET_DIR_ENV.into(),
            workspace_of(ctx, task)
                .await?
                .cargo_target_dir()
                .to_string_lossy()
                .into_owned(),
        ));
    }
    if claude && !targets.is_empty() {
        options.mcp_config = Some(mcp::task_config_json(ctx.base_url(), &task.name));
        options.mcp_tools = mcp::task_allowed_tools();
        options.system_prompt = Some(with_hand_back(options.system_prompt, &targets));
    }
    Ok(Launcher {
        adapter,
        options,
        claude,
    })
}

/// The type's own prompt, then what the agent may hand back to.
fn with_hand_back(own: Option<String>, targets: &[String]) -> String {
    let line = format!(
        "you can hand back to: {}. when the work needs one of those types, call the hand_back tool with `to` and `reason`, then stop.",
        targets.join(", ")
    );
    match own.filter(|p| !p.trim().is_empty()) {
        Some(own) => format!("{own}\n\n{line}"),
        None => line,
    }
}
