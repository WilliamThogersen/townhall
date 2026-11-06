//! What a tester's or reviewer's report says in its last lines: `VERDICT: APPROVE|REJECT`
//! (reviewer), `VERDICT: PASS|FAIL` (tester) and `COMMIT: <sha>`.

/// How many of a report's last non-blank lines are read for the verdict and the commit.
pub const TAIL_LINES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Approve,
    Reject,
    Pass,
    Fail,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Approve => "APPROVE",
            Verdict::Reject => "REJECT",
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
        }
    }

    pub fn clears(self) -> bool {
        matches!(self, Verdict::Approve | Verdict::Pass)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub verdict: Option<Verdict>,
    pub commit: Option<String>,
}

fn verdict_of(word: &str) -> Option<Verdict> {
    match word {
        "APPROVE" => Some(Verdict::Approve),
        "REJECT" => Some(Verdict::Reject),
        "PASS" => Some(Verdict::Pass),
        "FAIL" => Some(Verdict::Fail),
        _ => None,
    }
}

fn is_sha(word: &str) -> bool {
    (7..=40).contains(&word.len()) && word.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// The verdict and commit of the last `VERDICT:` and `COMMIT:` lines among the report's last
/// [`TAIL_LINES`] non-blank lines. A line that is not exactly one of the forms is ignored.
pub fn parse(text: &str) -> Report {
    let tail: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .rev()
        .take(TAIL_LINES)
        .collect();
    let value = |key: &str| tail.iter().find_map(|l| l.strip_prefix(key)).map(str::trim);
    Report {
        verdict: value("VERDICT:").and_then(verdict_of),
        commit: value("COMMIT:").filter(|c| is_sha(c)).map(String::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_lines_carry_the_verdict_and_the_commit() {
        let text = "All checks ran.\n\nVERDICT: APPROVE\nCOMMIT: 3f573aa\n";
        assert_eq!(
            parse(text),
            Report {
                verdict: Some(Verdict::Approve),
                commit: Some("3f573aa".into())
            }
        );
        let fail = parse("notes\nCOMMIT: abc1234\nVERDICT: FAIL\n\n\n");
        assert_eq!(fail.verdict, Some(Verdict::Fail));
        assert_eq!(fail.commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn every_verdict_word_is_read_and_only_those() {
        let word = |w: &str| parse(&format!("VERDICT: {w}")).verdict;
        assert_eq!(word("REJECT"), Some(Verdict::Reject));
        assert_eq!(word("PASS"), Some(Verdict::Pass));
        assert_eq!(word("  PASS  "), Some(Verdict::Pass));
        assert_eq!(word("MAYBE"), None);
        assert_eq!(word("approve"), None);
        assert_eq!(word("PASS but flaky"), None);
        assert!(Verdict::Approve.clears() && Verdict::Pass.clears());
        assert!(!Verdict::Reject.clears() && !Verdict::Fail.clears());
    }

    #[test]
    fn a_missing_line_is_none_and_the_commit_must_look_like_a_sha() {
        assert_eq!(parse("looks fine to me"), Report::default());
        assert_eq!(parse("COMMIT: not a sha").commit, None);
        assert_eq!(parse("COMMIT: HEAD").commit, None);
        assert_eq!(parse("COMMIT: ABCDEF1").commit, None);
        assert_eq!(parse("").commit, None);
    }

    #[test]
    fn only_the_tail_counts_and_the_last_line_of_a_kind_wins() {
        let early = format!("VERDICT: PASS\n{}", "line\n".repeat(TAIL_LINES));
        assert_eq!(parse(&early).verdict, None);
        let twice = parse("VERDICT: FAIL\nCOMMIT: aaaaaaa\nVERDICT: PASS\nCOMMIT: bbbbbbb\n");
        assert_eq!(twice.verdict, Some(Verdict::Pass));
        assert_eq!(twice.commit.as_deref(), Some("bbbbbbb"));
    }
}
