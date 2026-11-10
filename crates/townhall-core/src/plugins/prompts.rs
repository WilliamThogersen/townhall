use serde::{Deserialize, Serialize};

use super::check;
use crate::Result;

pub const MAX_PROMPT_CHARS: usize = 8_000;
pub const MAX_STARTERS: usize = 20;
pub const MAX_STARTER_TITLE_CHARS: usize = 80;
pub const MAX_STARTER_TEXT_CHARS: usize = 2_000;
const MAX_STARTERS_BYTES: usize = 256 * 1024;

const STARTERS: &str = "contributes.prompts.starters";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Starter {
    pub title: String,
    pub text: String,
}

pub fn parse_starters(text: &str) -> Result<Vec<Starter>> {
    check::size(STARTERS, text, MAX_STARTERS_BYTES)?;
    let found: Vec<Starter> = serde_json::from_str(text).map_err(|e| {
        check::invalid(
            STARTERS,
            format!("starters must be a JSON list of {{\"title\", \"text\"}}: {e}"),
        )
    })?;
    if found.len() > MAX_STARTERS {
        return Err(check::invalid(
            STARTERS,
            format!("a plugin may add at most {MAX_STARTERS} starters"),
        ));
    }
    found
        .into_iter()
        .map(|starter| {
            let title = starter.title.trim().to_string();
            let text = starter.text.trim().to_string();
            for (what, value, max) in [
                ("title", &title, MAX_STARTER_TITLE_CHARS),
                ("text", &text, MAX_STARTER_TEXT_CHARS),
            ] {
                if value.is_empty() {
                    return Err(check::invalid(
                        STARTERS,
                        format!("a starter {what} is empty"),
                    ));
                }
                if value.chars().count() > max {
                    return Err(check::invalid(
                        STARTERS,
                        format!("a starter {what} is over {max} characters"),
                    ));
                }
            }
            Ok(Starter { title, text })
        })
        .collect()
}

pub fn parse_markdown(what: &str, text: &str) -> Result<String> {
    let what = format!("contributes.prompts.{what}");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(check::invalid(&what, "the prompt is empty"));
    }
    if trimmed.chars().count() > MAX_PROMPT_CHARS {
        return Err(check::invalid(
            &what,
            format!("the prompt is over {MAX_PROMPT_CHARS} characters"),
        ));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(check::invalid(
            &what,
            "the prompt holds a control character",
        ));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests;
