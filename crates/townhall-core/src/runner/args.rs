//! The argv of one turn of townhall's own session: `claude -p` with stream-json in and
//! out, like Hive's `chatArgs`. The message goes in on stdin, so no flag carries it.
//! Every option and its value is one element (`--name=value`), as in `agent/claude.rs`,
//! and no value but the system prompt may start with `-` or hold control characters.

use std::path::PathBuf;

use crate::agent::{add_dir, plain, ISOLATED_SETTINGS};
use crate::model::TownhallModel;
use crate::{mcp, Result};

/// Tools townhall's session may use besides its own: reading code to plan, nothing more.
/// They are also the whole built-in set (`--tools`), so no other built-in tool exists.
pub const READ_ONLY_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];
/// Tools that write or run things. Denied outright, so a user's own allow rules in
/// `~/.claude/settings.json` cannot let townhall's session write to a repo.
pub const DENIED_TOOLS: [&str; 5] = ["Bash", "Edit", "MultiEdit", "Write", "NotebookEdit"];
/// Keeps the user's own Claude setup out of townhall's session: no settings files (so no
/// hooks, plugins, permission rules or CLAUDE.md from them), hooks and auto-memory off,
/// and no skills or slash commands. Auth is untouched: it comes from the
/// login (keychain or credentials file) and the environment, never from these.
pub const ISOLATION: [&str; 3] = [
    "--setting-sources=",
    ISOLATED_SETTINGS,
    "--disable-slash-commands",
];

/// Every tool the session may use: townhall's MCP tools, then the read-only ones.
pub fn allowed_tools() -> Vec<String> {
    mcp::allowed_tools()
        .into_iter()
        .chain(READ_ONLY_TOOLS.iter().map(|t| t.to_string()))
        .collect()
}

