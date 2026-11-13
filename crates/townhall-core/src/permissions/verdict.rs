#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Allow,
    Deny,
    AskUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub kind: Kind,
    pub reason: String,
}

const HEADS: [(&str, Kind); 5] = [
    ("NEEDS THE USER", Kind::AskUser),
    ("ASK USER", Kind::AskUser),
    ("ASK-USER", Kind::AskUser),
    ("ALLOW", Kind::Allow),
    ("DENY", Kind::Deny),
];
const RULE_CHARS: &str = "\u{2500}\u{2501}\u{2550}\u{254c}-=_ ";
const SCREEN_LINES: usize = 15;
const SHELL_BREAK: [char; 8] = [';', '|', '&', '<', '>', '`', '$', '('];

pub fn parse(answer: &str) -> Verdict {
    let mut lines = answer
        .lines()
        .map(|l| l.trim_matches(|c: char| c.is_whitespace() || "*_`#>".contains(c)))
        .filter(|l| !l.is_empty());
    let Some(first) = lines.next() else {
        return unclear("the adviser gave no answer");
    };
    let upper = first.to_uppercase();
    for (head, kind) in HEADS {
        let Some(rest) = upper.strip_prefix(head) else {
            continue;
        };
        if rest.chars().next().is_some_and(|c| c.is_alphanumeric()) {
            continue;
        }
        let rest = first[head.len()..]
            .trim_start_matches(|c: char| c.is_whitespace() || ":-*_\u{2014}".contains(c))
            .trim();
        let reason = match rest.is_empty() {
            true => lines.next().unwrap_or("").trim().to_string(),
            false => rest.to_string(),
        };
        return Verdict { kind, reason };
    }
    unclear(&format!("the adviser gave no clear verdict: {first}"))
}

