//! The agent type a task runs as (2.12): its adapter and launch options. The type's
//! permission mode only ever narrows the saved setting (`AgentDef::launch_options`).

use crate::agent::{AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};
use crate::context::Context;
use crate::definitions::AdapterKind;
use crate::model::Task;
use crate::{Error, Result};

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
    let options = found
        .def
        .launch_options(ctx.settings().await?.permission_mode);
    let (adapter, claude): (Box<dyn AgentAdapter>, bool) = match found.def.adapter {
        AdapterKind::Claude => (Box::new(ClaudeCodeAdapter::new(ctx.claude_bin())), true),
        AdapterKind::Shell => (Box::new(ShellAdapter), false),
    };
    Ok(Launcher {
        adapter,
        options,
        claude,
    })
}
