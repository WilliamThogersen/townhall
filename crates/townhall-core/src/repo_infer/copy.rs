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

/// The code rule's reason: `it has src/search/query.ts`.
pub(crate) fn has(hit: &str) -> String {
    format!("it has {hit}")
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
    format!("{list} {verb} not reachable. which one?")
}

/// `~ repo shop · develop, it has src/search/query.ts` for a found repo.
pub(crate) fn event_text(inference: &Inference, base_branch: &str) -> Option<String> {
    match inference {
        Inference::Found { repo, reason, .. } => {
            Some(format!("~ repo {repo} · {base_branch}, {reason}"))
        }
        Inference::Ask { .. } | Inference::NoRepos => None,
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
