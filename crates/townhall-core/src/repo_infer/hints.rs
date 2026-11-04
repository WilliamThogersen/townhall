//! Code hints pulled from a message: what to look for in each repo.

use std::collections::HashSet;

const MAX_HINTS: usize = 6;
const MIN_LEN: usize = 3;
const MAX_LEN: usize = 100;
const MIN_FILE_LEN: usize = 4;
const MAX_EXT_LEN: usize = 10;
const LEAD: &[char] = &['(', '[', '{', '<', '"', '\'', '*'];
const TRAIL: &[char] = &[
    ')', ']', '}', '>', '"', '\'', '*', ',', ';', ':', '!', '?', '.',
];
const NOT_EXTENSIONS: &[&str] = &["com", "org", "net", "io"];
const STOP: &[&str] = &[
    "and/or",
    "or/and",
    "either/or",
    "w/o",
    "i/o",
    "n/a",
    "y/n",
    "on/off",
    "yes/no",
    "true/false",
    "node.js",
    "next.js",
    "vue.js",
    "nuxt.js",
    "react.js",
    "three.js",
    "express.js",
    "follow-up",
    "built-in",
    "well-known",
    "real-time",
    "front-end",
    "back-end",
    "end-to-end",
    "up-to-date",
    "read-only",
    "open-source",
    "self-hosted",
    "one-off",
    "opt-in",
    "opt-out",
    "sign-in",
    "sign-up",
    "log-in",
    "set-up",
    "check-in",
    "re-run",
    "e-mail",
    "non-empty",
    "long-term",
    "short-term",
    "top-level",
    "third-party",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HintKind {
    /// Text in backticks.
    Quoted,
    /// Something with a `/`, like `src/search/query.ts`.
    Path,
    /// Something that starts with `/`, like `/api/login`.
    Route,
    /// A file name with an extension, like `query.ts`.
    File,
    /// A scoped package, like `@scope/pkg`.
    Package,
    /// A `snake_case` or `kebab-case` word, like `townhall-core`.
    Word,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hint {
    pub text: String,
    pub kind: HintKind,
}

impl Hint {
    /// The lowercase text to look for among tracked paths, if it can be one.
    pub fn path_needle(&self) -> Option<String> {
        if self.kind == HintKind::Package {
            return None;
        }
        let text = self.text.trim_start_matches("./").trim_matches('/');
        let ok = !text.is_empty() && !text.chars().any(char::is_whitespace);
        ok.then(|| text.to_ascii_lowercase())
    }
}

/// At most a handful of hints: backticks first, then paths, files, routes and
/// packages in message order, then crate-like words.
pub(crate) fn extract(message: &str) -> Vec<Hint> {
    let (quoted, outside) = split_backticks(message);
    let (words, strong): (Vec<Hint>, Vec<Hint>) = outside
        .split_whitespace()
        .filter_map(token_hint)
        .partition(|h| h.kind == HintKind::Word);
    let quoted = quoted.into_iter().map(|text| Hint {
        text,
        kind: HintKind::Quoted,
    });
    let mut seen = HashSet::new();
    quoted
        .chain(strong)
        .chain(words)
        .filter(|h| seen.insert(h.text.to_lowercase()))
        .take(MAX_HINTS)
        .collect()
}

/// Backtick spans without line breaks or other control characters, and the
/// rest of the message with every span left out.
fn split_backticks(message: &str) -> (Vec<String>, String) {
    let parts: Vec<&str> = message.split('`').collect();
    let closed = parts.len() - (parts.len() + 1) % 2;
    let inside = |i: usize| i % 2 == 1 && i < closed;
    let quoted = parts
        .iter()
        .enumerate()
        .filter(|(i, part)| inside(*i) && !part.chars().any(char::is_control))
        .map(|(_, part)| part.trim())
        .filter(|text| (MIN_LEN..=MAX_LEN).contains(&text.len()))
        .map(str::to_string)
        .collect();
    let outside = parts
        .iter()
        .enumerate()
        .filter(|(i, _)| !inside(*i))
        .map(|(_, part)| *part)
        .collect::<Vec<_>>()
        .join(" ");
    (quoted, outside)
}

fn token_hint(raw: &str) -> Option<Hint> {
    let token = raw.trim_start_matches(LEAD).trim_end_matches(TRAIL);
    classify(token).map(|kind| Hint {
        text: token.to_string(),
        kind,
    })
}

fn classify(token: &str) -> Option<HintKind> {
    let lower = token.to_ascii_lowercase();
    let skip = !(MIN_LEN..=MAX_LEN).contains(&token.len())
        || token.contains("://")
        || token.starts_with('~')
        || STOP.contains(&lower.as_str());
    if skip {
        return None;
    }
    let kind = match token.chars().next()? {
        '@' => HintKind::Package,
        '/' => HintKind::Route,
        _ if token.contains('/') => HintKind::Path,
        _ if token.contains('.') => HintKind::File,
        _ => HintKind::Word,
    };
    shaped(token, kind).then_some(kind)
}

fn shaped(token: &str, kind: HintKind) -> bool {
    match kind {
        HintKind::Package => is_package(token),
        HintKind::Route => only(token, "/-_.:") && has_letter(token) && !token.contains("//"),
        HintKind::Path => only(token, "/-_.@") && has_letter(token) && !token.contains("//"),
        HintKind::File => is_file(token),
        HintKind::Word => is_word(token),
        HintKind::Quoted => true,
    }
}

fn only(token: &str, extra: &str) -> bool {
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
}

fn has_letter(token: &str) -> bool {
    token.chars().any(|c| c.is_ascii_alphabetic())
}

fn starts_alnum(part: &str) -> bool {
    part.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
}

fn is_package(token: &str) -> bool {
    let Some((scope, name)) = token[1..].split_once('/') else {
        return false;
    };
    [scope, name]
        .iter()
        .all(|part| starts_alnum(part) && only(part, "-_."))
}

fn is_file(token: &str) -> bool {
    let Some((stem, ext)) = token.rsplit_once('.') else {
        return false;
    };
    let ext_ok = ext.len() <= MAX_EXT_LEN
        && ext.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && !NOT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str());
    let stem_ok = stem.is_empty() || stem.chars().any(|c| c.is_ascii_alphanumeric());
    token.len() >= MIN_FILE_LEN && only(token, "-_.") && ext_ok && stem_ok
}

fn is_word(token: &str) -> bool {
    token.len() > MIN_LEN
        && token
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && token
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && token.contains(['-', '_'])
        && only(token, "-_")
}

#[cfg(test)]
#[path = "hints_tests.rs"]
mod tests;
