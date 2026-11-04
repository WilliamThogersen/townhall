use crate::ids::is_session_id;
use crate::model::PermissionMode;
use crate::{Error, Result};

/// Per-launch options handed to an adapter.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub prompt: Option<String>,
    /// Raw command, used by adapters that run arbitrary programs.
    pub command: Option<String>,
    /// Always the saved setting (`Context::settings`), never a value from a request body.
    pub permission_mode: PermissionMode,
    /// `claude --session-id`, a UUID. `None` lets claude pick one.
    pub session_id: Option<String>,
    /// `claude --model`, for example `sonnet` or `opus`.
    pub model: Option<String>,
    /// `claude --allowedTools`, for example `Read` or `Bash(git diff:*)`.
    pub allowed_tools: Vec<String>,
    /// `claude --append-system-prompt`.
    pub system_prompt: Option<String>,
}

/// Knows how to turn launch options into the shell command run inside tmux.
pub trait AgentAdapter: Send + Sync {
    fn kind(&self) -> &'static str;
    fn launch_command(&self, options: &LaunchOptions) -> Result<String>;
}

/// Quote `s` so a POSIX shell treats it as one literal word.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// `s` as is when a shell reads it as one plain word, else quoted.
fn shell_word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-+@%:,=".contains(c));
    if plain {
        s.to_string()
    } else {
        shell_quote(s)
    }
}

pub const DEFAULT_CLAUDE_BIN: &str = "claude";

/// Launches the `claude` CLI with the permission mode, an optional session id and prompt.
#[derive(Debug, Clone)]
pub struct ClaudeCodeAdapter {
    bin: String,
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_CLAUDE_BIN)
    }
}

impl ClaudeCodeAdapter {
    /// `bin` is the program to run, `claude` unless `TOWNHALL_CLAUDE_BIN` points elsewhere.
    pub fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }
}

fn flag(name: &str, value: &str) -> [String; 2] {
    [name.to_string(), shell_quote(value)]
}

impl ClaudeCodeAdapter {
    fn session_words(options: &LaunchOptions) -> Result<Vec<String>> {
        match options.session_id.as_deref() {
            Some(id) if is_session_id(id) => Ok(flag("--session-id", id).into()),
            Some(id) => Err(Error::InvalidInput(format!(
                "session id `{id}` is not a UUID"
            ))),
            None => Ok(vec![]),
        }
    }
}

/// The permission flag: `--permission-mode <mode>`, or `--dangerously-skip-permissions`
/// for `skip all`.
fn permission_words(mode: PermissionMode) -> Vec<String> {
    match mode.cli_value() {
        Some(value) => flag("--permission-mode", value).into(),
        None => vec!["--dangerously-skip-permissions".to_string()],
    }
}

fn optional(name: &str, value: Option<&str>) -> Vec<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map_or_else(Vec::new, |v| flag(name, v).into())
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn kind(&self) -> &'static str {
        "claude"
    }

    /// `claude [--model m] <permission> [--append-system-prompt p] [--session-id id] [prompt]
    /// [--allowedTools t ...]`. Every value is quoted. `--allowedTools` takes several values,
    /// so it goes last where it cannot swallow the prompt.
    fn launch_command(&self, options: &LaunchOptions) -> Result<String> {
        let prompt = options.prompt.as_deref().filter(|p| !p.is_empty());
        let tools: Vec<String> = options
            .allowed_tools
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(shell_quote)
            .collect();
        let tools = match tools.is_empty() {
            true => vec![],
            false => [vec!["--allowedTools".to_string()], tools].concat(),
        };
        let words: Vec<String> = [vec![shell_word(&self.bin)]]
            .into_iter()
            .chain([optional("--model", options.model.as_deref())])
            .chain([permission_words(options.permission_mode)])
            .chain([optional(
                "--append-system-prompt",
                options.system_prompt.as_deref(),
            )])
            .chain([Self::session_words(options)?])
            .chain([prompt.map(shell_quote).into_iter().collect()])
            .chain([tools])
            .flatten()
            .collect();
        Ok(words.join(" "))
    }
}

/// Runs an arbitrary command. Mostly useful for tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShellAdapter;

