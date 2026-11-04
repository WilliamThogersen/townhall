//! The named rule: the message says a repo's name, or a path in it.

use std::path::{Path, PathBuf};

use super::Candidate;

const LEAD: &[char] = &['`', '(', '[', '{', '<', '"', '\''];
const TRAIL: &[char] = &[
    '`', ')', ']', '}', '>', '"', '\'', ',', ';', ':', '!', '?', '.',
];

/// Indices of the candidates the message names, in candidate order.
pub(crate) fn named(message: &str, candidates: &[Candidate], home: Option<&Path>) -> Vec<usize> {
    let lower = message.to_lowercase();
    let by_path: Vec<usize> = message
        .split_whitespace()
        .filter_map(|token| expand(token, home))
        .filter_map(|path| deepest(&path, candidates))
        .collect();
    candidates
        .iter()
        .enumerate()
        .filter(|(i, c)| by_path.contains(i) || mentions(&lower, &c.name.trim().to_lowercase()))
        .map(|(i, _)| i)
        .collect()
}

/// An absolute or `~/` path token as a path; anything else is not one.
fn expand(token: &str, home: Option<&Path>) -> Option<PathBuf> {
    let token = token.trim_start_matches(LEAD).trim_end_matches(TRAIL);
    match token.strip_prefix("~/") {
        Some(rest) => home.map(|h| h.join(rest)),
        None => token.starts_with('/').then(|| PathBuf::from(token)),
    }
}

/// The candidate whose path holds `path` most closely.
fn deepest(path: &Path, candidates: &[Candidate]) -> Option<usize> {
    let real = path.canonicalize().ok();
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| holds(&c.path, path, real.as_deref()))
        .max_by_key(|(_, c)| c.path.components().count())
        .map(|(i, _)| i)
}

fn holds(repo: &Path, path: &Path, real: Option<&Path>) -> bool {
    path.starts_with(repo)
        || real
            .zip(repo.canonicalize().ok())
            .is_some_and(|(real, repo)| real.starts_with(repo))
}

/// `name` appears in `haystack` as a whole word (both already lowercase).
fn mentions(haystack: &str, name: &str) -> bool {
    !name.is_empty()
        && haystack.match_indices(name).any(|(at, _)| {
            let before = haystack[..at].chars().next_back();
            let starts = before.is_none_or(|c| !joins(c) && c != '.');
            starts && ends_word(&haystack[at + name.len()..])
        })
}

/// Characters that make a longer word, path or package out of a name.
fn joins(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | '/')
}

/// A full stop ends the word; a dot before more letters makes a file name.
fn ends_word(rest: &str) -> bool {
    let mut chars = rest.chars();
    match chars.next() {
        None => true,
        Some('.') => !chars.next().is_some_and(char::is_alphanumeric),
        Some(c) => !joins(c),
    }
}

#[cfg(test)]
#[path = "named_tests.rs"]
mod tests;
