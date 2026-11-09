//! `.townhall-plugin/plugin.json` (section 2.3) and the plain `.claude-plugin/plugin.json`
//! it can stand beside (section 2.4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::check;
use crate::Result;

pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_UI_HEIGHT: u32 = 400;

pub const API_SCOPES: &[&str] = &[
    "tasks:read",
    "tasks:create",
    "conversations:read",
    "conversations:write",
    "settings:read",
    "profile:read",
    "library:read",
    "library:write",
    "storage",
    "events",
];

pub const HOOK_EVENTS: &[&str] = &[
    "task.created",
    "task.before_start",
    "task.started",
    "task.needs_you",
    "task.ended",
    "task.failed",
    "task.check",
    "task.handed_back",
    "pr.before_open",
    "pr.opened",
    "task.merged",
    "message.before_turn",
    "message.reply",
    "conversation.created",
    "conversation.ended",
    "repo.added",
    "daemon.started",
];

pub const UI_SLOTS: &[&str] = &[
    "sidebar.nav",
    "sidebar.foot",
    "page",
    "session.section",
    "task.section",
    "composer.action",
    "settings.group",
    "agents.detail",
    "control.panel",
];

pub const PROMPT_KINDS: &[&str] = &["starters", "brief", "planner"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpTools {
    List(Vec<String>),
    Detailed(McpToolsDetail),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct McpToolsDetail {
    pub tools: Vec<String>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClaudeHooksPermission {
    pub events: Vec<String>,
    #[serde(default)]
    pub agent_types: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeMcpPermission {
    pub servers: BTreeMap<String, McpTools>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudePermissions {
    #[serde(default)]
    pub hooks: Option<ClaudeHooksPermission>,
    #[serde(default)]
    pub mcp: Option<ClaudeMcpPermission>,
    #[serde(default)]
    pub api: Vec<String>,
    #[serde(default)]
    pub planner: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default)]
    pub api: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub claude: ClaudePermissions,
    #[serde(default)]
    pub prompts: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub ui: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiEntry {
    pub slot: String,
    pub title: String,
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Contributes {
    #[serde(default)]
    pub agent_types: Vec<String>,
    #[serde(default)]
    pub prompts: BTreeMap<String, String>,
    #[serde(default)]
    pub settings: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub hooks: Option<String>,
    #[serde(default)]
    pub ui: Vec<UiEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeFiles {
    #[serde(default)]
    pub hooks: Option<String>,
    #[serde(default)]
    pub mcp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TownhallManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    pub api_version: u32,
    #[serde(default)]
    pub min_townhall: Option<String>,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub claude: ClaudeFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeManifest {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<Author>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
}

pub fn parse_townhall(text: &str) -> Result<TownhallManifest> {
    check::size("plugin manifest", text, MAX_MANIFEST_BYTES)?;
    let manifest: TownhallManifest =
        serde_json::from_str(text).map_err(|e| check::invalid("plugin manifest", e))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn parse_claude(text: &str) -> Result<ClaudeManifest> {
    check::size("plugin manifest", text, MAX_MANIFEST_BYTES)?;
    let manifest: ClaudeManifest =
        serde_json::from_str(text).map_err(|e| check::invalid("plugin manifest", e))?;
    manifest.validate()?;
    Ok(manifest)
}

impl TownhallManifest {
    fn validate(&self) -> Result<()> {
        check::name("plugin name", &self.name)?;
        check::semver("version", &self.version)?;
        if self.api_version == 0 {
            return Err(check::invalid("apiVersion", "must be 1 or higher"));
        }
        if let Some(min) = &self.min_townhall {
            check::semver("minTownhall", min)?;
        }
        validate_links(self.author.as_ref(), self.homepage.as_deref(), None)?;
        self.permissions.validate()?;
        self.contributes.validate(&self.permissions)?;
        for (what, path) in [
            ("claude.hooks", &self.claude.hooks),
            ("claude.mcp", &self.claude.mcp),
        ] {
            if let Some(path) = path {
                check::rel_path(what, path)?;
            }
        }
        Ok(())
    }
}

impl ClaudeManifest {
    fn validate(&self) -> Result<()> {
        check::name("plugin name", &self.name)?;
        if let Some(version) = &self.version {
            check::semver("version", version)?;
        }
        validate_links(
            self.author.as_ref(),
            self.homepage.as_deref(),
            self.repository.as_deref(),
        )
    }
}

pub(super) fn validate_links(
    author: Option<&Author>,
    homepage: Option<&str>,
    repository: Option<&str>,
) -> Result<()> {
    if let Some(url) = author.and_then(|a| a.url.as_deref()) {
        check::http_url("author.url", url)?;
    }
    if let Some(url) = homepage {
        check::http_url("homepage", url)?;
    }
    if let Some(url) = repository {
        check::http_url("repository", url)?;
    }
    Ok(())
}

impl Permissions {
    pub(super) fn validate(&self) -> Result<()> {
        for scope in &self.api {
            check::one_of("permissions.api", scope, API_SCOPES)?;
        }
        for event in &self.hooks {
            check::one_of("permissions.hooks", event, HOOK_EVENTS)?;
        }
        for kind in &self.prompts {
            check::one_of("permissions.prompts", kind, PROMPT_KINDS)?;
        }
        for slot in &self.ui {
            check::one_of("permissions.ui", slot, UI_SLOTS)?;
        }
        for name in &self.env {
            check::env_name("permissions.env", name)?;
        }
        for scope in &self.claude.api {
            check::one_of("permissions.claude.api", scope, API_SCOPES)?;
        }
        if let Some(hooks) = &self.claude.hooks {
            for event in &hooks.events {
                check::word("permissions.claude.hooks.events", event, &['_'])?;
            }
            for agent_type in &hooks.agent_types {
                check::word(
                    "permissions.claude.hooks.agentTypes",
                    agent_type,
                    &['-', '_'],
                )?;
            }
        }
        if let Some(mcp) = &self.claude.mcp {
            for (server, tools) in &mcp.servers {
                check::word("permissions.claude.mcp.servers", server, &['-', '_'])?;
                if server.eq_ignore_ascii_case("townhall") {
                    return Err(check::invalid(
                        "permissions.claude.mcp.servers",
                        "`townhall` is reserved for townhall's own server",
                    ));
                }
                let names = match tools {
                    McpTools::List(names) => names,
                    McpTools::Detailed(detail) => &detail.tools,
                };
                for tool in names.iter().filter(|t| *t != "*") {
                    check::word("permissions.claude.mcp.servers", tool, &['-', '_'])?;
                }
            }
        }
        Ok(())
    }
}

impl Contributes {
    fn validate(&self, permissions: &Permissions) -> Result<()> {
        for path in &self.agent_types {
            check::rel_path("contributes.agentTypes", path)?;
        }
        for (kind, path) in &self.prompts {
            check::one_of("contributes.prompts", kind, PROMPT_KINDS)?;
            if !permissions.prompts.contains(kind) {
                return Err(check::invalid(
                    "contributes.prompts",
                    format!("`{kind}` needs to be listed under permissions.prompts"),
                ));
            }
            check::rel_path("contributes.prompts", path)?;
        }
        for (what, path) in [
            ("contributes.settings", &self.settings),
            ("contributes.theme", &self.theme),
            ("contributes.hooks", &self.hooks),
        ] {
            if let Some(path) = path {
                check::rel_path(what, path)?;
            }
        }
        for ui in &self.ui {
            check::one_of("contributes.ui.slot", &ui.slot, UI_SLOTS)?;
            if !permissions.ui.contains(&ui.slot) {
                return Err(check::invalid(
                    "contributes.ui",
                    format!("slot `{}` needs to be listed under permissions.ui", ui.slot),
                ));
            }
            if ui.title.trim().is_empty() {
                return Err(check::invalid("contributes.ui.title", "is empty"));
            }
            check::rel_path("contributes.ui.entry", &ui.entry)?;
            if let Some(height) = ui.height {
                if height == 0 || height > MAX_UI_HEIGHT {
                    return Err(check::invalid(
                        "contributes.ui.height",
                        format!("{height} is outside 1 to {MAX_UI_HEIGHT}"),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
