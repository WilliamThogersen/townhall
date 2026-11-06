//! What a tester's or reviewer's report says: `VERDICT: APPROVE|REJECT` (reviewer),
//! `VERDICT: PASS|FAIL` (tester) and `COMMIT: <sha>` (the full 40 hex characters), each on a line of its own anywhere in
//! the final message.

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

/// The only agent types a `requires` may name, each with the verdict words its report may use.
static VOCABULARY: [(&str, [Verdict; 2]); 2] = [
    ("tester", [Verdict::Pass, Verdict::Fail]),
    ("reviewer", [Verdict::Approve, Verdict::Reject]),
];

/// The verdict words `agent_type` may use (ignoring case), or `None` for a type that cannot be
/// required.
pub fn vocabulary(agent_type: &str) -> Option<&'static [Verdict]> {
    VOCABULARY
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(agent_type))
        .map(|(_, words)| words.as_slice())
}

/// Why `agent_type` cannot be required, or `None` when it can.
pub fn unrequirable(agent_type: &str) -> Option<String> {
    if vocabulary(agent_type).is_some() {
        return None;
    }
    let names: Vec<&str> = VOCABULARY.iter().map(|(name, _)| *name).collect();
    Some(format!(
        "`{agent_type}` cannot be required: it has no verdict words, only {} can be",
        names.join(" and ")
    ))
}

/// What a report says. Each field is `Err` with the reason when its lines cannot be trusted
/// and `Ok(None)` when there is no such line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub verdict: Result<Option<Verdict>, String>,
    pub commit: Result<Option<String>, String>,
}

fn is_sha(word: &str) -> bool {
    word.len() == 40 && word.chars().all(|c| c.is_ascii_hexdigit())
}

/// A fence's character and length: a run of at least three backticks or tildes.
type Fence = (char, usize);

fn fence_run(line: &str) -> Option<(Fence, &str)> {
    let line = line.trim_start();
    let mark = line.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let rest = line.trim_start_matches(mark);
    let len = line.len() - rest.len();
    (len >= 3).then_some(((mark, len), rest))
}

fn opens_fence(line: &str) -> Option<Fence> {
    fence_run(line).map(|(fence, _)| fence)
}

fn closes_fence(line: &str, open: Fence) -> bool {
    match fence_run(line) {
        Some(((mark, len), rest)) => mark == open.0 && len >= open.1 && rest.trim().is_empty(),
        None => false,
    }
}

fn is_quote(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn distinct<'a>(found: &[&'a str]) -> Vec<&'a str> {
    let mut out: Vec<&str> = vec![];
    for word in found {
        if !out.contains(word) {
            out.push(word);
        }
    }
    out
}

fn single<'a>(key: &str, found: &[&'a str]) -> Result<Option<&'a str>, String> {
    match distinct(found).as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(format!("conflicting {key} lines: {}", many.join(", "))),
    }
}

fn verdict_of(word: &str, allowed: &[Verdict]) -> Result<Verdict, String> {
    match allowed.iter().find(|v| v.as_str() == word) {
        Some(v) => Ok(*v),
        None => {
            let words: Vec<&str> = allowed.iter().map(|v| v.as_str()).collect();
            Err(format!(
                "`VERDICT: {word}` is not one of {}",
                words.join(", ")
            ))
        }
    }
}

