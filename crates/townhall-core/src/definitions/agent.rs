use serde::{Deserialize, Serialize};

use super::error::{into_result, invalid_name, DefKind, DefinitionError};
use super::parse::{parse, Parsed};
use super::words::{model_problem, tool_problem};

/// The adapter an agent runs with, matching `AgentAdapter::kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    Claude,
    Shell,
}

impl AdapterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AdapterKind::Claude => "claude",
            AdapterKind::Shell => "shell",
        }
    }
}

/// The permission modes a definition may set. `bypassPermissions` is refused:
/// skip all is set only through the settings endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "String")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
}

impl PermissionMode {
    /// The value `claude --permission-mode` takes.
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Plan => "plan",
        }
    }
}

impl TryFrom<String> for PermissionMode {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "default" => Ok(PermissionMode::Default),
            "acceptEdits" => Ok(PermissionMode::AcceptEdits),
            "plan" => Ok(PermissionMode::Plan),
            "bypassPermissions" => Err(format!(
                "permission_mode \"{value}\" is not allowed in a definition: skip all is set only in settings"
            )),
            _ => Err(format!(
                "permission_mode \"{value}\" is not one of default, acceptEdits, plan"
            )),
        }
    }
}

/// How one worker runs. TOML keys are snake_case, JSON output is camelCase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all(serialize = "camelCase"))]
pub struct AgentDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub adapter: AdapterKind,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<PermissionMode>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    /// The `shell` adapter's command. Required for `shell`, refused for `claude`.
    #[serde(default)]
    pub command: Option<String>,
}

/// Parse an agent definition, keeping `source` as written.
pub fn parse_agent(source: &str) -> Result<Parsed<AgentDef>, DefinitionError> {
    parse(DefKind::Agent, source)
}

/// Check an agent against every rule and return all errors found.
pub fn validate_agent(def: &AgentDef) -> Result<(), Vec<DefinitionError>> {
    let error = |field: &str, message: String| {
        DefinitionError::field(DefKind::Agent, &def.name, field, message)
    };
    let name = invalid_name(&def.name).map(|m| error("name", m));
    let model = def
        .model
        .as_deref()
        .filter(|m| !m.trim().is_empty())
        .and_then(model_problem);
    let tools = def.tools.iter().filter_map(|tool| tool_problem(tool));
    let command = command_problem(def).map(|m| error("command", m.to_string()));
    let errors = name
        .into_iter()
        .chain(model.map(|m| error("model", m)))
        .chain(tools.map(|m| error("tools", m)))
        .chain(command);
    into_result(errors.collect())
}

/// Parse, then validate: what every save, import and load runs.
pub fn check_agent(source: &str) -> Result<Parsed<AgentDef>, Vec<DefinitionError>> {
    let parsed = parse_agent(source).map_err(|e| vec![e])?;
    validate_agent(&parsed.def)?;
    Ok(parsed)
}

fn command_problem(def: &AgentDef) -> Option<&'static str> {
    let runnable = def.command.as_deref().is_some_and(|c| !c.trim().is_empty());
    match def.adapter {
        AdapterKind::Shell if !runnable => Some("is required for the shell adapter"),
        AdapterKind::Claude if def.command.is_some() => Some("is only for the shell adapter"),
        _ => None,
    }
}
