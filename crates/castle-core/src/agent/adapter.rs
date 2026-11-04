use crate::{Error, Result};

/// Per-launch options handed to an adapter.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub prompt: Option<String>,
    /// Raw command, used by adapters that run arbitrary programs.
    pub command: Option<String>,
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

/// Launches the `claude` CLI, optionally with an initial prompt.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCodeAdapter;

impl AgentAdapter for ClaudeCodeAdapter {
    fn kind(&self) -> &'static str {
        "claude"
    }

    fn launch_command(&self, options: &LaunchOptions) -> Result<String> {
        Ok(match options.prompt.as_deref().filter(|p| !p.is_empty()) {
            Some(prompt) => format!("claude {}", shell_quote(prompt)),
            None => "claude".to_string(),
        })
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
    fn claude_quotes_prompt() {
        let o = LaunchOptions { prompt: Some("it's $HOME".into()), command: None };
        assert_eq!(
            ClaudeCodeAdapter.launch_command(&o).unwrap(),
            "claude 'it'\\''s $HOME'"
        );
    }

    #[test]
    fn claude_without_prompt() {
        assert_eq!(ClaudeCodeAdapter.launch_command(&LaunchOptions::default()).unwrap(), "claude");
    }

    #[test]
    fn shell_requires_command() {
        assert!(ShellAdapter.launch_command(&LaunchOptions::default()).is_err());
    }
}
