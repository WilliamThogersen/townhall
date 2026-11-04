use std::fmt;

use serde::Serialize;

use crate::cmd::validate_name;

/// Which kind of definition an error belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DefKind {
    Agent,
    Workflow,
}

impl DefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DefKind::Agent => "agent",
            DefKind::Workflow => "workflow",
        }
    }
}

/// One problem in a definition. Displays as
/// `error: workflow "ship" step "test": from "deploy" is not a step`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionError {
    pub kind: DefKind,
    /// The definition's name, when the source has one.
    pub name: Option<String>,
    pub step: Option<String>,
    /// The field at fault. The message follows it.
    pub field: Option<String>,
    pub message: String,
    /// 1-based source line, known for parse errors.
    pub line: Option<usize>,
}

impl DefinitionError {
    pub(crate) fn field(kind: DefKind, name: &str, field: &str, message: String) -> Self {
        Self {
            kind,
            name: Some(name.to_string()),
            step: None,
            field: Some(field.to_string()),
            message,
            line: None,
        }
    }

    pub(crate) fn at_step(self, step: &str) -> Self {
        Self {
            step: Some(step.to_string()),
            ..self
        }
    }
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.kind.as_str())?;
        if let Some(name) = &self.name {
            write!(f, " \"{name}\"")?;
        }
        if let Some(step) = &self.step {
            write!(f, " step \"{step}\"")?;
        }
        f.write_str(": ")?;
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        if let Some(field) = &self.field {
            write!(f, "{field} ")?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for DefinitionError {}

/// `Ok` when there are no errors.
pub(crate) fn into_result(errors: Vec<DefinitionError>) -> Result<(), Vec<DefinitionError>> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The message for a `field` whose `value` fails `validate_name`.
pub(crate) fn invalid_name(value: &str) -> Option<String> {
    validate_name(value)
        .is_err()
        .then(|| format!("\"{value}\" is not valid: use letters, digits, - and _ only"))
}
