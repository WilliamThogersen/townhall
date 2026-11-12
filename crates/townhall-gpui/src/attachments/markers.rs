fn marker_len(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let rest = lower.strip_prefix('[')?;
    let word = if rest.starts_with("pasted image") {
        "pasted image".len()
    } else if rest.starts_with("image") {
        "image".len()
    } else {
        return None;
    };
    let tail = &rest[word..];
    let inner = if let Some(open) = tail.strip_prefix(':') {
        open.find(']')?
    } else {
        let numbered = tail
            .trim_start()
            .strip_prefix('#')
            .unwrap_or(tail.trim_start());
        let digits = numbered.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 {
            tail.len() - numbered.len() + digits
        } else {
            0
        }
    };
    let skipped = if tail.starts_with(':') {
        1 + inner
    } else {
        inner
    };
    tail[skipped..]
        .starts_with(']')
        .then_some(1 + word + skipped + 1)
}

fn without_markers(line: &str) -> String {
    let mut out = String::new();
    let mut at = 0;
    while at < line.len() {
        let rest = &line[at..];
        match marker_len(rest) {
            Some(len) => at += len,
            None => {
                let ch = rest.chars().next().unwrap_or_default();
                out.push(ch);
                at += ch.len_utf8();
            }
        }
    }
    out
}

fn collapse_spaces(line: &str) -> String {
    let mut out = String::new();
    let mut spaces = 0;
    for ch in line.chars() {
        if ch == ' ' {
            spaces += 1;
            if spaces == 1 {
                out.push(ch);
            }
        } else {
            spaces = 0;
            out.push(ch);
        }
    }
    out
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn strip_attachment_markers(text: &str, attached_names: &[String]) -> String {
    if attached_names.is_empty() {
        return text.to_string();
    }
    let kept: Vec<String> = text
        .split('\n')
        .filter(|line| {
            let stripped = without_markers(line);
            let rest = stripped.trim();
            if rest.is_empty() && !line.trim().is_empty() {
                return false;
            }
            let one_word = !rest.chars().any(char::is_whitespace);
            !(one_word && attached_names.iter().any(|n| n == basename(rest)))
        })
        .map(|line| collapse_spaces(&without_markers(line)).trim().to_string())
        .collect();
    let mut joined = kept.join("\n");
    while joined.contains("\n\n\n") {
        joined = joined.replace("\n\n\n", "\n\n");
    }
    joined.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nothing_is_removed_when_nothing_is_attached() {
        assert_eq!(
            strip_attachment_markers("see [Image #1]", &[]),
            "see [Image #1]"
        );
    }

    #[test]
    fn image_markers_go_and_the_text_around_them_stays() {
        let out = strip_attachment_markers("look at [Image #1] now", &names(&["a.png"]));
        assert_eq!(out, "look at now");
    }

    #[test]
    fn a_line_that_is_only_a_marker_goes() {
        let out = strip_attachment_markers("it fails\n[Image #1]\nwhy?", &names(&["a.png"]));
        assert_eq!(out, "it fails\nwhy?");
    }

    #[test]
    fn pasted_image_markers_in_each_spelling_go() {
        let attached = names(&["a.png"]);
        for marker in [
            "[image]",
            "[Pasted image]",
            "[Image 2]",
            "[image: shot.png]",
        ] {
            assert_eq!(
                strip_attachment_markers(&format!("x\n{marker}\ny"), &attached),
                "x\ny",
                "{marker}"
            );
        }
    }

    #[test]
    fn a_line_that_is_only_an_attached_name_or_path_goes() {
        let attached = names(&["shot.png"]);
        assert_eq!(
            strip_attachment_markers("hi\n/tmp/dir/shot.png\nbye", &attached),
            "hi\nbye"
        );
        assert_eq!(
            strip_attachment_markers("hi\nshot.png\nbye", &attached),
            "hi\nbye"
        );
        assert_eq!(
            strip_attachment_markers("hi\nother.png\nbye", &attached),
            "hi\nother.png\nbye"
        );
    }

    #[test]
    fn blank_runs_collapse_and_the_ends_are_trimmed() {
        let out = strip_attachment_markers("a\n\n\n\n[Image #1]\n\nb\n", &names(&["a.png"]));
        assert_eq!(out, "a\n\nb");
    }
}
