//! The named rule: the message says a repo's name, or a path in it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::Candidate;

const LEAD: &[char] = &['`', '(', '[', '{', '<', '"', '\''];
const TRAIL: &[char] = &[
    '`', ')', ']', '}', '>', '"', '\'', ',', ';', ':', '!', '?', '.',
];
/// Distinct path tokens checked per message; the rest are ignored.
const MAX_PATH_TOKENS: usize = 20;

/// A candidate's path as written and as the file system resolves it.
struct RepoPath<'a> {
    path: &'a Path,
    real: Option<PathBuf>,
}

/// Indices of the candidates the message names, in candidate order. Touches
/// the file system for path tokens, so run it off the async executor.
pub(crate) fn named(message: &str, candidates: &[Candidate], home: Option<&Path>) -> Vec<usize> {
    let lower = message.to_lowercase();
    let by_path = by_path(message, candidates, home);
    candidates
        .iter()
        .enumerate()
        .filter(|(i, c)| by_path.contains(i) || mentions(&lower, &c.name.trim().to_lowercase()))
        .map(|(i, _)| i)
        .collect()
}

fn by_path(message: &str, candidates: &[Candidate], home: Option<&Path>) -> Vec<usize> {
    let tokens = path_tokens(message, home);
    if tokens.is_empty() {
        return Vec::new();
    }
    let repos: Vec<RepoPath> = candidates
        .iter()
        .map(|c| RepoPath {
            path: &c.path,
            real: c.path.canonicalize().ok(),
        })
        .collect();
    tokens
        .iter()
        .filter_map(|path| deepest(path, &repos))
        .collect()
}

/// The first few distinct absolute or `~/` path tokens.
fn path_tokens(message: &str, home: Option<&Path>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    message
        .split_whitespace()
        .map(|token| token.trim_start_matches(LEAD).trim_end_matches(TRAIL))
        .filter(|token| token.starts_with('/') || token.starts_with("~/"))
        .filter(|token| seen.insert(*token))
        .filter_map(|token| expand(token, home))
        .take(MAX_PATH_TOKENS)
        .collect()
}

fn expand(token: &str, home: Option<&Path>) -> Option<PathBuf> {
    match token.strip_prefix("~/") {
        Some(rest) => home.map(|h| h.join(rest)),
        None => Some(PathBuf::from(token)),
    }
}

/// The candidate whose path holds `path` most closely.
fn deepest(path: &Path, repos: &[RepoPath]) -> Option<usize> {
    let real = path.canonicalize().ok();
    repos
        .iter()
        .enumerate()
        .filter(|(_, repo)| holds(repo, path, real.as_deref()))
        .max_by_key(|(_, repo)| repo.path.components().count())
        .map(|(i, _)| i)
}

fn holds(repo: &RepoPath, path: &Path, real: Option<&Path>) -> bool {
    path.starts_with(repo.path)
        || real
            .zip(repo.real.as_deref())
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
