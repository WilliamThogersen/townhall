//! User-defined agents and workflows: serde types, TOML parsing and validation.
//! TOML is only the text format; later units store the source text as written.

mod agent;
mod avatar;
mod error;
mod graph;
mod launch;
mod names;
mod order;
mod parse;
mod tools;
mod validate;
mod vars;
mod words;
mod workflow;

pub use agent::{
    check_agent, parse_agent, validate_agent, AdapterKind, AgentDef, Effort, PermissionMode,
};
pub use avatar::{Accessory, AvatarColor, AvatarDef, Eyes, Head, Mouth, GRID_SIZE};
pub use error::{DefKind, DefinitionError};
pub use launch::LaunchFields;
pub use names::{step_agent_name, MAX_STEP_ID_LEN, SHORT_RUN_ID_LEN};
pub use order::start_order;
pub use parse::Parsed;
pub use tools::add_tool;
pub use validate::validate_workflow;
pub use vars::{render, variables, TemplateError, Values, Var};
pub use workflow::{check_workflow, parse_workflow, Step, WorkflowDef, MAX_STEPS};
