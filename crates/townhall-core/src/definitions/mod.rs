//! User-defined agents and workflows: serde types, TOML parsing and validation.
//! TOML is only the text format; later units store the source text as written.

mod agent;
mod error;
mod graph;
mod launch;
mod names;
mod order;
mod parse;
mod validate;
mod vars;
mod workflow;

pub use agent::{check_agent, parse_agent, validate_agent, AdapterKind, AgentDef, PermissionMode};
pub use error::{DefKind, DefinitionError};
pub use launch::LaunchFields;
pub use names::{step_agent_name, SHORT_RUN_ID_LEN};
pub use order::start_order;
pub use parse::Parsed;
pub use validate::validate_workflow;
pub use vars::{render, variables, TemplateError, Values, Var};
pub use workflow::{check_workflow, parse_workflow, Step, WorkflowDef};
