use std::collections::BTreeMap;

use super::agent::{AgentDef, Effort, PermissionMode};
use crate::agent::LaunchOptions;
use crate::model;

/// The launch settings an agent definition carries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchFields {
    /// `claude --model`.
    pub model: Option<String>,
    /// `claude --effort`. `None` keeps the CLI's own default.
    pub effort: Option<Effort>,
    /// `claude --permission-mode`. `None` means `acceptEdits`.
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

/// A definition's mode maps onto the launch mode. Nothing maps back, so no definition
/// can reach `Auto`.
impl From<PermissionMode> for model::PermissionMode {
    fn from(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::Default => Self::Default,
            PermissionMode::AcceptEdits => Self::AcceptEdits,
            PermissionMode::Plan => Self::Plan,
            PermissionMode::BypassPermissions => Self::BypassPermissions,
        }
    }
}

impl AgentDef {
    /// The launch settings of this definition. Blank text counts as unset.
    pub fn launch_fields(&self) -> LaunchFields {
        LaunchFields {
            model: non_blank(&self.model),
            effort: self.effort,
            permission_mode: self.permission_mode,
            allowed_tools: self.tools.clone(),
            system_prompt: non_blank(&self.prompt),
            command: non_blank(&self.command),
            env: self.env.clone(),
        }
    }

    /// `LaunchOptions` for a validated definition. A definition with no permission mode
    /// runs in `acceptEdits`. The caller adds the step's `prompt` and `session_id`.
    pub fn launch_options(&self) -> LaunchOptions {
        let fields = self.launch_fields();
        let permission_mode = fields
            .permission_mode
            .map_or(model::PermissionMode::AcceptEdits, Into::into);
        LaunchOptions {
            permission_mode,
            command: fields.command,
            model: fields.model,
            effort: fields.effort,
            allowed_tools: fields.allowed_tools,
            system_prompt: fields.system_prompt,
            env: fields.env,
            ..LaunchOptions::default()
        }
    }
}

impl AgentDef {
    /// The mode townhall's own conversation runs in: this definition's (`default` when it
    /// sets none). The planner never skips permissions, so skip all reads as `default`.
    pub fn conversation_mode(&self) -> model::PermissionMode {
        match self.permission_mode {
            None | Some(PermissionMode::BypassPermissions) => model::PermissionMode::Default,
            Some(own) => own.into(),
        }
    }
}

fn non_blank(value: &Option<String>) -> Option<String> {
    value.clone().filter(|v| !v.trim().is_empty())
}
