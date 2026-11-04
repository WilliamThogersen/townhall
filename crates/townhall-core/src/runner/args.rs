//! The argv of one turn of townhall's own session: `claude -p` with stream-json in and
//! out, like Hive's `chatArgs`. The message goes in on stdin, so no flag carries it.

use std::path::PathBuf;

use crate::mcp;

/// Tools townhall's session may use besides its own: reading code to plan, nothing more.
pub const READ_ONLY_TOOLS: [&str; 4] = ["Read", "Grep", "Glob", "LS"];
/// Tools that write or run things. Denied outright, so a user's own allow rules in
/// `~/.claude/settings.json` cannot let townhall's session write to a repo.
pub const DENIED_TOOLS: [&str; 5] = ["Bash", "Edit", "MultiEdit", "Write", "NotebookEdit"];

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
}

fn session(a: &TurnArgs) -> [String; 2] {
    let flag = if a.resume { "--resume" } else { "--session-id" };
    [flag.to_string(), a.session_id.clone()]
}

/// The flags in a fixed order; the ones that take several values go last.
pub fn build(a: &TurnArgs) -> Vec<String> {
    let fixed = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--input-format",
        "stream-json",
        "--permission-prompt-tool",
        "stdio",
        "--include-partial-messages",
    ];
    let mut argv: Vec<String> = fixed.iter().map(|s| s.to_string()).collect();
    argv.extend(session(a));
    argv.extend(["--permission-mode".into(), "default".into()]);
    argv.extend(["--append-system-prompt".into(), a.system_prompt.clone()]);
    argv.extend(["--strict-mcp-config".into(), "--mcp-config".into()]);
    argv.push(a.mcp_config.clone());
    argv.push("--allowedTools".into());
    argv.extend(allowed_tools());
    argv.push("--disallowedTools".into());
    argv.extend(DENIED_TOOLS.iter().map(|t| t.to_string()));
    if !a.add_dirs.is_empty() {
        argv.push("--add-dir".into());
        argv.extend(a.add_dirs.iter().map(|d| d.to_string_lossy().into_owned()));
    }
    argv
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
        }
    }

    #[test]
    fn first_and_later_turns() {
        let first = build(&args(false, vec!["/r/b".into()]));
        assert_eq!(
            first[9..],
            [
                "--session-id",
                "s-1",
                "--permission-mode",
                "default",
                "--append-system-prompt",
                "plan",
                "--strict-mcp-config",
                "--mcp-config",
                "{}",
                "--allowedTools",
                "mcp__townhall__create_task",
                "mcp__townhall__set_title",
                "Read",
                "Grep",
                "Glob",
                "LS",
                "--disallowedTools",
                "Bash",
                "Edit",
                "MultiEdit",
                "Write",
                "NotebookEdit",
                "--add-dir",
                "/r/b"
            ]
        );
        let later = build(&args(true, vec![]));
        assert_eq!(later[9..11], ["--resume", "s-1"]);
        assert!(!later.contains(&"--add-dir".to_string()));
        assert!(!later.iter().any(|a| a.contains("dangerously")));
    }

    #[test]
    fn only_listed_tools_are_allowed() {
        assert!(is_allowed("mcp__townhall__create_task") && is_allowed("Grep"));
        assert!(!is_allowed("Edit") && !is_allowed("Bash") && !is_allowed("mcp__other__x"));
    }
}