pub fn is_allowed(tool: &str) -> bool {
    allowed_tools().iter().any(|t| t == tool)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnArgs {
    pub session_id: String,
    /// `--resume` once the session started, `--session-id` before.
    pub resume: bool,
    pub system_prompt: String,
    pub mcp_config: String,
    pub add_dirs: Vec<PathBuf>,
    /// The `townhallModel` setting.
    pub model: TownhallModel,
}

const FIXED: [&str; 6] = [
    "-p",
    "--output-format=stream-json",
    "--verbose",
    "--input-format=stream-json",
    "--permission-prompt-tool=stdio",
    "--include-partial-messages",
];

fn session(a: &TurnArgs) -> Result<String> {
    plain("session id", &a.session_id)?;
    let flag = if a.resume { "--resume" } else { "--session-id" };
    Ok(format!("{flag}={}", a.session_id))
}

/// The options in a fixed order, one element each, the `--add-dir`s last.
pub fn build(a: &TurnArgs) -> Result<Vec<String>> {
    plain("mcp config", &a.mcp_config)?;
    let options = [
        session(a)?,
        format!("--model={}", a.model.cli_value()),
        "--permission-mode=default".into(),
        format!("--append-system-prompt={}", a.system_prompt),
        "--strict-mcp-config".into(),
        format!("--mcp-config={}", a.mcp_config),
        format!("--tools={}", READ_ONLY_TOOLS.join(",")),
        format!("--allowedTools={}", allowed_tools().join(",")),
        format!("--disallowedTools={}", DENIED_TOOLS.join(",")),
    ];
    let isolation = ISOLATION.iter().map(|s| s.to_string());
    let dirs = a
        .add_dirs
        .iter()
        .map(|d| add_dir(d))
        .collect::<Result<Vec<_>>>()?;
    Ok(FIXED
        .iter()
        .map(|s| s.to_string())
        .chain(options)
        .chain(isolation)
        .chain(dirs)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(resume: bool, add_dirs: Vec<PathBuf>) -> TurnArgs {
        TurnArgs {
            session_id: "s-1".into(),
            resume,
            system_prompt: "plan".into(),
            mcp_config: "{}".into(),
            add_dirs,
            model: TownhallModel::default(),
        }
    }

    #[test]
    fn first_and_later_turns() {
        let first = build(&args(false, vec!["/r/b".into(), "/r/c".into()])).unwrap();
        assert_eq!(
            first,
            [
                "-p",
                "--output-format=stream-json",
                "--verbose",
                "--input-format=stream-json",
                "--permission-prompt-tool=stdio",
                "--include-partial-messages",
                "--session-id=s-1",
                "--model=sonnet",
                "--permission-mode=default",
                "--append-system-prompt=plan",
                "--strict-mcp-config",
                "--mcp-config={}",
                "--tools=Read,Grep,Glob",
                "--allowedTools=mcp__townhall__create_task,mcp__townhall__set_title,mcp__townhall__task_status,mcp__townhall__list_agent_types,mcp__townhall__read_agent_type,mcp__townhall__save_agent_type,mcp__townhall__read_logs,mcp__townhall__log_summary,mcp__townhall__conversation_insights,Read,Grep,Glob",
                "--disallowedTools=Bash,Edit,MultiEdit,Write,NotebookEdit",
                "--setting-sources=",
                "--settings={\"disableAllHooks\":true,\"autoMemoryEnabled\":false}",
                "--disable-slash-commands",
                "--add-dir=/r/b",
                "--add-dir=/r/c",
            ]
        );
        let later = build(&args(true, vec![])).unwrap();
        assert_eq!(later[6], "--resume=s-1");
        assert!(!later.iter().any(|a| a.starts_with("--add-dir")));
        assert!(!later.iter().any(|a| a.contains("dangerously")));
        assert!(!first.iter().any(|a| a == "--bare" || a == "--safe-mode"));
    }

    #[test]
    fn the_model_follows_the_setting() {
        for (model, flag) in [
            (TownhallModel::Sonnet, "--model=sonnet"),
            (TownhallModel::Opus, "--model=opus"),
            (TownhallModel::Haiku, "--model=haiku"),
        ] {
            let argv = build(&TurnArgs {
                model,
                ..args(true, vec![])
            })
            .unwrap();
            let models: Vec<&String> = argv.iter().filter(|a| a.starts_with("--model")).collect();
            assert_eq!(models, [flag]);
        }
    }

    #[test]
    fn values_that_look_like_flags_are_refused() {
        let refused = |change: &dyn Fn(&mut TurnArgs)| {
            let mut a = args(false, vec![]);
            change(&mut a);
            build(&a).unwrap_err().to_string()
        };
        for id in ["-s", "--dangerously-skip-permissions", "s\n1"] {
            let error = refused(&|a| a.session_id = id.into());
            assert!(error.contains("session id"), "{error}");
        }
        for config in ["--strict-mcp-config", "{}\u{7}", "{\n}"] {
            let error = refused(&|a| a.mcp_config = config.into());
            assert!(error.contains("mcp config"), "{error}");
        }
        for dir in ["-r", "relative/dir", "/r/\nb"] {
            let error = refused(&|a| a.add_dirs = vec![dir.into()]);
            assert!(error.contains("folder"), "{error}");
        }
    }

    #[test]
    fn the_system_prompt_stays_inside_its_own_element() {
        let mut a = args(false, vec![]);
        a.system_prompt = "--resume\n- read first\n- then plan".into();
        let argv = build(&a).unwrap();
        assert!(argv.contains(&"--append-system-prompt=--resume\n- read first\n- then plan".into()));
        assert!(argv[1..].iter().all(|a| a.starts_with("--")), "{argv:?}");
    }

    #[test]
    fn only_listed_tools_are_allowed() {
        assert!(is_allowed("mcp__townhall__create_task") && is_allowed("Grep"));
        assert!(!is_allowed("Edit") && !is_allowed("Bash") && !is_allowed("mcp__other__x"));
    }
}
