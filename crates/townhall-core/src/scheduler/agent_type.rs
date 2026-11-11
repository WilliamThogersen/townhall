//! The agent type a task runs as (2.12): its adapter and launch options. The type's
//! permission mode is the task's; a type with none runs in `acceptEdits`.

use std::path::Path;

use crate::agent::{AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};
use crate::context::Context;
use crate::definitions::AdapterKind;
use crate::mcp;
use crate::model::{AgentType, Task, ADVISER_AGENT_TYPE};
use crate::plugins::{agent_types, claude_hooks, claude_mcp};
use crate::{Error, Result};

use super::brief;

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
        let why = match agent_types::owner_of(store, &task.agent).await? {
            Some(plugin) if plugin.enabled => {
                format!(
                    "comes from plugin {}, which no longer provides it",
                    plugin.id
                )
            }
            Some(plugin) => format!("comes from plugin {}, which is disabled", plugin.id),
            None => "was deleted after the task was created".to_string(),
        };
        return Err(Error::Conflict(format!(
            "agent type `{}` {}: create the task again with one of: {}",
            task.agent,
            why,
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
    let advises = found.def.name.eq_ignore_ascii_case(ADVISER_AGENT_TYPE);
    if claude && !targets.is_empty() {
        options.mcp_config = Some(mcp::task_config_json(ctx.base_url(), &task.name));
        options.mcp_tools = mcp::task_allowed_tools();
        options.system_prompt = Some(with_rules(options.system_prompt, &targets, !advises));
    }
    if claude {
        let mut chosen = found.def.plugins.clone();
        chosen.extend(found.plugin.clone());
        let parts = claude_mcp::selected(store, &chosen).await?;
        if !parts.is_empty() {
            let config = claude_mcp::merge(options.mcp_config.as_deref(), &parts);
            let path = brief::write_mcp(ctx.home(), &task.name, config).await?;
            options.mcp_config = Some(path.display().to_string());
            options
                .mcp_tools
                .extend(parts.into_iter().flat_map(|p| p.tools));
        }
        let hooks = claude_hooks::selected(store, &chosen).await?;
        if let Some(settings) = claude_hooks::merge(&hooks) {
            let path = brief::write_settings(ctx.home(), &task.name, settings).await?;
            options.settings = Some(path.display().to_string());
        }
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

const ASK_ADVISER_RULE: &str = "before you stop to ask the user, ask the adviser: call the ask_adviser tool with your question and what it needs to know. ask the user only when the adviser's answer says it needs the user (needsUser is true).";

fn with_rules(own: Option<String>, targets: &[String], ask_adviser: bool) -> String {
    let hand_back = (!targets.is_empty()).then(|| {
        format!(
            "you can hand back to: {}. when the work needs one of those types, call the hand_back tool with `to` and `reason`, then stop.",
            targets.join(", ")
        )
    });
    let adviser = ask_adviser.then(|| ASK_ADVISER_RULE.to_string());
    let rules = hand_back
        .into_iter()
        .chain(adviser)
        .collect::<Vec<_>>()
        .join(" ");
    let own = own.filter(|p| !p.trim().is_empty());
    match (own, rules.is_empty()) {
        (Some(own), false) => format!("{own}\n\n{rules}"),
        (Some(own), true) => own,
        (None, _) => rules,
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
    fn the_rules_follow_the_type_prompt_and_tell_agents_to_ask_the_adviser_first() {
        let targets = ["tester".to_string()];
        let rules = with_rules(Some("be kind".into()), &targets, true);
        let parts: Vec<&str> = rules.split("\n\n").collect();
        assert_eq!(parts[0], "be kind");
        assert!(parts[1].starts_with("you can hand back to: tester."));
        assert!(parts[1].contains(" before you stop to ask the user, ask the adviser"));
        assert!(parts[1].ends_with("(needsUser is true)."));
        assert_eq!(with_rules(None, &[], true), ASK_ADVISER_RULE);
        assert_eq!(
            with_rules(None, &targets, false).matches("adviser").count(),
            0
        );
        assert_eq!(with_rules(None, &targets, true).lines().count(), 1);
        assert_eq!(with_rules(Some("be kind".into()), &[], false), "be kind");
        assert_eq!(with_rules(Some("  ".into()), &[], false), "");
    }

    #[test]
    fn ask_adviser_is_in_the_task_allow_list() {
        assert!(mcp::task_allowed_tools().contains(&"mcp__townhall__ask_adviser".to_string()));
    }

    #[test]
    fn the_hand_back_tools_are_always_in_the_task_allow_list() {
        let tools = mcp::task_allowed_tools();
        assert!(tools.contains(&"mcp__townhall__hand_back".to_string()));
        assert!(tools.iter().all(|t| t.starts_with("mcp__townhall__")));
    }
}
