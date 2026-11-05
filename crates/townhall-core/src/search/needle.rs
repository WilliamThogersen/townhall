//! Matching that ignores case and runs of whitespace, and the one-line snippet around a match.
//! Folding is per character (`char::to_lowercase`), so it covers every letter, not only ASCII
//! as SQLite's `like` does, and a position in the folded text maps back to the original.

use std::ops::Range;

/// The most characters a snippet shows, not counting its `…` marks.
pub const SNIPPET_CHARS: usize = 80;
/// Characters of context kept before the match when the text is cut.
const LEAD_CHARS: usize = 24;
/// How far a cut may move to land between two words instead of inside one.
const WORD_SNAP_CHARS: usize = 12;
const CUT: &str = "…";

/// The text on one line: each run of whitespace becomes one space, the ends trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// A search text ready to match: on one line and folded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needle(String);

impl Needle {
    pub fn new(text: &str) -> Self {
        Self(fold(&one_line(text)))
    }

    /// True when `text` holds the needle. An empty needle matches nothing.
    pub fn is_in(&self, text: &str) -> bool {
        !self.0.is_empty() && fold(&one_line(text)).contains(&self.0)
    }

    /// About [`SNIPPET_CHARS`] characters of `text` on one line around the first match, with
    /// `…` where it was cut. `None` when `text` does not hold the needle.
    pub fn snippet(&self, text: &str) -> Option<String> {
        if !self.is_in(text) {
            return None;
        }
        let line = one_line(text);
        let found = self.find_in(&line)?;
        let chars: Vec<char> = line.chars().collect();
        Some(window(&chars, found))
    }

    /// The characters of `line` the first match covers.
    fn find_in(&self, line: &str) -> Option<Range<usize>> {
        let mut folded = String::with_capacity(line.len());
        let mut origin = Vec::with_capacity(line.len());
        for (index, c) in line.chars().enumerate() {
            folded.extend(c.to_lowercase());
            origin.resize(folded.len(), index);
        }
        let at = folded.find(&self.0)?;
        let last = at + self.0.len().checked_sub(1)?;
        Some(origin[at]..origin[last] + 1)
    }
}

fn window(chars: &[char], found: Range<usize>) -> String {
    if chars.len() <= SNIPPET_CHARS {
        return chars.iter().collect();
    }
    let from = found
        .start
        .saturating_sub(LEAD_CHARS)
        .min(chars.len() - SNIPPET_CHARS);
    let from = snap_start(chars, from, found.start);
    let to = snap_end(chars, from + SNIPPET_CHARS, found.end);
    let body: String = chars[from..to].iter().collect();
    let before = if from > 0 { CUT } else { "" };
    let after = if to < chars.len() { CUT } else { "" };
    format!("{before}{}{after}", body.trim())
}

/// Moves a cut at `from` forward past the next space, when one comes soon and before the match.
fn snap_start(chars: &[char], from: usize, match_start: usize) -> usize {
    if from == 0 || chars[from - 1] == ' ' {
        return from;
    }
    let reach = (from + WORD_SNAP_CHARS).min(match_start);
    (from..reach)
        .find(|&i| chars[i] == ' ')
        .map_or(from, |space| space + 1)
}

/// Moves a cut at `to` back to the last space, when one comes soon and after the match.
fn snap_end(chars: &[char], to: usize, match_end: usize) -> usize {
    let to = to.min(chars.len());
    if to == chars.len() || chars[to] == ' ' {
        return to;
    }
    let reach = to.saturating_sub(WORD_SNAP_CHARS).max(match_end).min(to);
    (reach..to).rev().find(|&i| chars[i] == ' ').unwrap_or(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(count: usize) -> String {
        (0..count)
            .map(|i| format!("word{i:02}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn matching_ignores_case_and_whitespace_runs() {
        let needle = Needle::new("  Redirect   LOOP ");
        assert!(needle.is_in("the login redirect\n\nloop again"));
        assert!(!needle.is_in("redirectloop"));
        assert!(!Needle::new("100%").is_in("1000 users"));
        assert!(!Needle::new("a_b").is_in("axb"));
        assert!(Needle::new("a_b").is_in("see A_B here"));
    }

    #[test]
    fn matching_folds_letters_beyond_ascii() {
        assert!(Needle::new("ÆBLEKAGE").is_in("en god æblekage"));
        assert!(Needle::new("søren").is_in("Hej SØREN"));
        assert!(!Needle::new("").is_in("anything"));
    }

    #[test]
    fn a_short_text_is_the_whole_snippet_on_one_line() {
        let needle = Needle::new("login");
        assert_eq!(
            needle.snippet("fix the\n  Login   bug").as_deref(),
            Some("fix the Login bug")
        );
        assert_eq!(needle.snippet("nothing here"), None);
    }

    #[test]
    fn a_long_text_is_cut_around_the_match_between_words() {
        let text = format!("{} needle {}", words(30), words(30));
        let snippet = Needle::new("NEEDLE").snippet(&text).unwrap();
        assert!(
            snippet.starts_with(CUT) && snippet.ends_with(CUT),
            "{snippet}"
        );
        assert!(snippet.contains("needle"), "{snippet}");
        let body = snippet.trim_start_matches(CUT).trim_end_matches(CUT);
        assert!(body.chars().count() <= SNIPPET_CHARS, "{snippet}");
        assert!(body.chars().count() >= SNIPPET_CHARS - 2 * WORD_SNAP_CHARS);
        for part in body.split(' ').filter(|p| *p != "needle") {
            assert!(
                part.len() == 6 && part.starts_with("word"),
                "cut inside a word: {part}"
            );
        }
        let lead = body.find("needle").unwrap();
        assert!(lead <= LEAD_CHARS + 1, "{snippet}");
    }

    #[test]
    fn a_match_near_an_end_keeps_that_end() {
        let start = Needle::new("word00").snippet(&words(40)).unwrap();
        assert!(
            start.starts_with("word00 word01") && start.ends_with(CUT),
            "{start}"
        );
        let end = Needle::new("word39").snippet(&words(40)).unwrap();
        assert!(
            end.starts_with(CUT) && end.ends_with("word38 word39"),
            "{end}"
        );
    }

    #[test]
    fn a_text_without_spaces_is_cut_where_it_falls() {
        let text = format!("{}needle{}", "x".repeat(100), "y".repeat(100));
        let snippet = Needle::new("needle").snippet(&text).unwrap();
        let body = snippet.trim_start_matches(CUT).trim_end_matches(CUT);
        assert_eq!(body.chars().count(), SNIPPET_CHARS);
        assert!(body.starts_with(&"x".repeat(LEAD_CHARS)) && body.contains("needle"));
    }

    #[test]
    fn positions_map_back_when_a_letter_folds_to_several() {
        let needle = Needle::new("stanbul");
        assert_eq!(needle.find_in("İstanbul"), Some(1..8));
        assert_eq!(Needle::new("i\u{307}s").find_in("İstanbul"), Some(0..2));
    }
}
