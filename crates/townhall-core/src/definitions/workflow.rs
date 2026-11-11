use serde::{Deserialize, Serialize};

use super::error::{DefKind, DefinitionError};
use super::parse::{parse, Parsed};
use super::validate::{validate_pipeline, KnownAgent};

/// The most steps a workflow may have. Validation is quadratic in the step count.
pub const MAX_STEPS: usize = 200;

/// Which agents run, in what order, and what each one gets as input.
/// TOML writes steps as `[[step]]`, JSON output as `steps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all(serialize = "camelCase"))]
pub struct WorkflowDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename(serialize = "steps", deserialize = "step"))]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub work: Option<String>,
    #[serde(default = "default_fix_rounds")]
    pub max_fix_rounds: u32,
}

fn default_fix_rounds() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all(serialize = "camelCase"))]
pub struct When {
    #[serde(default)]
    pub paths: Vec<String>,
}

/// One step: a saved agent started with a prompt once its `after` steps finish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all(serialize = "camelCase"))]
pub struct Step {
    pub id: String,
    pub agent: String,
    /// Step ids that must finish first. Empty means start at once.
    #[serde(default)]
    pub after: Vec<String>,
    /// Step id whose branch this step's worktree starts from.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub when: Option<When>,
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub max_retries: Option<u32>,
}

impl WorkflowDef {
    /// The agents this workflow uses, once each, in first use order.
    pub fn agent_names(&self) -> Vec<&str> {
        self.steps
            .iter()
            .enumerate()
            .filter(|(i, step)| !self.steps[..*i].iter().any(|s| s.agent == step.agent))
            .map(|(_, step)| step.agent.as_str())
            .collect()
    }
}

/// Parse a workflow definition, keeping `source` as written.
pub fn parse_workflow(source: &str) -> Result<Parsed<WorkflowDef>, DefinitionError> {
    parse(DefKind::Workflow, source)
}

/// Parse, then validate against the saved agents: what every save, import and load runs.
pub fn check_workflow(
    source: &str,
    known_agents: &[&str],
) -> Result<Parsed<WorkflowDef>, Vec<DefinitionError>> {
    let parsed = parse_workflow(source).map_err(|e| vec![e])?;
    let agents: Vec<KnownAgent> = known_agents
        .iter()
        .map(|name| KnownAgent::new(name, &[]))
        .collect();
    validate_pipeline(&parsed.def, &agents)?;
    Ok(parsed)
}

pub fn check_pipeline(
    source: &str,
    agents: &[KnownAgent],
) -> Result<Parsed<WorkflowDef>, Vec<DefinitionError>> {
    let parsed = parse_workflow(source).map_err(|e| vec![e])?;
    validate_pipeline(&parsed.def, agents)?;
    Ok(parsed)
}
