//! An agent's question as townhall shows it: the text verbatim but short, its numbered
//! options as `N. label` lines, and the direct answers (`choices`) that can be typed back.

/// Longest question text kept; a longer one keeps its end, where the question is.
const MAX_QUESTION: usize = 400;
const MAX_LABEL: usize = 100;
const MAX_OPTIONS: usize = 9;
const ELLIPSIS: char = '\u{2026}';
/// Cursor marks a selection list draws in front of the current option.
const CURSORS: [char; 5] = ['\u{276f}', '\u{203a}', '>', '\u{25b8}', '\u{25ba}'];

/// What townhall asks for claude's folder trust dialog, and the answers it offers. The
/// dialog takes arrow keys, not text: the cursor starts on `no, exit`.
pub(crate) const TRUST_QUESTION: &str = "claude asks whether to trust this worktree";
const TRUST_CHOICES: [&str; 2] = ["yes, trust it", "no, exit"];

/// One numbered option: `1. Yes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub number: u32,
    pub label: String,
}

fn clip_end(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max - 1).collect();
    format!("{}{ELLIPSIS}", head.trim_end())
}

/// `text` trimmed; past [`MAX_QUESTION`] chars only its end is kept, after an ellipsis.
pub(crate) fn shorten(text: &str) -> String {
    let text = text.trim();
    let count = text.chars().count();
    if count <= MAX_QUESTION {
        return text.to_string();
    }
    let tail: String = text.chars().skip(count - (MAX_QUESTION - 1)).collect();
    format!("{ELLIPSIS}{}", tail.trim_start())
}

/// `line` without its leading space and selection cursor.
pub(crate) fn without_cursor(line: &str) -> &str {
    let line = line.trim_start();
    match line.strip_prefix(CURSORS) {
        Some(rest) if rest.starts_with(' ') => rest.trim_start(),
        _ => line,
    }
}

/// A line `1. label` or `2) label`, after an optional selection cursor.
pub(crate) fn numbered(line: &str) -> Option<Item> {
    let line = without_cursor(line);
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 2 {
        return None;
    }
    let rest = &line[digits.len()..];
    let label = rest.strip_prefix(['.', ')'])?.strip_prefix(' ')?.trim();
    if label.is_empty() {
        return None;
    }
    Some(Item {
        number: digits.parse().ok()?,
        label: label.to_string(),
    })
}

/// The question, then one `N. label` line per option (at most nine, labels clipped).
pub(crate) fn with_options(question: &str, options: &[Item]) -> String {
    let lines: Vec<String> = options
        .iter()
        .take(MAX_OPTIONS)
        .map(|o| format!("{}. {}", o.number, clip_end(&o.label, MAX_LABEL)))
        .collect();
    [shorten(question)]
        .into_iter()
        .chain(lines)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The two answers of a `y/n`, `[Y/n]` or `(yes/no)` marker that ends `line` (spaces, `?`
/// or `:` may follow), lowercased. Text typed after the marker means it was answered.
fn yes_no(line: &str) -> Option<Vec<String>> {
    let lower = line.to_lowercase();
    let tail = lower.trim_end_matches(|c: char| c.is_whitespace() || "?:".contains(c));
    let last = tail
        .trim_end_matches([')', ']'])
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '/'))
        .next()?;
    let (yes, no) = last.split_once('/')?;
    let fits = matches!(
        (yes, no),
        ("y", "n") | ("yes", "no") | ("y", "no") | ("yes", "n")
    );
    fits.then(|| vec![yes.to_string(), no.to_string()])
}

/// The numbers of the last run of options numbered 1, 2, 3 ... in `lines`.
fn numbers(lines: &[&str]) -> Vec<String> {
    let mut run: Vec<u32> = vec![];
    let mut last: Vec<u32> = vec![];
    for line in lines {
        let next = numbered(line).map(|o| o.number);
        if next == Some(run.len() as u32 + 1) {
            run.push(run.len() as u32 + 1);
            continue;
        }
        if run.len() >= 2 {
            last = std::mem::take(&mut run);
        }
        run = if next == Some(1) { vec![1] } else { vec![] };
    }
    let found = if run.len() >= 2 { run } else { last };
    found.iter().map(u32::to_string).collect()
}

/// What can be typed back: the numbers of the question's numbered options, else the two
/// words of a `y/n` marker. Empty when it offers neither. The trust question has its own.
pub fn choices(question: &str) -> Vec<String> {
    if question == TRUST_QUESTION {
        return TRUST_CHOICES.map(String::from).to_vec();
    }
    let lines: Vec<&str> = question.lines().collect();
    let found = numbers(&lines);
    if !found.is_empty() {
        return found;
    }
    lines
        .iter()
        .rev()
        .find_map(|l| yes_no(l))
        .unwrap_or_default()
}

