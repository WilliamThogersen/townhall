//! `conversation_insights`: recurring patterns across every conversation, found without a
//! model. Pure and synchronous: the caller loads the data from the store (`mcp/insights.rs`)
//! and this module counts, ranks, caps and redacts it.

use crate::redact::redact;

#[cfg(test)]
mod tests;

/// A user phrase or event text with ids, numbers and paths stripped, lowercased, so
/// repeats of the same request match.
pub fn normalize_phrase(text: &str) -> String {
    redact(text)
        .to_lowercase()
        .split_whitespace()
        .filter(|w| !w.contains(|c: char| c.is_ascii_digit() || c == '/' || c == '\\'))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
