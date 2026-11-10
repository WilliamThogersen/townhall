use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::check;
use crate::{Error, Result};

pub const MAX_FIELDS: usize = 50;
pub const MAX_STRING_CHARS: usize = 2_000;
pub const MAX_CHOICES: usize = 50;
const MAX_LABEL_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const WHAT: &str = "contributes.settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Boolean,
    String,
    Integer,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: FieldKind,
    pub default: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<String>>,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaFile {
    fields: Vec<Field>,
}

pub fn parse(text: &str) -> Result<Vec<Field>> {
    check::size(WHAT, text, MAX_SCHEMA_BYTES)?;
    let file: SchemaFile = serde_json::from_str(text).map_err(|e| {
        check::invalid(
            WHAT,
            format!("settings must look like {{\"fields\": [...]}}: {e}"),
        )
    })?;
    if file.fields.len() > MAX_FIELDS {
        return Err(check::invalid(
            WHAT,
            format!("a plugin may have at most {MAX_FIELDS} settings"),
        ));
    }
    for (at, field) in file.fields.iter().enumerate() {
        field.validate()?;
        if file.fields[..at]
            .iter()
            .any(|earlier| earlier.key.eq_ignore_ascii_case(&field.key))
        {
            return Err(check::invalid(
                WHAT,
                format!("key `{}` is listed twice", field.key),
            ));
        }
    }
    Ok(file.fields)
}

fn bad(field: &Field, why: impl std::fmt::Display) -> Error {
    check::invalid(&format!("{WHAT} `{}`", field.key), why)
}

impl Field {
    fn validate(&self) -> Result<()> {
        check::name("settings field key", &self.key)?;
        let label = self.label.trim();
        if label.is_empty() || label.chars().count() > MAX_LABEL_CHARS {
            return Err(bad(
                self,
                format!("the label needs text of at most {MAX_LABEL_CHARS} characters"),
            ));
        }
        if let Some(description) = &self.description {
            if description.chars().count() > MAX_DESCRIPTION_CHARS {
                return Err(bad(
                    self,
                    format!("the description is over {MAX_DESCRIPTION_CHARS} characters"),
                ));
            }
        }
        if self.kind != FieldKind::Integer {
            for (name, set) in [("min", self.min.is_some()), ("max", self.max.is_some())] {
                if set {
                    return Err(bad(self, format!("{name} is only for integer settings")));
                }
            }
        }
        if self.kind != FieldKind::Enum && self.choices.is_some() {
            return Err(bad(self, "choices are only for enum settings"));
        }
        if self.secret && self.kind != FieldKind::String {
            return Err(bad(self, "only a string setting can be secret"));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if min > max {
                return Err(bad(self, format!("min {min} is above max {max}")));
            }
        }
        if self.kind == FieldKind::Enum {
            self.validate_choices()?;
        }
        self.check(&self.default)
            .map(|_| ())
            .map_err(|e| bad(self, format!("the default is not valid: {e}")))
    }

    fn validate_choices(&self) -> Result<()> {
        let choices = self.choices.as_deref().unwrap_or_default();
        let usable = !choices.is_empty()
            && choices.len() <= MAX_CHOICES
            && choices.iter().all(|c| {
                !c.trim().is_empty() && c.trim() == c && c.chars().count() <= MAX_LABEL_CHARS
            })
            && choices
                .iter()
                .enumerate()
                .all(|(at, c)| !choices[..at].contains(c));
        if usable {
            Ok(())
        } else {
            Err(bad(
                self,
                format!(
                    "choices need 1 to {MAX_CHOICES} different, trimmed, non-empty entries of at most {MAX_LABEL_CHARS} characters"
                ),
            ))
        }
    }

    pub fn check(&self, value: &Value) -> Result<Value> {
        let wrong = |wanted: &str| check::invalid(&self.key, format!("must be {wanted}"));
        match self.kind {
            FieldKind::Boolean if value.is_boolean() => Ok(value.clone()),
            FieldKind::Boolean => Err(wrong("true or false")),
            FieldKind::String => match value.as_str() {
                Some(text) if text.chars().count() <= MAX_STRING_CHARS => Ok(value.clone()),
                Some(_) => Err(wrong(&format!("at most {MAX_STRING_CHARS} characters"))),
                None => Err(wrong("text")),
            },
            FieldKind::Integer => {
                let number = value.as_i64().ok_or_else(|| wrong("a whole number"))?;
                if self.min.is_some_and(|min| number < min) {
                    return Err(wrong(&format!("at least {}", self.min.unwrap_or_default())));
                }
                if self.max.is_some_and(|max| number > max) {
                    return Err(wrong(&format!("at most {}", self.max.unwrap_or_default())));
                }
                Ok(value.clone())
            }
            FieldKind::Enum => {
                let choices = self.choices.as_deref().unwrap_or_default();
                match value.as_str() {
                    Some(text) if choices.iter().any(|c| c == text) => Ok(value.clone()),
                    _ => Err(wrong(&format!("one of {}", choices.join(", ")))),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
