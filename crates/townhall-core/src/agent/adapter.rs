use std::path::PathBuf;

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
    /// `claude --add-dir`: absolute folders outside the worktree claude may read without
    /// asking (a task's brief, its conversation's attachments).
    pub add_dirs: Vec<PathBuf>,
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
pub(super) fn shell_word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-+@%:,=".contains(c));
    if plain {
        s.to_string()
    } else {
        shell_quote(s)
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

    #[test]
    fn quoting_survives_a_shell() {
        let out = std::process::Command::new("sh")
            .args([
                "-c",
                &format!("printf %s {}", shell_quote("it's $HOME `x`")),
            ])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "it's $HOME `x`");
        assert_eq!(shell_word("plain-word_1.0"), "plain-word_1.0");
        assert_eq!(shell_word("two words"), "'two words'");
    }

    #[test]
    fn shell_requires_command() {
        assert!(ShellAdapter
            .launch_command(&LaunchOptions::default())
            .is_err());
        let o = LaunchOptions {
            command: Some("  sleep 1 ".into()),
            ..LaunchOptions::default()
        };
        assert_eq!(ShellAdapter.launch_command(&o).unwrap(), "sleep 1");
    }
}
