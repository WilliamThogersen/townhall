use std::collections::HashMap;
use std::fmt;

/// A variable a step prompt may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Var {
    /// `{{input}}`: the request the run was started with.
    Input,
    /// `{{repo}}`: the repo name.
    Repo,
    /// `{{steps.<id>.result}}`: the last assistant message of that step.
    StepResult(String),
    /// `{{steps.<id>.branch}}`: that step's branch.
    StepBranch(String),
}

impl Var {
    /// The step this variable reads from, if any.
    pub fn step(&self) -> Option<&str> {
        match self {
            Var::StepResult(id) | Var::StepBranch(id) => Some(id),
            Var::Input | Var::Repo => None,
        }
    }

    fn parse(inner: &str) -> Option<Var> {
        match inner {
            "input" => Some(Var::Input),
            "repo" => Some(Var::Repo),
            _ => {
                let (id, property) = inner.strip_prefix("steps.")?.split_once('.')?;
                match (id.is_empty(), property) {
                    (false, "result") => Some(Var::StepResult(id.to_string())),
                    (false, "branch") => Some(Var::StepBranch(id.to_string())),
                    _ => None,
                }
            }
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Var::Input => f.write_str("{{input}}"),
            Var::Repo => f.write_str("{{repo}}"),
            Var::StepResult(id) => write!(f, "{{{{steps.{id}.result}}}}"),
            Var::StepBranch(id) => write!(f, "{{{{steps.{id}.branch}}}}"),
        }
    }
}

/// Why a prompt could not be read or rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// A `{{` with no `}}` after it.
    Unclosed,
    /// The text between the braces is no known variable.
    Unknown(String),
    /// A known variable with no value at render time.
    Missing(Var),
}

impl TemplateError {
    /// The message without the leading `prompt`.
    pub(crate) fn detail(&self) -> String {
        match self {
            TemplateError::Unclosed => "has a \"{{\" without a closing \"}}\"".to_string(),
            TemplateError::Unknown(inner) => format!("uses unknown variable \"{{{{{inner}}}}}\""),
            TemplateError::Missing(var) => format!("has no value for \"{var}\""),
        }
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "prompt {}", self.detail())
    }
}

impl std::error::Error for TemplateError {}

/// The values a run fills a step prompt with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values {
    pub input: String,
    pub repo: String,
    /// Step id to that step's last assistant message.
    pub results: HashMap<String, String>,
    /// Step id to that step's branch.
    pub branches: HashMap<String, String>,
}

impl Values {
    fn get(&self, var: &Var) -> Option<&str> {
        match var {
            Var::Input => Some(&self.input),
            Var::Repo => Some(&self.repo),
            Var::StepResult(id) => self.results.get(id).map(String::as_str),
            Var::StepBranch(id) => self.branches.get(id).map(String::as_str),
        }
    }
}

pub(crate) enum Token<'a> {
    Text(&'a str),
    Var(Var),
    Bad(TemplateError),
}

/// Split a prompt into text and `{{...}}` variables. Spaces inside the braces are ignored.
pub(crate) fn tokens(prompt: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = prompt;
    while let Some(open) = rest.find("{{") {
        out.push(Token::Text(&rest[..open]));
        let inside = &rest[open + 2..];
        let Some(close) = inside.find("}}") else {
            out.push(Token::Bad(TemplateError::Unclosed));
            return out;
        };
        let inner = inside[..close].trim();
        out.push(Var::parse(inner).map_or_else(
            || Token::Bad(TemplateError::Unknown(inner.to_string())),
            Token::Var,
        ));
        rest = &inside[close + 2..];
    }
    out.push(Token::Text(rest));
    out
}

/// The variables a prompt uses, in order, or every problem in it.
pub fn variables(prompt: &str) -> Result<Vec<Var>, Vec<TemplateError>> {
    let (vars, errors) = tokens(prompt).into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut vars, mut errors), token| {
            match token {
                Token::Var(var) => vars.push(var),
                Token::Bad(error) => errors.push(error),
                Token::Text(_) => {}
            }
            (vars, errors)
        },
    );
    if errors.is_empty() {
        Ok(vars)
    } else {
        Err(errors)
    }
}

/// Fill every variable in `prompt`. Fails on an unknown variable or a missing
/// value, so no `{{...}}` is ever left in the text. Values are not expanded again.
pub fn render(prompt: &str, values: &Values) -> Result<String, TemplateError> {
    tokens(prompt)
        .into_iter()
        .map(|token| match token {
            Token::Text(text) => Ok(text),
            Token::Var(var) => values.get(&var).ok_or(TemplateError::Missing(var)),
            Token::Bad(error) => Err(error),
        })
        .collect()
}
