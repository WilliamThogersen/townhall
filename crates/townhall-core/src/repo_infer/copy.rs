//! Interface copy for inference reasons and the `~` event.

use super::Inference;

pub(crate) const NAMED: &str = "you named it";
pub(crate) const ONLY: &str = "only repo";

const WORDS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

/// `two`, `three`, ... up to nine, digits after that.
fn count(n: usize) -> String {
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), |word| word.to_string())
}

/// Copy never holds control characters: a file or repo name with a newline
/// shows a space there.
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The code rule's reason: `it has src/search/query.ts`.
pub(crate) fn has(hit: &str) -> String {
    format!("it has {}", clean(hit))
}

pub(crate) fn named_several(n: usize) -> String {
    format!("you named {} repos. which one?", count(n))
}

pub(crate) fn share(n: usize, hint: &str) -> String {
    format!("{} repos have {hint}. which one?", count(n))
}

pub(crate) fn could_fit(n: usize) -> String {
    format!("{} repos could fit. which one?", count(n))
}

/// `townhall is not reachable. which one?` for named repos that were skipped.
pub(crate) fn unreachable(names: &[String]) -> String {
    let (list, verb) = match names {
        [one] => (one.clone(), "is"),
        [rest @ .., last] => (format!("{} and {last}", rest.join(", ")), "are"),
        [] => (String::new(), "are"),
    };
    format!("{} {verb} not reachable. which one?", clean(&list))
}

/// `~ repo shop · develop, it has src/search/query.ts` for a found repo.
pub(crate) fn event_text(inference: &Inference, base_branch: &str) -> Option<String> {
    match inference {
        Inference::Found { repo, reason, .. } => {
            Some(clean(&format!("~ repo {repo} · {base_branch}, {reason}")))
        }
        Inference::Ask { .. } | Inference::NoRepos | Inference::NoSignal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_words_up_to_nine() {
        assert_eq!(could_fit(2), "two repos could fit. which one?");
        assert_eq!(named_several(9), "you named nine repos. which one?");
        assert_eq!(share(12, "query.ts"), "12 repos have query.ts. which one?");
        assert_eq!(has("src/a.ts"), "it has src/a.ts");
    }

    #[test]
    fn control_characters_become_spaces() {
        assert_eq!(has("a\nb/query.ts"), "it has a b/query.ts");
        assert_eq!(
            unreachable(&["town\thall".to_string()]),
            "town hall is not reachable. which one?"
        );
        let found = Inference::Found {
            repo: "sh\rop".into(),
            rule: crate::repo_infer::Rule::Code,
            reason: "it has x\u{1b}y".into(),
        };
        assert_eq!(
            event_text(&found, "ma\nin").as_deref(),
            Some("~ repo sh op · ma in, it has x y")
        );
    }

    #[test]
    fn unreachable_lists_the_names() {
        let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(
            unreachable(&names(&["townhall"])),
            "townhall is not reachable. which one?"
        );
        assert_eq!(
            unreachable(&names(&["a", "b"])),
            "a and b are not reachable. which one?"
        );
        assert_eq!(
            unreachable(&names(&["a", "b", "c"])),
            "a, b and c are not reachable. which one?"
        );
    }
}
