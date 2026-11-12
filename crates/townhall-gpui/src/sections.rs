use std::collections::HashSet;

use townhall_core::model::Conversation;

pub struct Filter<'a> {
    pub query: &'a str,
    pub text_matches: &'a HashSet<String>,
}

pub struct Sections {
    pub active: Vec<Conversation>,
    pub older: Vec<Conversation>,
}

#[allow(dead_code)]
pub struct FoldParts {
    pub before: Vec<Conversation>,
    pub current: Option<Conversation>,
    pub after: Vec<Conversation>,
}

pub fn normalize_query(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn matches_title(title: &str, normalized: &str) -> bool {
    normalize_query(title).contains(normalized)
}

fn newest_first(a: &Conversation, b: &Conversation) -> std::cmp::Ordering {
    b.updated_at
        .cmp(&a.updated_at)
        .then_with(|| b.created_at.cmp(&a.created_at))
}

pub fn split_sections(
    conversations: &[Conversation],
    needs_you: impl Fn(&Conversation) -> bool,
    filter: Option<&Filter>,
) -> Sections {
    let normalized = filter.map(|f| normalize_query(f.query)).unwrap_or_default();
    let shown = |c: &&Conversation| match filter {
        Some(f) if !normalized.is_empty() => {
            matches_title(&c.title, &normalized) || f.text_matches.contains(&c.id)
        }
        _ => true,
    };
    let shown: Vec<&Conversation> = conversations.iter().filter(shown).collect();
    let mut active: Vec<Conversation> = shown
        .iter()
        .filter(|c| !c.ended)
        .map(|c| (*c).clone())
        .collect();
    let mut older: Vec<Conversation> = shown
        .iter()
        .filter(|c| c.ended)
        .map(|c| (*c).clone())
        .collect();
    active.sort_by(|a, b| {
        needs_you(b)
            .cmp(&needs_you(a))
            .then_with(|| newest_first(a, b))
    });
    older.sort_by(newest_first);
    Sections { active, older }
}

pub fn fold_parts(rows: &[Conversation], current_id: Option<&str>) -> FoldParts {
    let at = current_id.and_then(|id| rows.iter().position(|row| row.id == id));
    match at {
        None => FoldParts {
            before: rows.to_vec(),
            current: None,
            after: Vec::new(),
        },
        Some(at) => FoldParts {
            before: rows[..at].to_vec(),
            current: Some(rows[at].clone()),
            after: rows[at + 1..].to_vec(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::conversation;

    fn at(minute: u32) -> String {
        format!("2026-11-04T10:{minute:02}:00.000Z")
    }

    fn c(id: &str, minute: u32, ended: bool) -> Conversation {
        let mut c = conversation(id, id, &at(minute));
        c.ended = ended;
        c
    }

    fn ids(rows: &[Conversation]) -> Vec<&str> {
        rows.iter().map(|r| r.id.as_str()).collect()
    }

    fn nobody(_: &Conversation) -> bool {
        false
    }

    fn rows() -> Vec<Conversation> {
        vec![
            c("old", 1, false),
            c("new", 9, false),
            c("ended-old", 2, true),
            c("ended-new", 8, true),
            c("mid", 5, false),
        ]
    }

    #[test]
    fn normalize_and_match_ignore_case_and_runs_of_whitespace() {
        assert_eq!(normalize_query("  Login \n  LOOP "), "login loop");
        assert!(matches_title("the login\tloop again", "login loop"));
        assert!(matches_title("ÆBLEKAGE", &normalize_query("æble")));
        assert!(!matches_title("checkout", "login"));
    }

    #[test]
    fn ended_go_under_older_newest_activity_first_in_both() {
        let s = split_sections(&rows(), nobody, None);
        assert_eq!(ids(&s.active), ["new", "mid", "old"]);
        assert_eq!(ids(&s.older), ["ended-new", "ended-old"]);
    }

    #[test]
    fn active_rows_that_need_you_come_first() {
        let s = split_sections(&rows(), |c| c.id == "old" || c.id == "ended-old", None);
        assert_eq!(ids(&s.active), ["old", "new", "mid"]);
        assert_eq!(ids(&s.older), ["ended-new", "ended-old"]);
    }

    #[test]
    fn ties_in_updated_at_fall_back_to_created_at() {
        let mut a = c("a", 5, false);
        let mut b = c("b", 5, false);
        a.created_at = "2026-11-04T09:00:00.000Z".into();
        b.created_at = "2026-11-04T09:30:00.000Z".into();
        let s = split_sections(&[a, b], nobody, None);
        assert_eq!(ids(&s.active), ["b", "a"]);
    }

    #[test]
    fn filters_by_title_and_by_message_matches() {
        let matches: HashSet<String> = ["mid".to_string()].into();
        let filter = Filter {
            query: "NE",
            text_matches: &matches,
        };
        let s = split_sections(&rows(), nobody, Some(&filter));
        assert_eq!(ids(&s.active), ["new", "mid"]);
        assert_eq!(ids(&s.older), ["ended-new"]);
    }

    #[test]
    fn a_blank_query_shows_everything() {
        let matches = HashSet::new();
        let filter = Filter {
            query: "   ",
            text_matches: &matches,
        };
        assert_eq!(
            split_sections(&rows(), nobody, Some(&filter)).active.len(),
            3
        );
    }

    #[test]
    fn fold_parts_split_around_the_current_row() {
        let rows = vec![c("a", 1, false), c("b", 2, false), c("c", 3, false)];
        let parts = fold_parts(&rows, Some("b"));
        assert_eq!(ids(&parts.before), ["a"]);
        assert_eq!(parts.current.map(|c| c.id), Some("b".to_string()));
        assert_eq!(ids(&parts.after), ["c"]);
    }

    #[test]
    fn fold_parts_keep_every_row_before_when_none_is_current() {
        let rows = vec![c("a", 1, false), c("b", 2, false), c("c", 3, false)];
        let parts = fold_parts(&rows, None);
        assert_eq!(ids(&parts.before), ["a", "b", "c"]);
        assert!(parts.current.is_none());
        assert!(parts.after.is_empty());
    }
}
