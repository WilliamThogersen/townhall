use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::avatar::AvatarDef;
use super::error::{into_result, invalid_name, DefKind, DefinitionError};
use super::parse::{parse, Parsed};
use super::words::{model_problem, tool_problem};
use crate::scheduler::report::unrequirable;

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

/// The permission modes a definition may set. `bypassPermissions` is skip all: saving a
/// type that sets it always asks the user to confirm (`mcp/library.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "String")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    BypassPermissions,
}

impl PermissionMode {
    /// The name used in TOML and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Plan => "plan",
            PermissionMode::BypassPermissions => "bypassPermissions",
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
            "bypassPermissions" => Ok(PermissionMode::BypassPermissions),
            _ => Err(format!(
                "permission_mode \"{value}\" is not one of default, acceptEdits, plan, bypassPermissions"
            )),
        }
    }
}

/// The effort levels `claude --effort` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", try_from = "String")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    /// The value `claude --effort` takes.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

impl TryFrom<String> for Effort {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "low" => Ok(Effort::Low),
            "medium" => Ok(Effort::Medium),
            "high" => Ok(Effort::High),
            "xhigh" => Ok(Effort::Xhigh),
            "max" => Ok(Effort::Max),
            _ => Err(format!(
                "effort \"{value}\" is not one of low, medium, high, xhigh, max"
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
    pub effort: Option<Effort>,
    #[serde(default)]
    pub permission_mode: Option<PermissionMode>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    /// The `shell` adapter's command. Required for `shell`, refused for `claude`.
    #[serde(default)]
    pub command: Option<String>,
    /// The agent types this one may hand its task back to. `None`: any type.
    #[serde(default)]
    pub can_hand_back_to: Option<Vec<String>>,
    /// The agent types whose tasks must have finished with a passing report, somewhere in a
    /// task's `after` chain, before a task of this type may be created or started.
    #[serde(default)]
    pub requires: Vec<String>,
    /// The `[avatar]` table. `None`: the face derived from the name.
    #[serde(default)]
    pub avatar: Option<AvatarDef>,
    /// Environment variables the launched process gets, on top of the daemon's. Values are
    /// never logged. Townhall gives no name a meaning: they are the user's to choose.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
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
    let hand_back = hand_back_problems(def)
        .into_iter()
        .map(|m| error("can_hand_back_to", m));
    let requires = requires_problems(&def.requires)
        .into_iter()
        .map(|m| error("requires", m));
    let command = command_problem(def).map(|m| error("command", m.to_string()));
    let grid = def
        .avatar
        .iter()
        .flat_map(AvatarDef::grid_problems)
        .map(|m| error("avatar.grid", m));
    let env = env_problems(&def.env).into_iter().map(|m| error("env", m));
    let errors = name
        .into_iter()
        .chain(model.map(|m| error("model", m)))
        .chain(tools.map(|m| error("tools", m)))
        .chain(hand_back)
        .chain(requires)
        .chain(command)
        .chain(grid)
        .chain(env);
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

/// Names the offending key only: a value may be a secret.
fn env_problems(env: &BTreeMap<String, String>) -> Vec<String> {
    env.keys()
        .filter_map(|key| match (key.is_empty(), key.contains('=')) {
            (true, _) => Some("a variable name must not be empty".to_string()),
            (_, true) => Some(format!(
                "variable name \"{}\" must not contain `=`",
                key.escape_debug()
            )),
            _ => None,
        })
        .collect()
}

fn hand_back_problems(def: &AgentDef) -> Vec<String> {
    name_list_problems(def.can_hand_back_to.as_deref().unwrap_or_default())
}

fn requires_problems(names: &[String]) -> Vec<String> {
    let unusable = names
        .iter()
        .filter(|name| invalid_name(name).is_none())
        .filter_map(|name| unrequirable(name));
    name_list_problems(names)
        .into_iter()
        .chain(unusable)
        .collect()
}

fn name_list_problems(names: &[String]) -> Vec<String> {
    names
        .iter()
        .enumerate()
        .filter_map(|(at, name)| {
            let repeated = names[..at].iter().any(|n| n.eq_ignore_ascii_case(name));
            match (invalid_name(name), repeated) {
                (Some(problem), _) => Some(format!("entry \"{}\" {problem}", name.escape_debug())),
                (None, true) => Some(format!("entry \"{}\" is listed twice", name.escape_debug())),
                (None, false) => None,
            }
        })
        .collect()
}

impl AgentDef {
    /// The saved types (`all`, by name) this agent may hand back to, in the order of `all`:
    /// never itself, and only the listed ones when `can_hand_back_to` is set.
    pub fn hand_back_targets(&self, own: &str, all: &[String]) -> Vec<String> {
        all.iter()
            .filter(|name| !name.eq_ignore_ascii_case(own))
            .filter(|name| match &self.can_hand_back_to {
                None => true,
                Some(allowed) => allowed.iter().any(|a| a.eq_ignore_ascii_case(name)),
            })
            .cloned()
            .collect()
    }
}
