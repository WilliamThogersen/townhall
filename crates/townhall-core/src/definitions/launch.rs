use super::agent::{AgentDef, PermissionMode};
use crate::agent::LaunchOptions;
use crate::model;

/// The launch settings an agent definition carries.
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

/// A definition's mode maps onto the settings mode. Nothing maps back, so no
/// definition can reach `BypassPermissions` or `Auto`.
impl From<PermissionMode> for model::PermissionMode {
    fn from(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::Default => Self::Default,
            PermissionMode::AcceptEdits => Self::AcceptEdits,
            PermissionMode::Plan => Self::Plan,
        }
    }
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

    /// `LaunchOptions` for a validated definition. Its own permission mode wins
    /// when set, else `settings_mode`, the saved setting, is used. The caller adds
    /// the step's `prompt` and `session_id`.
    pub fn launch_options(&self, settings_mode: model::PermissionMode) -> LaunchOptions {
        let fields = self.launch_fields();
        LaunchOptions {
            permission_mode: fields.permission_mode.map_or(settings_mode, Into::into),
            command: fields.command,
            model: fields.model,
            allowed_tools: fields.allowed_tools,
            system_prompt: fields.system_prompt,
            ..LaunchOptions::default()
        }
    }
}

fn non_blank(value: &Option<String>) -> Option<String> {
    value.clone().filter(|v| !v.trim().is_empty())
}
