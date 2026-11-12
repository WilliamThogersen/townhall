use std::ops::Range;

use crate::markdown::{Span, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockKey {
    pub entry: u64,
    pub block: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub key: BlockKey,
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    pub style: Style,
    pub link: bool,
    pub selected: bool,
}

pub fn look_runs(
    len: usize,
    spans: &[Span],
    selected: Option<Range<usize>>,
) -> Vec<(Range<usize>, Look)> {
    let selected = selected
        .map(|r| r.start.min(len)..r.end.min(len))
        .filter(|r| r.start < r.end);
    let mut runs = Vec::new();
    for span in spans {
        let range = span.range.start.min(len)..span.range.end.min(len);
        if range.start >= range.end {
            continue;
        }
        let look = |selected| Look {
            style: span.style,
            link: span.link.is_some(),
            selected,
        };
        let Some(sel) = selected.as_ref() else {
            runs.push((range, look(false)));
            continue;
        };
        let inside = range.start.max(sel.start)..range.end.min(sel.end);
        if inside.start >= inside.end {
            runs.push((range, look(false)));
            continue;
        }
        if range.start < inside.start {
            runs.push((range.start..inside.start, look(false)));
        }
        runs.push((inside.clone(), look(true)));
        if inside.end < range.end {
            runs.push((inside.end..range.end, look(false)));
        }
    }
    runs
}

pub fn clamp_to_boundary(text: &str, ix: usize) -> usize {
    let mut ix = ix.min(text.len());
    while !text.is_char_boundary(ix) {
        ix -= 1;
    }
    ix
}

pub fn selected_text(text: &str, range: Range<usize>) -> String {
    let start = clamp_to_boundary(text, range.start);
    let end = clamp_to_boundary(text, range.end);
    if start >= end {
        return String::new();
    }
    text[start..end].to_string()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn word_range(text: &str, ix: usize) -> Range<usize> {
    let ix = clamp_to_boundary(text, ix);
    let at = text[ix..].chars().next();
    let before = text[..ix].chars().next_back();
    let inside = match (at, before) {
        (Some(c), _) if is_word(c) => ix,
        (_, Some(c)) if is_word(c) => ix - c.len_utf8(),
        (Some(c), _) => return ix..ix + c.len_utf8(),
        (None, Some(c)) => return ix - c.len_utf8()..ix,
        (None, None) => return 0..0,
    };
    let start = text[..inside]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(inside, |(i, _)| i);
    let end = text[inside..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(i, _)| inside + i);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style {
        bold: false,
        italic: false,
        strike: false,
        code: false,
    };
    const BOLD: Style = Style {
        bold: true,
        italic: false,
        strike: false,
        code: false,
    };

    fn key() -> BlockKey {
        BlockKey { entry: 1, block: 0 }
    }

    fn look(style: Style, link: bool, selected: bool) -> Look {
        Look {
            style,
            link,
            selected,
        }
    }

    fn spans() -> Vec<Span> {
        vec![
            Span {
                range: 0..4,
                style: PLAIN,
                link: None,
            },
            Span {
                range: 4..8,
                style: BOLD,
                link: Some("https://x.test".into()),
            },
            Span {
                range: 8..10,
                style: PLAIN,
                link: None,
            },
        ]
    }

    #[test]
    fn the_range_is_ordered_whichever_way_the_user_dragged() {
        let forward = Selection {
            key: key(),
            anchor: 2,
            head: 7,
        };
        let backward = Selection {
            key: key(),
            anchor: 7,
            head: 2,
        };
        assert_eq!(forward.range(), 2..7);
        assert_eq!(backward.range(), 2..7);
        assert!(!forward.is_empty());
        assert!(Selection {
            key: key(),
            anchor: 3,
            head: 3
        }
        .is_empty());
    }

    #[test]
    fn without_a_selection_the_runs_follow_the_spans() {
        assert_eq!(
            look_runs(10, &spans(), None),
            [
                (0..4, look(PLAIN, false, false)),
                (4..8, look(BOLD, true, false)),
                (8..10, look(PLAIN, false, false)),
            ]
        );
    }

    #[test]
    fn a_selection_splits_the_runs_it_cuts() {
        assert_eq!(
            look_runs(10, &spans(), Some(2..6)),
            [
                (0..2, look(PLAIN, false, false)),
                (2..4, look(PLAIN, false, true)),
                (4..6, look(BOLD, true, true)),
                (6..8, look(BOLD, true, false)),
                (8..10, look(PLAIN, false, false)),
            ]
        );
    }

    #[test]
    fn an_empty_or_out_of_range_selection_changes_nothing() {
        let plain = look_runs(10, &spans(), None);
        assert_eq!(look_runs(10, &spans(), Some(3..3)), plain);
        assert_eq!(look_runs(10, &spans(), Some(12..14)), plain);
    }

    #[test]
    fn a_selection_running_past_the_end_is_cut_to_the_text() {
        let runs = look_runs(10, &spans(), Some(9..40));
        assert_eq!(runs.last().unwrap(), &(9..10, look(PLAIN, false, true)));
    }

    #[test]
    fn runs_never_overlap_and_cover_the_text() {
        for sel in [0..10, 1..9, 4..8, 3..5, 0..1] {
            let runs = look_runs(10, &spans(), Some(sel.clone()));
            let mut at = 0;
            for (range, _) in &runs {
                assert_eq!(range.start, at, "{sel:?}");
                at = range.end;
            }
            assert_eq!(at, 10, "{sel:?}");
        }
    }

    #[test]
    fn indexes_snap_back_to_a_character_boundary() {
        let text = "aø b";
        assert_eq!(clamp_to_boundary(text, 2), 1);
        assert_eq!(clamp_to_boundary(text, 3), 3);
        assert_eq!(clamp_to_boundary(text, 99), text.len());
    }

    #[test]
    fn selected_text_is_the_slice_and_survives_odd_indexes() {
        assert_eq!(selected_text("hello world", 6..11), "world");
        assert_eq!(selected_text("aø b", 0..2), "a");
        assert_eq!(selected_text("abc", 2..99), "c");
        assert_eq!(selected_text("abc", 2..2), "");
    }

    #[test]
    fn a_word_is_the_run_of_letters_digits_and_underscores_under_the_index() {
        let text = "fix the_build now";
        assert_eq!(word_range(text, 5), 4..13);
        assert_eq!(word_range(text, 0), 0..3);
        assert_eq!(word_range(text, 3), 0..3);
        assert_eq!(word_range(text, 17), 14..17);
    }

    #[test]
    fn a_space_selects_itself() {
        assert_eq!(word_range("a  b", 2), 2..3);
        assert_eq!(word_range("", 0), 0..0);
    }
}
