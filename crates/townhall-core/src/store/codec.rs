//! Turning contract values into column text and back.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::{Error, Result};

/// A unit-like enum as its serde name: `TaskState::NeedsYou` is `needsYou`.
pub fn name_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(e) => format!("unserializable: {e}"),
    }
}

pub fn parse_name<T: DeserializeOwned>(column: &str, text: &str) -> Result<T> {
    serde_json::from_value(Value::String(text.to_string()))
        .map_err(|e| corrupt(column, &format!("`{text}`: {e}")))
}

pub fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| corrupt("json", &e.to_string()))
}

pub fn from_json<T: DeserializeOwned>(column: &str, text: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|e| corrupt(column, &e.to_string()))
}

pub fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| Error::InvalidInput(format!("path is not UTF-8: {}", path.display())))
}

fn corrupt(column: &str, message: &str) -> Error {
    Error::Corrupt {
        path: format!("townhall.db column {column}"),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskState;

    #[test]
    fn names_round_trip() {
        assert_eq!(name_of(&TaskState::NeedsYou), "needsYou");
        assert_eq!(
            parse_name::<TaskState>("state", "inCi").unwrap(),
            TaskState::InCi
        );
        assert!(matches!(
            parse_name::<TaskState>("state", "nope"),
            Err(Error::Corrupt { .. })
        ));
        assert!(from_json::<Vec<String>>("after", "[1").is_err());
    }
}