/// Whether `line` ends in a question mark (markdown emphasis and quotes aside) or offers
/// `y/n`.
pub(crate) fn asks(line: &str) -> bool {
    let bare = line.trim_end_matches(|c: char| c.is_whitespace() || "*_`\"')]:".contains(c));
    bare.ends_with('?') || bare.ends_with('\u{ff1f}') || yes_no(line).is_some()
}

fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// The question a message closes with: its last paragraph when that asks something, with
/// the numbered list right before it. `None` when the message does not end in a question.
pub(crate) fn closing(text: &str) -> Option<String> {
    let paras = paragraphs(text);
    let last = paras.last()?;
    if !asks(last.lines().last().unwrap_or_default()) {
        return None;
    }
    let before = paras.len().checked_sub(2).map(|i| &paras[i]);
    let list = before.filter(|p| p.lines().filter_map(numbered).count() >= 2);
    let question = match list {
        Some(list) => format!("{list}\n{last}"),
        None => last.clone(),
    };
    Some(shorten(&question))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_lines_with_and_without_a_cursor() {
        let one = numbered("\u{276f} 1. Yes").unwrap();
        assert_eq!((one.number, one.label.as_str()), (1, "Yes"));
        assert_eq!(numbered("  2) sqlite").unwrap().number, 2);
        for not in [
            "1.Yes",
            "1. ",
            "x. no",
            "123. three digits",
            "1 MCP server failed",
        ] {
            assert!(numbered(not).is_none(), "{not}");
        }
    }

    #[test]
    fn choices_are_option_numbers_or_yes_no() {
        assert_eq!(
            choices("Do you want to proceed?\n1. Yes\n2. Yes, always\n3. No"),
            ["1", "2", "3"]
        );
        assert_eq!(choices("1) postgres\n2) sqlite\n#?"), ["1", "2"]);
        assert_eq!(choices("continue? [Y/n]"), ["y", "n"]);
        assert_eq!(choices("overwrite (yes/no)?"), ["yes", "no"]);
        assert!(choices("what next?").is_empty());
        assert!(choices("1. only one\nok?").is_empty());
        assert!(choices("read and/or write?").is_empty());
        assert_eq!(choices("read and/or write? [y/n]"), ["y", "n"]);
        assert_eq!(choices("1. a\n2. b\n1. c\nnext?"), ["1", "2"]);
    }

    #[test]
    fn the_trust_question_offers_its_own_answers() {
        assert_eq!(
            choices("claude asks whether to trust this worktree"),
            ["yes, trust it", "no, exit"]
        );
        assert_eq!(TRUST_QUESTION, "claude asks whether to trust this worktree");
    }

    #[test]
    fn a_yes_no_marker_asks_only_at_the_end_of_the_line() {
        for asking in [
            "go on? (y/n) ",
            "proceed [Y/n]:",
            "overwrite (yes/no)?",
            "keep y/n",
        ] {
            assert!(asks(asking), "{asking}");
        }
        for answered in [
            "go on? (y/n) y",
            "keep [y/n] no thanks",
            "the y/n flag is set",
        ] {
            assert!(!asks(answered), "{answered}");
            assert!(choices(answered).is_empty(), "{answered}");
        }
    }

    #[test]
    fn asks_sees_question_marks_through_markdown() {
        assert!(asks("Should I also update the docs?"));
        assert!(asks("**Which one?**"));
        assert!(asks("proceed? [y/n] "));
        assert!(!asks("Done. The tests pass."));
        assert!(!asks("? for shortcuts"));
    }

    #[test]
    fn closing_takes_the_last_paragraph_and_its_list() {
        let text = "I looked at it.\n\n1. Patch the middleware\n2. Change the cookie\n\nWhich do you prefer?";
        assert_eq!(
            closing(text).unwrap(),
            "1. Patch the middleware\n2. Change the cookie\nWhich do you prefer?"
        );
        assert_eq!(
            closing("Fixed.\n\nShould I push?").unwrap(),
            "Should I push?"
        );
        assert!(closing("All done, the tests pass.").is_none());
        assert!(closing("").is_none());
    }

    #[test]
    fn long_text_keeps_its_end() {
        let long = format!("{}why?", "a".repeat(1000));
        let short = shorten(&long);
        assert_eq!(short.chars().count(), MAX_QUESTION);
        assert!(short.starts_with(ELLIPSIS) && short.ends_with("why?"));
        assert_eq!(shorten("  ok?  "), "ok?");
    }

    #[test]
    fn options_are_listed_under_the_question() {
        let opts = [
            Item {
                number: 1,
                label: "Yes".into(),
            },
            Item {
                number: 2,
                label: "x".repeat(150),
            },
        ];
        let text = with_options("Proceed?", &opts);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0..2], ["Proceed?", "1. Yes"]);
        assert_eq!(lines[2].chars().count(), 3 + MAX_LABEL);
        assert_eq!(choices(&text), ["1", "2"]);
    }
}
