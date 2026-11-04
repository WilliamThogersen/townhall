use super::agent::{AgentDef, PermissionMode};

/// The launch settings an agent definition carries, ready to map onto `LaunchOptions`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchFields {
    /// `claude --model`.
    pub model: Option<String>,
    /// `claude --permission-mode`. `None` means the default from settings.
    pub permission_mode: Option<PermissionMode>,
    /// `claude --allowedTools`.
    pub allowed_tools: Vec<String>,
    /// `claude --append-system-prompt`.
    pub system_prompt: Option<String>,
    /// The `shell` adapter's command.
    pub command: Option<String>,
}

impl AgentDef {
    /// The launch settings of this definition. Blank text counts as unset.
    pub fn launch_fields(&self) -> LaunchFields {
        LaunchFields {
            model: non_blank(&self.model),
            permission_mode: self.permission_mode,
            allowed_tools: self.tools.clone(),
            system_prompt: non_blank(&self.prompt),
            command: non_blank(&self.command),
        }
    }
}

fn non_blank(value: &Option<String>) -> Option<String> {
    value.clone().filter(|v| !v.trim().is_empty())
}
