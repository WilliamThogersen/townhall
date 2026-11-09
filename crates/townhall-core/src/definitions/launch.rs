use std::collections::BTreeMap;

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
    /// Extra environment variables for the launched process.
    pub env: BTreeMap<String, String>,
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
            env: self.env.clone(),
        }
    }

    /// `LaunchOptions` for a validated definition. Its permission mode can only
    /// narrow `settings_mode`, the saved setting: the stricter of the two is used.
    /// The caller adds the step's `prompt` and `session_id`.
    pub fn launch_options(&self, settings_mode: model::PermissionMode) -> LaunchOptions {
        let fields = self.launch_fields();
        let permission_mode = fields
            .permission_mode
            .map_or(settings_mode, |own| stricter(own.into(), settings_mode));
        LaunchOptions {
            permission_mode,
            command: fields.command,
            model: fields.model,
            allowed_tools: fields.allowed_tools,
            system_prompt: fields.system_prompt,
            env: fields.env,
            ..LaunchOptions::default()
        }
    }
}

/// Strictest first: `plan`, `default`, `acceptEdits`, `auto`, then skip all.
fn strictness(mode: model::PermissionMode) -> u8 {
    match mode {
        model::PermissionMode::Plan => 0,
        model::PermissionMode::Default => 1,
        model::PermissionMode::AcceptEdits => 2,
        model::PermissionMode::Auto => 3,
        model::PermissionMode::BypassPermissions => 4,
    }
}

fn stricter(a: model::PermissionMode, b: model::PermissionMode) -> model::PermissionMode {
    if strictness(a) <= strictness(b) {
        a
    } else {
        b
    }
}

fn non_blank(value: &Option<String>) -> Option<String> {
    value.clone().filter(|v| !v.trim().is_empty())
}