/// The verdict and commit of the report's `VERDICT: <WORD>` and `COMMIT: <sha>` lines,
/// scanned over the whole text. A line counts only when it starts at the first column, holds
/// nothing after the value but whitespace, and is neither inside a fenced code block nor in a
/// blockquote; any other line is ignored. A fence opens with three or more backticks or
/// tildes and closes only with the same character, at least as many, and nothing else on the
/// line; one never closed runs to the end. A line right after a `>` line, with no blank line
/// between, is taken as quoted too. Identical repeats are one line; different ones are
/// a conflict. The verdict word must belong to `agent_type`'s [`vocabulary`]; a type without
/// one gets an error for any verdict word.
pub fn parse(text: &str, agent_type: &str) -> Report {
    let mut verdicts = vec![];
    let mut commits = vec![];
    let mut fence: Option<Fence> = None;
    let mut quoting = false;
    for line in text.lines() {
        if let Some(open) = fence {
            if closes_fence(line, open) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = opens_fence(line) {
            fence = Some(open);
            quoting = false;
            continue;
        }
        if line.trim().is_empty() {
            quoting = false;
            continue;
        }
        quoting = quoting || is_quote(line);
        if quoting {
            continue;
        }
        let line = line.trim_end();
        if let Some(word) = line.strip_prefix("VERDICT: ") {
            if !word.is_empty() && !word.contains(char::is_whitespace) {
                verdicts.push(word);
            }
        } else if let Some(sha) = line.strip_prefix("COMMIT: ") {
            if is_sha(sha) {
                commits.push(sha);
            }
        }
    }
    Report {
        verdict: single("VERDICT", &verdicts).and_then(|word| {
            let Some(word) = word else { return Ok(None) };
            match vocabulary(agent_type) {
                Some(allowed) => verdict_of(word, allowed).map(Some),
                None => Err(unrequirable(agent_type).unwrap_or_default()),
            }
        }),
        commit: single("COMMIT", &commits).map(|sha| sha.map(String::from)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "3f573aa3f573aa3f573aa3f573aa3f573aa3f573";
    const B: &str = "abc1234abc1234abc1234abc1234abc1234abc12";

    fn verdict(text: &str, agent: &str) -> Result<Option<Verdict>, String> {
        parse(text, agent).verdict
    }

    fn commit(text: &str) -> Result<Option<String>, String> {
        parse(text, "tester").commit
    }

    #[test]
    fn the_verdict_and_the_commit_may_come_first_or_last() {
        let last = parse(
            &format!("All checks ran.\n\nVERDICT: APPROVE\nCOMMIT: {A}\n"),
            "reviewer",
        );
        assert_eq!(last.verdict, Ok(Some(Verdict::Approve)));
        assert_eq!(last.commit, Ok(Some(A.into())));
        let long = "line\n".repeat(200);
        let first = parse(&format!("VERDICT: FAIL\nCOMMIT: {B}\n{long}"), "tester");
        assert_eq!(first.verdict, Ok(Some(Verdict::Fail)));
        assert_eq!(first.commit, Ok(Some(B.into())));
        let middle = parse(&format!("{long}VERDICT: PASS\n{long}"), "tester");
        assert_eq!(middle.verdict, Ok(Some(Verdict::Pass)));
    }

    #[test]
    fn a_word_must_be_in_the_required_types_vocabulary() {
        assert_eq!(verdict("VERDICT: PASS", "tester"), Ok(Some(Verdict::Pass)));
        assert_eq!(verdict("VERDICT: FAIL", "Tester"), Ok(Some(Verdict::Fail)));
        assert_eq!(
            verdict("VERDICT: REJECT", "reviewer"),
            Ok(Some(Verdict::Reject))
        );
        let wrong = verdict("VERDICT: APPROVE", "tester").unwrap_err();
        assert!(
            wrong.contains("APPROVE") && wrong.contains("PASS, FAIL"),
            "{wrong}"
        );
        assert!(verdict("VERDICT: PASS", "reviewer").is_err());
        assert!(verdict("VERDICT: MAYBE", "tester").is_err());
        assert!(verdict("VERDICT: approve", "reviewer").is_err());
        assert!(Verdict::Approve.clears() && Verdict::Pass.clears());
        assert!(!Verdict::Reject.clears() && !Verdict::Fail.clears());
    }

    #[test]
    fn only_tester_and_reviewer_have_a_vocabulary() {
        assert_eq!(
            vocabulary("Tester"),
            Some(&[Verdict::Pass, Verdict::Fail][..])
        );
        assert_eq!(
            vocabulary("REVIEWER"),
            Some(&[Verdict::Approve, Verdict::Reject][..])
        );
        assert!(unrequirable("tester").is_none() && unrequirable("reviewer").is_none());
        let why = unrequirable("coder").unwrap();
        assert!(
            why.contains("`coder`") && why.contains("tester and reviewer"),
            "{why}"
        );
        for word in ["PASS", "FAIL", "APPROVE", "REJECT"] {
            let text = format!("VERDICT: {word}");
            assert!(verdict(&text, "coder").is_err(), "{word}");
            assert!(verdict(&text, "").is_err(), "{word}");
        }
        assert_eq!(verdict("no verdict line", "coder"), Ok(None));
    }

    #[test]
    fn trailing_whitespace_is_fine_but_nothing_else_around_the_line() {
        assert_eq!(
            verdict("VERDICT: PASS  \t\r\n", "tester"),
            Ok(Some(Verdict::Pass))
        );
        for text in [
            " VERDICT: PASS",
            "\tVERDICT: PASS",
            "- VERDICT: PASS",
            "**VERDICT: PASS**",
            "VERDICT: PASS but flaky",
            "VERDICT:PASS",
            "VERDICT:  PASS",
            "verdict: PASS",
            "VERDICT: ",
        ] {
            assert_eq!(verdict(text, "tester"), Ok(None), "{text:?}");
        }
    }

    #[test]
    fn fenced_and_quoted_lines_do_not_count() {
        let fenced = format!("```\nVERDICT: PASS\nCOMMIT: {A}\n```\n");
        assert_eq!(parse(&fenced, "tester").verdict, Ok(None));
        assert_eq!(commit(&fenced), Ok(None));
        let tilde = "~~~text\nVERDICT: PASS\n~~~\nVERDICT: FAIL\n";
        assert_eq!(verdict(tilde, "tester"), Ok(Some(Verdict::Fail)));
        let quoted = format!("> VERDICT: PASS\n>COMMIT: {A}\n  > VERDICT: PASS\n");
        assert_eq!(parse(&quoted, "tester").verdict, Ok(None));
        assert_eq!(commit(&quoted), Ok(None));
        let after = "```\nVERDICT: FAIL\n```\nVERDICT: PASS\n";
        assert_eq!(verdict(after, "tester"), Ok(Some(Verdict::Pass)));
    }

    #[test]
    fn a_fence_closes_only_with_its_own_character_and_at_least_its_length() {
        let tilde_inside = "```\n~~~\nVERDICT: PASS\n```\nVERDICT: FAIL\n";
        assert_eq!(verdict(tilde_inside, "tester"), Ok(Some(Verdict::Fail)));
        let backtick_inside = "~~~\n```\nVERDICT: PASS\n~~~\nVERDICT: FAIL\n";
        assert_eq!(verdict(backtick_inside, "tester"), Ok(Some(Verdict::Fail)));
        let shorter = "````\n```\nVERDICT: PASS\n````\nVERDICT: FAIL\n";
        assert_eq!(verdict(shorter, "tester"), Ok(Some(Verdict::Fail)));
        let longer = "```\nVERDICT: PASS\n`````\nVERDICT: FAIL\n";
        assert_eq!(verdict(longer, "tester"), Ok(Some(Verdict::Fail)));
        let text_after = "```\n``` VERDICT: PASS\nVERDICT: FAIL\n```\nVERDICT: PASS\n";
        assert_eq!(verdict(text_after, "tester"), Ok(Some(Verdict::Pass)));
    }

    #[test]
    fn a_verdict_after_an_unterminated_fence_does_not_count() {
        for open in ["```", "~~~", "````text"] {
            let text = format!("{open}\nsome output\nVERDICT: PASS\nCOMMIT: {A}\n");
            assert_eq!(parse(&text, "tester").verdict, Ok(None), "{open}");
            assert_eq!(commit(&text), Ok(None), "{open}");
        }
        let after = format!("```\nunclosed\nVERDICT: PASS\nCOMMIT: {A}\n");
        assert_eq!(parse(&after, "tester").verdict, Ok(None));
    }

    #[test]
    fn a_line_right_after_a_quote_is_quoted_until_a_blank_line() {
        let lazy = format!("> quoted\nVERDICT: PASS\nCOMMIT: {A}\n");
        assert_eq!(verdict(&lazy, "tester"), Ok(None));
        assert_eq!(commit(&lazy), Ok(None));
        let long = "> a\nlazy\nVERDICT: PASS\n";
        assert_eq!(verdict(long, "tester"), Ok(None));
        let blank = "> quoted\n\nVERDICT: PASS\n";
        assert_eq!(verdict(blank, "tester"), Ok(Some(Verdict::Pass)));
        let before = "VERDICT: PASS\n> quoted\n";
        assert_eq!(verdict(before, "tester"), Ok(Some(Verdict::Pass)));
        let fenced = "> quoted\n```\nVERDICT: FAIL\n```\n\nVERDICT: PASS\n";
        assert_eq!(verdict(fenced, "tester"), Ok(Some(Verdict::Pass)));
    }

    #[test]
    fn different_lines_conflict_and_identical_repeats_do_not() {
        let conflict = verdict("VERDICT: FAIL\nnote\nVERDICT: PASS\n", "tester").unwrap_err();
        assert_eq!(conflict, "conflicting VERDICT lines: FAIL, PASS");
        let same = format!("VERDICT: PASS\nCOMMIT: {A}\nVERDICT: PASS\nCOMMIT: {A}\n");
        assert_eq!(verdict(&same, "tester"), Ok(Some(Verdict::Pass)));
        assert_eq!(commit(&same), Ok(Some(A.into())));
        let commits = commit(&format!("COMMIT: {A}\nCOMMIT: {B}\n")).unwrap_err();
        assert_eq!(commits, format!("conflicting COMMIT lines: {A}, {B}"));
    }

    #[test]
    fn a_missing_line_is_none_and_the_commit_must_look_like_a_sha() {
        assert_eq!(parse("looks fine to me", "tester").verdict, Ok(None));
        assert_eq!(commit("looks fine to me"), Ok(None));
        assert_eq!(commit("COMMIT: not a sha"), Ok(None));
        assert_eq!(commit("COMMIT: HEAD"), Ok(None));
        assert_eq!(commit("COMMIT: abc123"), Ok(None));
        assert_eq!(commit("COMMIT: 3f573aa"), Ok(None));
        assert_eq!(commit(&format!("COMMIT: {}", &A[..39])), Ok(None));
        assert_eq!(commit(&format!("COMMIT: {A}0")), Ok(None));
        assert_eq!(commit(&format!("COMMIT: {}", "z".repeat(40))), Ok(None));
        let upper = A.to_uppercase();
        assert_eq!(commit(&format!("COMMIT: {upper}")), Ok(Some(upper)));
        assert_eq!(commit(""), Ok(None));
    }
}