fn unclear(reason: &str) -> Verdict {
    Verdict {
        kind: Kind::AskUser,
        reason: reason.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub command: String,
    pub allow_key: String,
    pub deny_key: String,
}

fn option_of(line: &str) -> Option<(String, String)> {
    let line = line
        .trim()
        .trim_start_matches(|c: char| "\u{276f}\u{203a}>".contains(c));
    let (number, label) = line.trim().split_once(['.', ')'])?;
    let valid =
        !number.is_empty() && number.len() <= 2 && number.chars().all(|c| c.is_ascii_digit());
    valid.then(|| (number.to_string(), label.trim().to_string()))
}

fn is_rule(line: &str) -> bool {
    line.chars().all(|c| RULE_CHARS.contains(c))
}

fn screen_command(screen: &str) -> String {
    let lines: Vec<&str> = screen.lines().map(str::trim).collect();
    let Some(at) = lines.iter().rposition(|l| l.starts_with("Do you want")) else {
        return String::new();
    };
    let above = lines[..at]
        .iter()
        .rev()
        .take(SCREEN_LINES)
        .take_while(|l| l.is_empty() || (!is_rule(l) && !l.starts_with('\u{23fa}')))
        .filter(|l| !l.is_empty());
    let mut kept: Vec<&str> = above.copied().collect();
    kept.reverse();
    kept.join("\n")
}

pub fn prompt_of(question: &str, screen: Option<&str>) -> Option<Prompt> {
    let lines: Vec<&str> = question
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let at = lines.iter().position(|l| l.starts_with("Do you want"))?;
    let options: Vec<(String, String)> = lines[at + 1..]
        .iter()
        .filter_map(|l| option_of(l))
        .collect();
    let starts = |label: &str, word: &str| label.to_lowercase().starts_with(word);
    let allow = options
        .iter()
        .find(|(_, l)| starts(l, "yes") && !l.to_lowercase().contains("don't ask again"))?;
    let deny = options.iter().rev().find(|(_, l)| starts(l, "no"))?;
    let mut command = lines[..at].join("\n");
    if command.is_empty() {
        command = screen.map(screen_command).unwrap_or_default();
    }
    (!command.is_empty()).then(|| Prompt {
        command,
        allow_key: allow.0.clone(),
        deny_key: deny.0.clone(),
    })
}

pub fn pattern_of(command: &str) -> String {
    let first = command.lines().next().unwrap_or("");
    let Some((tool, what)) = first.split_once(": ") else {
        return String::new();
    };
    let named = !tool.is_empty() && tool.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !named || what.trim().is_empty() {
        return String::new();
    }
    if tool != "Bash" {
        return tool.to_string();
    }
    let head = what.split(SHELL_BREAK).next().unwrap_or("");
    let mut words = head.split_whitespace().skip_while(|w| is_assignment(w));
    let Some(program) = words.next() else {
        return String::new();
    };
    let program_ok = program
        .chars()
        .all(|c| c.is_alphanumeric() || "_./+-".contains(c));
    if !program_ok {
        return String::new();
    }
    let sub = words
        .next()
        .filter(|w| w.starts_with(|c: char| c.is_ascii_alphabetic()))
        .filter(|w| {
            w.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        });
    match sub {
        Some(sub) => format!("Bash({program} {sub} *)"),
        None => format!("Bash({program} *)"),
    }
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(answer: &str) -> (Kind, String) {
        let v = parse(answer);
        (v.kind, v.reason)
    }

    #[test]
    fn the_first_line_is_the_verdict_and_the_rest_its_reason() {
        assert_eq!(
            verdict("ALLOW: a read-only git command in its own worktree"),
            (
                Kind::Allow,
                "a read-only git command in its own worktree".into()
            )
        );
        assert_eq!(
            verdict("DENY: it would wipe the build cache the next task needs\nmore text"),
            (
                Kind::Deny,
                "it would wipe the build cache the next task needs".into()
            )
        );
        assert_eq!(
            verdict("ASK USER: it publishes to npm"),
            (Kind::AskUser, "it publishes to npm".into())
        );
        assert_eq!(
            verdict("NEEDS THE USER: which registry?"),
            (Kind::AskUser, "which registry?".into())
        );
    }

    #[test]
    fn markup_and_case_around_the_verdict_are_ignored() {
        assert_eq!(verdict("**Allow** - tests only").0, Kind::Allow);
        assert_eq!(verdict("\n\n  deny: no\n").1, "no");
        assert_eq!(
            verdict("DENY\nuse cargo check instead").1,
            "use cargo check instead"
        );
        assert_eq!(verdict("ask-user: unsure").0, Kind::AskUser);
    }

    #[test]
    fn an_answer_without_a_clear_verdict_asks_the_user() {
        for answer in ["", "Sure, go ahead", "I think this is fine", "ALLOWED"] {
            let v = parse(answer);
            assert_eq!(v.kind, Kind::AskUser, "{answer}");
            assert!(!v.reason.is_empty(), "{answer}");
        }
    }

    const PENDING: &str = "Bash: rm -rf build\nDo you want to proceed?\n1. Yes\n2. Yes, and don't ask again for rm commands in /w/fix\n3. No, and tell Claude what to do differently (esc)";

    #[test]
    fn a_permission_prompt_names_its_command_and_its_yes_and_no_keys() {
        let p = prompt_of(PENDING, None).unwrap();
        assert_eq!(p.command, "Bash: rm -rf build");
        assert_eq!((p.allow_key.as_str(), p.deny_key.as_str()), ("1", "3"));
        let two = "Edit: src/a.rs\nDo you want to make this edit to src/a.rs?\n1. Yes\n2. No";
        let p = prompt_of(two, None).unwrap();
        assert_eq!((p.allow_key.as_str(), p.deny_key.as_str()), ("1", "2"));
    }

    #[test]
    fn without_a_call_line_the_command_is_read_from_the_screen() {
        let question = "Do you want to proceed?\n1. Yes\n2. No";
        let screen = "\u{23fa} Bash(ls)\n\u{2500}\u{2500}\u{2500}\u{2500}\n Bash command\n\n   git push --force origin main\n   Push\n\n Do you want to proceed?\n \u{276f} 1. Yes\n   2. No\n";
        let p = prompt_of(question, Some(screen)).unwrap();
        assert!(
            p.command.contains("git push --force origin main"),
            "{}",
            p.command
        );
        assert!(!p.command.contains("Bash(ls)"), "{}", p.command);
    }

    #[test]
    fn other_questions_are_no_permission_prompts() {
        for question in [
            "Which one do you mean?\n1. a\n2. b",
            "Do you want to proceed?\n1. Maybe\n2. Later",
            "continue? [y/n]",
            "Do you trust the files in this folder?\n1. Yes, proceed\n2. No, exit",
            "",
        ] {
            assert_eq!(prompt_of(question, None), None, "{question}");
        }
    }

    #[test]
    fn a_command_becomes_a_tool_pattern() {
        for (command, pattern) in [
            ("Bash: git status -s", "Bash(git status *)"),
            ("Bash: cargo test -p x && rm -rf y", "Bash(cargo test *)"),
            ("Bash: ls", "Bash(ls *)"),
            ("Bash: FOO=1 npm run build", "Bash(npm run *)"),
            (
                "Bash: ./scripts/demo.sh --fast",
                "Bash(./scripts/demo.sh *)",
            ),
            ("Edit: src/a.rs", "Edit"),
            ("WebFetch: https://example.com", "WebFetch"),
            ("Bash: $(evil)", ""),
            ("no call line", ""),
        ] {
            assert_eq!(pattern_of(command), pattern, "{command}");
        }
    }
}
