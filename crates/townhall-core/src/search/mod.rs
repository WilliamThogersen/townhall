//! Search (unit 3.3): conversations by title and by the text of your and townhall's messages.
//! `Store::search` runs it; `GET /api/search` serves it. See `docs/daemon-api.md`.

mod needle;

use serde::{Deserialize, Serialize};

pub use needle::{Needle, SNIPPET_CHARS};

pub const MAX_QUERY_CHARS: usize = 200;
pub const DEFAULT_LIMIT: usize = 50;
pub const MAX_LIMIT: usize = 200;

/// Why a search request was refused; the text is the API's `400` error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidSearch {
    #[error("search needs 1 to {} characters", MAX_QUERY_CHARS)]
    Text,
    #[error("limit needs to be a whole number from 1 to {}", MAX_LIMIT)]
    Limit,
}

/// A checked search: the text trimmed, 1 to [`MAX_QUERY_CHARS`] characters, and how many hits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    text: String,
    limit: usize,
}

impl SearchQuery {
    /// `limit` is 1 to [`MAX_LIMIT`], [`DEFAULT_LIMIT`] when missing or blank.
    pub fn parse(text: Option<&str>, limit: Option<&str>) -> Result<Self, InvalidSearch> {
        let text = text.unwrap_or_default().trim();
        if !(1..=MAX_QUERY_CHARS).contains(&text.chars().count()) {
            return Err(InvalidSearch::Text);
        }
        let limit = match limit.map(str::trim).filter(|l| !l.is_empty()) {
            None => DEFAULT_LIMIT,
            Some(raw) => raw
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_LIMIT).contains(n))
                .ok_or(InvalidSearch::Limit)?,
        };
        Ok(Self {
            text: text.to_string(),
            limit,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// What matched: the conversation's title, or one of your or townhall's messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchMatch {
    Title,
    Message,
}

/// One conversation that holds the search text. A title match comes without an entry and a
/// snippet; a message match names the newest matching entry and a one-line snippet of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub conversation_id: String,
    pub title: String,
    #[serde(rename = "match")]
    pub matched: SearchMatch,
    pub entry_id: Option<u64>,
    pub snippet: Option<String>,
    /// The conversation's `updatedAt`.
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_text_is_trimmed_and_1_to_200_characters() {
        let q = SearchQuery::parse(Some("  login  "), None).unwrap();
        assert_eq!((q.text(), q.limit()), ("login", DEFAULT_LIMIT));
        let longest = "ø".repeat(MAX_QUERY_CHARS);
        assert!(SearchQuery::parse(Some(&longest), None).is_ok());
        for bad in [None, Some(""), Some(" \n\t "), Some(&*"a".repeat(201))] {
            assert_eq!(SearchQuery::parse(bad, None), Err(InvalidSearch::Text));
        }
        assert_eq!(
            InvalidSearch::Text.to_string(),
            "search needs 1 to 200 characters"
        );
    }

    #[test]
    fn the_limit_is_1_to_200_and_defaults_to_50() {
        let limit = |raw| SearchQuery::parse(Some("x"), raw).map(|q| q.limit());
        assert_eq!(limit(None), Ok(50));
        assert_eq!(limit(Some(" ")), Ok(50));
        assert_eq!(limit(Some("1")), Ok(1));
        assert_eq!(limit(Some("200")), Ok(200));
        for bad in ["0", "201", "-1", "abc", "2.5"] {
            assert_eq!(limit(Some(bad)), Err(InvalidSearch::Limit), "{bad}");
        }
        assert_eq!(
            InvalidSearch::Limit.to_string(),
            "limit needs to be a whole number from 1 to 200"
        );
    }

    #[test]
    fn a_hit_is_camel_case_with_match_and_nulls() {
        let hit = SearchHit {
            conversation_id: "c1".into(),
            title: "login loop".into(),
            matched: SearchMatch::Title,
            entry_id: None,
            snippet: None,
            updated_at: "2026-11-04T10:42:05.000Z".into(),
        };
        assert_eq!(
            serde_json::to_value(&hit).unwrap(),
            json!({
                "conversationId": "c1", "title": "login loop", "match": "title",
                "entryId": null, "snippet": null, "updatedAt": "2026-11-04T10:42:05.000Z"
            })
        );
    }
}
