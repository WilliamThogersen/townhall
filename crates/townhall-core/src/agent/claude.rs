//! `ClaudeCodeAdapter`: the `claude` command line for one agent.
//!
//! `env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_ENTRYPOINT
//! -u CLAUDE_CODE_SSE_PORT -u CLAUDE_EFFORT claude [--model=M] [--permission-mode=MODE | --dangerously-skip-permissions]
//! [--append-system-prompt=P] [--session-id=ID] [--allowedTools=T1,T2] [--add-dir=D ...]
//! --setting-sources= --settings=S --strict-mcp-config [-- PROMPT]`
//!
//! Every option is one argv element (`--name=value`) and every element is shell-quoted, so
//! no value can split or start a flag. The prompt is the only positional, after `--`.
//! claude still runs a subcommand when the first positional equals its name (`-- update`
//! is `claude update`), so a prompt without whitespace gets a trailing newline: no
//! subcommand name has one. `env -u` drops [`OUTER_SESSION_ENV`]: tmux starts the agent
//! with the tmux server's environment, which can hold them even when the daemon has not.

use std::path::Path;

use super::adapter::{shell_quote, shell_word, AgentAdapter, LaunchOptions};
use crate::ids::is_session_id;
use crate::model::PermissionMode;
use crate::{Error, Result};

pub const DEFAULT_CLAUDE_BIN: &str = "claude";
const END_OF_OPTIONS: &str = "--";

/// Settings for every claude townhall starts: no hooks, and no auto-memory (`MEMORY.md`),
/// which loads even when no settings source does.
pub const ISOLATED_SETTINGS: &str =
    "--settings={\"disableAllHooks\":true,\"autoMemoryEnabled\":false}";

/// An agent runs only on what townhall gives it: no settings files (so no hooks, plugins,
/// status line, permission rules, CLAUDE.md or rules from the user or the repo), no
/// auto-memory, and no MCP server, since none comes with `--mcp-config`. Built-in tools,
/// skills and slash commands stay. Auth is untouched: it comes from the login and the
/// environment, never from these.
pub const AGENT_ISOLATION: [&str; 3] = [
    "--setting-sources=",
    ISOLATED_SETTINGS,
    "--strict-mcp-config",
];

/// What a running Claude Code sets for the programs it starts. A claude that inherits them
/// runs as part of that outer session (`CLAUDE_EFFORT` even sets its effort), so no claude
/// townhall starts gets them. Other `CLAUDE_*` and `ANTHROPIC_*` variables stay: they can
/// carry the user's auth or provider setup.
pub const OUTER_SESSION_ENV: [&str; 5] = [
    "CLAUDECODE",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_EFFORT",
];

/// `env -u NAME` for each of [`OUTER_SESSION_ENV`], in front of the program.
fn without_outer_session() -> Vec<String> {
    let unset = OUTER_SESSION_ENV.iter().flat_map(|name| ["-u", name]);
    ["env"].into_iter().chain(unset).map(String::from).collect()
}

/// Launches the `claude` CLI; see the module docs for the exact argv.
#[derive(Debug, Clone)]
pub struct ClaudeCodeAdapter {
    bin: String,
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_CLAUDE_BIN)
    }
}

/// Values that sit next to flags must never look like one.
pub(crate) fn plain(what: &str, value: &str) -> Result<()> {
    if value.starts_with('-') || value.chars().any(char::is_control) {
        return Err(Error::InvalidInput(format!(
            "{what} {value:?} must not start with `-` or hold control characters"
        )));
    }
    Ok(())
}

fn model(options: &LaunchOptions) -> Result<Vec<String>> {
    match options
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        Some(m) => plain("model", m).map(|()| vec![format!("--model={m}")]),
        None => Ok(vec![]),
    }
}

/// `--permission-mode=<mode>`, or `--dangerously-skip-permissions` for `skip all`.
fn permission(mode: PermissionMode) -> Vec<String> {
    match mode.cli_value() {
        Some(value) => vec![format!("--permission-mode={value}")],
        None => vec!["--dangerously-skip-permissions".into()],
    }
}

fn system_prompt(options: &LaunchOptions) -> Vec<String> {
    match options
        .system_prompt
        .as_deref()
        .filter(|p| !p.trim().is_empty())
    {
        Some(p) => vec![format!("--append-system-prompt={p}")],
        None => vec![],
    }
}

fn session(options: &LaunchOptions) -> Result<Vec<String>> {
    match options.session_id.as_deref() {
        Some(id) => {
            plain("session id", id)?;
            if !is_session_id(id) {
                return Err(Error::InvalidInput(format!(
                    "session id {id:?} is not a UUID"
                )));
            }
            Ok(vec![format!("--session-id={id}")])
        }
        None => Ok(vec![]),
    }
}

/// One `--allowedTools=A,B` element: claude splits it on commas, so no entry may hold one.
fn tools(options: &LaunchOptions) -> Result<Vec<String>> {
    let mut names = vec![];
    for tool in options
        .allowed_tools
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
    {
        plain("tool", tool)?;
        if tool.contains(',') {
            return Err(Error::InvalidInput(format!(
                "tool {tool:?} must not hold a comma"
            )));
        }
        names.push(tool);
    }
    Ok(match names.is_empty() {
        true => vec![],
        false => vec![format!("--allowedTools={}", names.join(","))],
    })
}

/// `--add-dir=<folder>`: absolute, UTF-8, never option-like.
pub(crate) fn add_dir(dir: &Path) -> Result<String> {
    let text = dir
        .to_str()
        .ok_or_else(|| Error::InvalidInput(format!("folder {dir:?} is not UTF-8")))?;
    plain("folder", text)?;
    if !dir.is_absolute() {
        return Err(Error::InvalidInput(format!(
            "folder {text:?} must be an absolute path"
        )));
    }
    Ok(format!("--add-dir={text}"))
}

/// One `--add-dir=<folder>` per folder.
fn add_dirs(options: &LaunchOptions) -> Result<Vec<String>> {
    options.add_dirs.iter().map(|dir| add_dir(dir)).collect()
}

fn prompt(options: &LaunchOptions) -> Vec<String> {
    match options.prompt.as_deref().filter(|p| !p.is_empty()) {
        Some(p) if p.chars().any(char::is_whitespace) => vec![END_OF_OPTIONS.into(), p.into()],
        Some(p) => vec![END_OF_OPTIONS.into(), format!("{p}\n")],
        None => vec![],
    }
}

impl ClaudeCodeAdapter {
    /// `bin` is the program to run, `claude` unless `TOWNHALL_CLAUDE_BIN` points elsewhere.
    pub fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }

    /// The argv after the program, unquoted: for callers that spawn claude without a shell.
    pub fn args(options: &LaunchOptions) -> Result<Vec<String>> {
        Ok([
            model(options)?,
            permission(options.permission_mode),
            system_prompt(options),
            session(options)?,
            tools(options)?,
            add_dirs(options)?,
            AGENT_ISOLATION.map(String::from).to_vec(),
            prompt(options),
        ]
        .concat())
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn kind(&self) -> &'static str {
        "claude"
    }

    fn launch_command(&self, options: &LaunchOptions) -> Result<String> {
        let words: Vec<String> = without_outer_session()
            .into_iter()
            .chain([shell_word(&self.bin)])
            .chain(Self::args(options)?.iter().map(|a| shell_quote(a)))
            .collect();
        Ok(words.join(" "))
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
