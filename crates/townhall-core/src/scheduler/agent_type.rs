//! The agent type a task runs as (2.12): its adapter and launch options. The type's
//! permission mode is the task's; a type with none runs in `acceptEdits`.

use std::path::Path;

use crate::agent::{AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};
use crate::context::Context;
use crate::definitions::AdapterKind;
use crate::mcp;
use crate::model::{AgentType, Task};
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
    let mut options = found.def.launch_options();
    let names: Vec<String> = store
        .agent_types()
        .await?
        .into_iter()
        .filter(AgentType::for_tasks)
        .map(|t| t.def.name)
        .collect();
    let targets = found.def.hand_back_targets(&found.def.name, &names);
    let (adapter, claude): (Box<dyn AgentAdapter>, bool) = match found.def.adapter {
        AdapterKind::Claude => (Box::new(ClaudeCodeAdapter::new(ctx.claude_bin())), true),
        AdapterKind::Shell => (Box::new(ShellAdapter), false),
    };
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

/// Rules every claude agent gets besides its type's: `cd` into its own worktree (and
/// `pwd`), and reading the plan files claude keeps under `~/.claude/plans`. Read only:
/// nothing here writes, runs another program or reaches outside those two places.
pub(super) fn own_rules(worktree: &Path) -> Vec<String> {
    let dir = worktree.display();
    vec![
        format!("Bash(cd {dir})"),
        format!("Bash(cd {dir}/*)"),
        "Bash(pwd)".into(),
        "Read(~/.claude/plans/*)".into(),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_may_cd_in_its_worktree_and_read_its_plans_and_no_more() {
        let rules = own_rules(Path::new("/h/worktrees/t1/fix"));
        assert_eq!(
            rules,
            [
                "Bash(cd /h/worktrees/t1/fix)",
                "Bash(cd /h/worktrees/t1/fix/*)",
                "Bash(pwd)",
                "Read(~/.claude/plans/*)",
            ]
        );
        for rule in &rules {
            let wide = [
                "Bash(*", "Bash)", "Write", "Edit", "WebFetch", "Read(*", "Read(/",
            ];
            assert!(!wide.iter().any(|w| rule.starts_with(w)), "{rule}");
            assert!(!rule.contains("rm") && !rule.contains("git"), "{rule}");
        }
    }

    #[test]
    fn the_hand_back_tools_are_always_in_the_task_allow_list() {
        let tools = mcp::task_allowed_tools();
        assert!(tools.contains(&"mcp__townhall__hand_back".to_string()));
        assert!(tools.iter().all(|t| t.starts_with("mcp__townhall__")));
    }
}
