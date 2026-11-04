//! Ids for conversations and attachments, and Claude session ids.

use crate::{Error, Result};

const ID_LEN: usize = 12;
const MAX_ID_LEN: usize = 64;

/// 12 random lowercase hex characters: short enough for a URL.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..ID_LEN].to_string()
}

/// A UUID, as `claude --session-id` requires.
pub fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn is_session_id(value: &str) -> bool {
    uuid::Uuid::try_parse(value).is_ok()
}

/// Ids become folder names, so only `[A-Za-z0-9_-]`, at most 64 characters.
pub fn validate_id(what: &'static str, id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!("invalid {what} id `{id}`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_short_hex_and_distinct() {
        let (a, b) = (new_id(), new_id());
        assert_eq!(a.len(), ID_LEN);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert!(validate_id("conversation", &a).is_ok());
    }

    #[test]
    fn session_ids_are_uuids() {
        assert!(is_session_id(&new_session_id()));
        assert!(!is_session_id("nope"));
    }

    #[test]
    fn rejects_unsafe_ids() {
        for bad in ["", "../x", "a/b", "a b", &"x".repeat(65)] {
            assert!(validate_id("conversation", bad).is_err(), "{bad}");
        }
    }
}
