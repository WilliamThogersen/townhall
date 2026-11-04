//! Questions read off an agent's visible screen. Two shapes:
//! - a selection prompt at the bottom: a line ending in `?`, then options numbered from 1
//!   (Claude's permission, trust and question pickers draw this),
//! - a plain program's last line asking something (`continue? [y/n]`).

use super::question::{asks, numbered, with_options, Item};

/// The last option of a prompt sits within this many lines of the bottom.
const BOTTOM: usize = 6;
/// Lines (descriptions) allowed between two options.
const GAP: usize = 2;
/// How far above the first option the question may be.
const ABOVE: usize = 6;
const BOX: &str = "\u{2502}\u{2503}\u{2551}\u{256d}\u{256e}\u{2570}\u{256f}\u{250c}\u{2510}\u{2514}\u{2518}\u{2500}\u{2501}\u{2550}\u{254c}";

/// A line without the box drawing around it.
fn clean(line: &str) -> &str {
    line.trim_matches(|c: char| c.is_whitespace() || BOX.contains(c))
}

/// The screen's lines that have text, cleaned, top to bottom.
fn content(screen: &str) -> Vec<&str> {
    screen
        .lines()
        .map(clean)
        .filter(|l| !l.is_empty())
        .collect()
}

/// The options counting down from the one at `last` to 1, and where option 1 is.
fn options_up_to(lines: &[&str], last: usize) -> Option<(Vec<Item>, usize)> {
    let mut found = vec![numbered(lines[last])?];
    let (mut at, mut gap) = (last, 0);
    while found[0].number > 1 {
        at = at.checked_sub(1)?;
        match numbered(lines[at]) {
            Some(o) if o.number + 1 == found[0].number => {
                found.insert(0, o);
                gap = 0;
            }
            Some(_) => return None,
            None if gap < GAP => gap += 1,
            None => return None,
        }
    }
    (found.len() >= 2).then_some((found, at))
}

/// A selection prompt at the bottom of the screen, as the question with its options.
pub(crate) fn prompt(screen: &str) -> Option<String> {
    let lines = content(screen);
    let from = lines.len().saturating_sub(BOTTOM);
    let last = (from..lines.len())
        .rev()
        .find(|&i| numbered(lines[i]).is_some())?;
    let (options, first) = options_up_to(&lines, last)?;
    let question = (first.saturating_sub(ABOVE)..first)
        .rev()
        .map(|i| lines[i])
        .find(|l| asks(l))?;
    Some(with_options(question, &options))
}

/// The last line of the screen when it asks something, with the numbered options right
/// above it (a shell `select` menu).
pub(crate) fn trailing_question(screen: &str) -> Option<String> {
    let lines = content(screen);
    let (last, above) = lines.split_last()?;
    if !asks(last) {
        return None;
    }
    let listed = above
        .iter()
        .rev()
        .take_while(|l| numbered(l).is_some())
        .count();
    let block = &lines[lines.len() - 1 - listed..];
    Some(super::question::shorten(&block.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::question::choices;

    const PERMISSION: &str = "\
\u{23fa} Bash(rm -rf build)
  \u{23bf}  Running\u{2026}

\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}
 Bash command

   rm -rf build
   Remove the build folder

 Do you want to proceed?
 \u{276f} 1. Yes
   2. Yes, and don't ask again for rm commands in /w/fix-login
   3. No, and tell Claude what to do differently (esc)

 Esc to cancel
";

    const BOXED: &str = "\
\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}
\u{2502} Do you trust the files in this folder?       \u{2502}
\u{2502}                                              \u{2502}
\u{2502} /Users/me/.townhall/worktrees/k/fix-login    \u{2502}
\u{2502} Claude Code may read, write, or execute files \u{2502}
\u{2502} \u{276f} 1. Yes, proceed                          \u{2502}
\u{2502}   2. No, exit                                 \u{2502}
\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}
   Enter to confirm \u{b7} Esc to exit
";

    const PICKER: &str = "\
\u{2610} Which 2-5

\"2-5\" fits two different lists. Which one do you mean?

\u{276f} 1. Table 2-5: skip #1
     Run the server script without delayed invalidation.
  2. App items 2-4
     24 products per page.
  3. Both
  4. Type something.

Enter to select \u{b7} \u{2191}/\u{2193} to navigate \u{b7} Esc to cancel
";

    #[test]
    fn reads_claudes_permission_prompt() {
        let q = prompt(PERMISSION).unwrap();
        assert_eq!(
            q,
            "Do you want to proceed?\n1. Yes\n\
             2. Yes, and don't ask again for rm commands in /w/fix-login\n\
             3. No, and tell Claude what to do differently (esc)"
        );
        assert_eq!(choices(&q), ["1", "2", "3"]);
    }

    #[test]
    fn reads_boxed_prompts_and_pickers_with_descriptions() {
        let trust = prompt(BOXED).unwrap();
        assert!(trust.starts_with("Do you trust the files in this folder?\n1. Yes, proceed"));
        let pick = prompt(PICKER).unwrap();
        assert!(pick.starts_with("\"2-5\" fits two different lists. Which one do you mean?\n"));
        assert_eq!(choices(&pick), ["1", "2", "3", "4"]);
    }

    #[test]
    fn a_list_without_a_question_or_far_from_the_bottom_is_no_prompt() {
        assert!(prompt("Done:\n1. fixed the login\n2. added a test\n> ").is_none());
        let far = format!("Which?\n1. a\n2. b\n{}", "line\n".repeat(BOTTOM + 1));
        assert!(prompt(&far).is_none());
        assert!(prompt("Pick one?\n2. b\n3. c").is_none());
        assert!(prompt("").is_none());
    }

    #[test]
    fn trailing_question_reads_shell_prompts() {
        assert_eq!(
            trailing_question("working\ncontinue? [y/n]\n\n").unwrap(),
            "continue? [y/n]"
        );
        assert_eq!(
            trailing_question("1) postgres\n2) sqlite\n#?").unwrap(),
            "1) postgres\n2) sqlite\n#?"
        );
        assert!(trailing_question("building...\ndone").is_none());
        assert!(trailing_question("go on? (y/n) y\n").is_none(), "answered");
        assert!(trailing_question("? for shortcuts").is_none());
        assert!(trailing_question("").is_none());
    }
}
