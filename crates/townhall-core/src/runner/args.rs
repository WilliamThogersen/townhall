//! The argv of one turn of townhall's own session: `claude -p` with stream-json in and
//! out, like Hive's `chatArgs`. The message goes in on stdin, so no flag carries it.
//! Every option and its value is one element (`--name=value`), as in `agent/claude.rs`,
//! and no value but the system prompt may start with `-` or hold control characters.

use std::path::PathBuf;

use crate::agent::{add_dir, plain, ISOLATED_SETTINGS};
use crate::model::PermissionMode;
use crate::{mcp, Error, Result};

/// Tools townhall's session may use besides its own: reading code to plan, nothing more.
/// They are also the whole built-in set (`--tools`), so no other built-in tool exists.
pub const READ_ONLY_TOOLS: [&str; 5] = ["Read", "Grep", "Glob", "WebFetch", "WebSearch"];
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
    /// The `--model` value.
    pub model: String,
    pub permission_mode: PermissionMode,
    /// Every tool the session may use without asking; the built-in ones among them are
    /// also the whole built-in set (`--tools`).
    pub tools: Vec<String>,
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

fn permission(a: &TurnArgs) -> Result<String> {
    match a.permission_mode.cli_value() {
        Some(value) => Ok(format!("--permission-mode={value}")),
        None => Err(Error::InvalidInput(
            "the permission mode of townhall's session has no flag".into(),
        )),
    }
}

/// The built-in tools the entries name (`Bash(git log:*)` is `Bash`; MCP tools are none).
fn built_in(tools: &[String]) -> Vec<&str> {
    let mut names: Vec<&str> = vec![];
    for tool in tools {
        let name = tool.split('(').next().unwrap_or(tool);
        if !name.starts_with("mcp__") && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The options in a fixed order, one element each, the `--add-dir`s last.
pub fn build(a: &TurnArgs) -> Result<Vec<String>> {
    plain("mcp config", &a.mcp_config)?;
    plain("model", &a.model)?;
    for tool in &a.tools {
        plain("tool", tool)?;
    }
    let options = [
        session(a)?,
        format!("--model={}", a.model),
        permission(a)?,
        format!("--append-system-prompt={}", a.system_prompt),
        "--strict-mcp-config".into(),
        format!("--mcp-config={}", a.mcp_config),
        format!("--tools={}", built_in(&a.tools).join(",")),
        format!("--allowedTools={}", a.tools.join(",")),
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
            model: "sonnet".into(),
            permission_mode: PermissionMode::Default,
            tools: allowed_tools(),
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
                "--tools=Read,Grep,Glob,WebFetch,WebSearch",
                "--allowedTools=mcp__townhall__create_task,mcp__townhall__set_title,mcp__townhall__task_status,mcp__townhall__list_agent_types,mcp__townhall__read_agent_type,mcp__townhall__save_agent_type,mcp__townhall__read_logs,mcp__townhall__log_summary,mcp__townhall__conversation_insights,Read,Grep,Glob,WebFetch,WebSearch",
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
    fn the_model_follows_the_persona() {
        for model in ["sonnet", "opus", "haiku", "claude-opus-4-1"] {
            let argv = build(&TurnArgs {
                model: model.into(),
                ..args(true, vec![])
            })
            .unwrap();
            let models: Vec<&String> = argv.iter().filter(|a| a.starts_with("--model")).collect();
            assert_eq!(models, [&format!("--model={model}")]);
        }
    }

    #[test]
    fn the_mode_and_tools_follow_the_persona() {
        let argv = build(&TurnArgs {
            permission_mode: PermissionMode::Plan,
            tools: vec![
                "Read".into(),
                "Grep".into(),
                "mcp__townhall__create_task".into(),
            ],
            ..args(true, vec![])
        })
        .unwrap();
        for flag in [
            "--permission-mode=plan",
            "--tools=Read,Grep",
            "--allowedTools=Read,Grep,mcp__townhall__create_task",
            "--disallowedTools=Bash,Edit,MultiEdit,Write,NotebookEdit",
        ] {
            assert!(argv.contains(&flag.to_string()), "{flag}: {argv:?}");
        }
        assert!(!argv.contains(&"--permission-mode=default".to_string()));
    }

    #[test]
    fn a_mode_without_a_flag_is_refused() {
        let error = build(&TurnArgs {
            permission_mode: PermissionMode::BypassPermissions,
            ..args(true, vec![])
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("permission mode"), "{error}");
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
        for model in ["-p", "opus\n"] {
            let error = refused(&|a| a.model = model.into());
            assert!(error.contains("model"), "{error}");
        }
        for tool in ["--bare", "Read\n"] {
            let error = refused(&|a| a.tools = vec![tool.into()]);
            assert!(error.contains("tool"), "{error}");
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