impl AgentAdapter for ShellAdapter {
    fn kind(&self) -> &'static str {
        "shell"
    }

    fn launch_command(&self, options: &LaunchOptions) -> Result<String> {
        match options.command.as_deref().map(str::trim) {
            Some(c) if !c.is_empty() => Ok(c.to_string()),
            _ => Err(Error::InvalidInput(
                "shell adapter requires a non-empty `command`".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude(options: &LaunchOptions) -> String {
        ClaudeCodeAdapter::default()
            .launch_command(options)
            .unwrap()
    }

    #[test]
    fn claude_quotes_prompt() {
        let o = LaunchOptions {
            prompt: Some("it's $HOME".into()),
            ..LaunchOptions::default()
        };
        assert_eq!(
            claude(&o),
            "claude --permission-mode 'acceptEdits' 'it'\\''s $HOME'"
        );
    }

    #[test]
    fn claude_without_prompt_defaults_to_accept_edits() {
        assert_eq!(
            claude(&LaunchOptions::default()),
            "claude --permission-mode 'acceptEdits'"
        );
    }

    #[test]
    fn skip_all_maps_to_the_dangerous_flag() {
        let o = LaunchOptions {
            permission_mode: PermissionMode::BypassPermissions,
            ..LaunchOptions::default()
        };
        assert_eq!(claude(&o), "claude --dangerously-skip-permissions");
        let plan = LaunchOptions {
            permission_mode: PermissionMode::Plan,
            ..LaunchOptions::default()
        };
        assert_eq!(claude(&plan), "claude --permission-mode 'plan'");
    }

    #[test]
    fn session_id_must_be_a_uuid() {
        let id = "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b";
        let o = LaunchOptions {
            session_id: Some(id.into()),
            prompt: Some("go".into()),
            ..LaunchOptions::default()
        };
        assert_eq!(
            claude(&o),
            format!("claude --permission-mode 'acceptEdits' --session-id '{id}' 'go'")
        );
        let bad = LaunchOptions {
            session_id: Some("x; rm -rf /".into()),
            ..LaunchOptions::default()
        };
        assert!(ClaudeCodeAdapter::default().launch_command(&bad).is_err());
    }

    #[test]
    fn every_flag_is_quoted_and_tools_go_last() {
        let o = LaunchOptions {
            prompt: Some("fix it".into()),
            model: Some("claude-opus's".into()),
            system_prompt: Some("you test first;\nthen $COMMIT".into()),
            allowed_tools: vec!["Read".into(), "Bash(git diff:*)".into(), "  ".into()],
            permission_mode: PermissionMode::Plan,
            ..LaunchOptions::default()
        };
        assert_eq!(
            claude(&o),
            "claude --model 'claude-opus'\\''s' --permission-mode 'plan' \
             --append-system-prompt 'you test first;\nthen $COMMIT' 'fix it' \
             --allowedTools 'Read' 'Bash(git diff:*)'"
        );
    }

    #[test]
    fn blank_optional_values_add_no_flag() {
        let o = LaunchOptions {
            model: Some(" ".into()),
            system_prompt: Some(String::new()),
            ..LaunchOptions::default()
        };
        assert_eq!(claude(&o), "claude --permission-mode 'acceptEdits'");
    }

    #[test]
    fn quoted_words_survive_a_real_shell() {
        let o = LaunchOptions {
            prompt: Some("a 'b' $c `d`".into()),
            model: Some("m\"x".into()),
            allowed_tools: vec!["Bash(rm -rf:*)".into()],
            ..LaunchOptions::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let echo = dir.path().join("echo-args");
        std::fs::write(&echo, "#!/bin/sh\nprintf '%s|' \"$@\"\n").unwrap();
        let command = ClaudeCodeAdapter::new(echo.to_string_lossy())
            .launch_command(&o)
            .unwrap();
        let out = std::process::Command::new("sh")
            .args(["-c", &format!("sh {command}")])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            "--model|m\"x|--permission-mode|acceptEdits|a 'b' $c `d`|--allowedTools|Bash(rm -rf:*)|"
        );
    }

    #[test]
    fn custom_bin_is_quoted_only_when_needed() {
        let plain = ClaudeCodeAdapter::new("/opt/fake/claude-1.0");
        let spaced = ClaudeCodeAdapter::new("/my tools/claude");
        let o = LaunchOptions::default();
        assert!(plain
            .launch_command(&o)
            .unwrap()
            .starts_with("/opt/fake/claude-1.0 "));
        assert!(spaced
            .launch_command(&o)
            .unwrap()
            .starts_with("'/my tools/claude' "));
    }

    #[test]
    fn shell_requires_command() {
        assert!(ShellAdapter
            .launch_command(&LaunchOptions::default())
            .is_err());
    }
}
