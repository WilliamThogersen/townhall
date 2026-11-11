//! `ClaudeCodeAdapter`: the `claude` command line for one agent.
//!
//! `env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_ENTRYPOINT
//! -u CLAUDE_CODE_SSE_PORT -u CLAUDE_EFFORT claude [--model=M] [--effort=E] [--permission-mode=MODE | --dangerously-skip-permissions]
//! [--append-system-prompt=P] [--session-id=ID] [--tools=B1,B2 --allowedTools=T1,T2] [--add-dir=D ...]
//! --setting-sources= --settings=S --strict-mcp-config [--mcp-config=JSON] [-- PROMPT]`
//!
//! Every option is one argv element (`--name=value`) and every element is shell-quoted, so
//! no value can split or start a flag. The prompt is the only positional, after `--`.
//! claude still runs a subcommand when the first positional equals its name (`-- update`
//! is `claude update`), so a prompt without whitespace gets a trailing newline: no
//! subcommand name has one. `env -u` drops [`OUTER_SESSION_ENV`]: tmux starts the agent
//! with the tmux server's environment, which can hold them even when the daemon has not.

use std::path::Path;

use super::adapter::{env_words, shell_quote, shell_word, AgentAdapter, LaunchOptions};
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

pub const MCP_TOOL_TIMEOUT_ENV: &str = "MCP_TOOL_TIMEOUT";
const MCP_TOOL_TIMEOUT_MARGIN: std::time::Duration = std::time::Duration::from_secs(60);

pub fn mcp_tool_timeout_ms() -> String {
    (crate::mcp::ADVICE_TIMEOUT + MCP_TOOL_TIMEOUT_MARGIN)
        .as_millis()
        .to_string()
}

/// `env -u NAME` for each of [`OUTER_SESSION_ENV`], in front of the program.
fn without_outer_session() -> Vec<String> {
    let unset = OUTER_SESSION_ENV.iter().flat_map(|name| ["-u", name]);
    ["env"]
        .into_iter()
        .chain(unset)
        .map(String::from)
        .chain([format!("{MCP_TOOL_TIMEOUT_ENV}={}", mcp_tool_timeout_ms())])
        .collect()
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

fn effort(options: &LaunchOptions) -> Vec<String> {
    options
        .effort
        .map(|e| format!("--effort={}", e.as_str()))
        .into_iter()
        .collect()
}

/// `--permission-mode=<mode>`, or `--dangerously-skip-permissions` for `skip all`. An agent
/// with a tool list runs in `dontAsk` (`plan` stays): whatever the list does not allow is
/// denied, never asked about, and skip all cannot lift the list.
fn permission(options: &LaunchOptions, tools: &[&str]) -> Vec<String> {
    let mode = options.permission_mode;
    if !tools.is_empty() && mode != PermissionMode::Plan {
        return vec!["--permission-mode=dontAsk".into()];
    }
    match mode.cli_value() {
        Some(value) => vec![format!("--permission-mode={value}")],
        None => vec!["--dangerously-skip-permissions".into()],
    }
}

fn system_prompt(options: &LaunchOptions, tools: &[&str]) -> Vec<String> {
    let own = options
        .system_prompt
        .as_deref()
        .filter(|p| !p.trim().is_empty());
    let notice = (!tools.is_empty()).then(|| {
        format!(
            "these are the tools you have access to: {}. every other tool is denied.",
            tools.join(", ")
        )
    });
    match (own, notice) {
        (Some(p), Some(n)) => vec![format!("--append-system-prompt={p}\n\n{n}")],
        (Some(p), None) => vec![format!("--append-system-prompt={p}")],
        (None, Some(n)) => vec![format!("--append-system-prompt={n}")],
        (None, None) => vec![],
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

/// The checked tool entries: claude splits `--allowedTools` on commas, so none may hold one.
fn tool_entries(options: &LaunchOptions) -> Result<Vec<&str>> {
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
    Ok(names)
}

fn checked_entries<'a>(what: &str, list: &'a [String]) -> Result<Vec<&'a str>> {
    list.iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|tool| {
            plain(what, tool)?;
            match tool.contains(',') {
                true => Err(Error::InvalidInput(format!(
                    "{what} {tool:?} must not hold a comma"
                ))),
                false => Ok(tool),
            }
        })
        .collect()
}

/// A type with no tool list keeps every built-in tool, so its MCP tools and the extra
/// rules are only allowed.
fn only_allowed(listed: &[&str], allowed: &[&str]) -> Vec<String> {
    match listed.is_empty() && !allowed.is_empty() {
        true => vec![format!("--allowedTools={}", allowed.join(","))],
        false => vec![],
    }
}

fn isolation(options: &LaunchOptions) -> Result<Vec<String>> {
    let settings = match options.settings.as_deref().filter(|s| !s.is_empty()) {
        Some(path) => {
            plain("settings", path)?;
            format!("--settings={path}")
        }
        None => ISOLATED_SETTINGS.to_string(),
    };
    Ok(AGENT_ISOLATION
        .map(|flag| match flag == ISOLATED_SETTINGS {
            true => settings.clone(),
            false => flag.to_string(),
        })
        .to_vec())
}

fn mcp_config(options: &LaunchOptions) -> Result<Vec<String>> {
    match options.mcp_config.as_deref().filter(|c| !c.is_empty()) {
        Some(config) => {
            plain("mcp config", config).map(|()| vec![format!("--mcp-config={config}")])
        }
        None => Ok(vec![]),
    }
}

/// `--tools=` the built-in tools the entries name (`Bash(git diff:*)` is `Bash`; MCP tools
/// are no built-in), so no other built-in exists, then `--allowedTools=` the entries.
fn tools(entries: &[&str], extra: &[&str]) -> Vec<String> {
    if entries.is_empty() {
        return vec![];
    }
    let mut built_in: Vec<&str> = vec![];
    for entry in entries {
        let name = entry.split('(').next().unwrap_or(entry);
        if !name.starts_with("mcp__") && !built_in.contains(&name) {
            built_in.push(name);
        }
    }
    vec![
        format!("--tools={}", built_in.join(",")),
        format!(
            "--allowedTools={}",
            entries
                .iter()
                .chain(extra)
                .copied()
                .collect::<Vec<_>>()
                .join(",")
        ),
    ]
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
        let entries = tool_entries(options)?;
        let mcp = checked_entries("mcp tool", &options.mcp_tools)?;
        let extra = checked_entries("extra rule", &options.extra_allowed)?;
        let listed: Vec<&str> = match entries.is_empty() {
            true => vec![],
            false => entries.iter().chain(&mcp).copied().collect(),
        };
        let unlisted: Vec<&str> = mcp.iter().chain(&extra).copied().collect();
        Ok([
            model(options)?,
            effort(options),
            permission(options, &listed),
            system_prompt(options, &listed),
            session(options)?,
            tools(&listed, &extra),
            only_allowed(&listed, &unlisted),
            add_dirs(options)?,
            isolation(options)?,
            mcp_config(options)?,
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
            .chain(env_words(options)?)
            .chain([shell_word(&self.bin)])
            .chain(Self::args(options)?.iter().map(|a| shell_quote(a)))
            .collect();
        Ok(words.join(" "))
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
