use std::iter::Peekable;
use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Inline),
    Heading {
        level: u8,
        text: Inline,
    },
    Code {
        lang: Option<String>,
        text: String,
    },
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Table {
        align: Vec<Align>,
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
    Rule,
}

pub fn safe_href(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    ["http:", "https:", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
        .then(|| trimmed.to_string())
}

type Events<'a> = Peekable<Parser<'a>>;

pub fn parse(text: &str) -> Vec<Block> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut events = Parser::new_ext(text, options).peekable();
    let mut blocks = Vec::new();
    while events.peek().is_some() {
        blocks.extend(read_blocks(&mut events));
        if matches!(events.peek(), Some(Event::End(_))) {
            events.next();
        }
    }
    blocks
}

fn is_inline_start(event: &Event) -> bool {
    match event {
        Event::Text(_)
        | Event::Code(_)
        | Event::InlineHtml(_)
        | Event::SoftBreak
        | Event::HardBreak => true,
        Event::Start(tag) => matches!(
            tag,
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
        ),
        _ => false,
    }
}

fn read_blocks(events: &mut Events) -> Vec<Block> {
    let mut blocks = Vec::new();
    while let Some(event) = events.peek() {
        if matches!(event, Event::End(_)) {
            break;
        }
        if is_inline_start(event) {
            let inline = read_inline(events);
            if !inline.text.is_empty() {
                blocks.push(Block::Paragraph(inline));
            }
            continue;
        }
        let Some(event) = events.next() else { break };
        match event {
            Event::Start(Tag::Paragraph) => {
                let inline = read_inline(events);
                events.next();
                blocks.push(Block::Paragraph(inline));
            }
            Event::Start(Tag::Heading { level, .. }) => {
                let text = read_inline(events);
                events.next();
                blocks.push(Block::Heading {
                    level: heading_level(level),
                    text,
                });
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let mut text = String::new();
                while let Some(Event::Text(chunk)) = events.peek() {
                    text.push_str(chunk);
                    events.next();
                }
                events.next();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(String::from),
                    CodeBlockKind::Indented => None,
                };
                blocks.push(Block::Code {
                    lang,
                    text: text.strip_suffix('\n').unwrap_or(&text).to_string(),
                });
            }
            Event::Start(Tag::HtmlBlock) => {
                let mut text = String::new();
                while let Some(Event::Html(chunk)) = events.peek() {
                    text.push_str(chunk);
                    events.next();
                }
                events.next();
                let text = text.trim_end_matches('\n').to_string();
                blocks.push(Block::Paragraph(plain(&text)));
            }
            Event::Start(Tag::BlockQuote(_)) => {
                let inner = read_blocks(events);
                events.next();
                blocks.push(Block::Quote(inner));
            }
            Event::Start(Tag::List(start)) => {
                let mut items = Vec::new();
                while matches!(events.peek(), Some(Event::Start(Tag::Item))) {
                    events.next();
                    let task = match events.peek() {
                        Some(Event::TaskListMarker(done)) => {
                            let done = *done;
                            events.next();
                            Some(done)
                        }
                        _ => None,
                    };
                    let inner = read_blocks(events);
                    events.next();
                    items.push(ListItem {
                        task,
                        blocks: inner,
                    });
                }
                events.next();
                blocks.push(Block::List { start, items });
            }
            Event::Start(Tag::Table(aligns)) => blocks.push(read_table(events, &aligns)),
            Event::Rule => blocks.push(Block::Rule),
            _ => {}
        }
    }
    blocks
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn read_table(events: &mut Events, aligns: &[Alignment]) -> Block {
    let align = aligns
        .iter()
        .map(|a| match a {
            Alignment::Center => Align::Center,
            Alignment::Right => Align::Right,
            Alignment::Left | Alignment::None => Align::Left,
        })
        .collect();
    let mut head = Vec::new();
    let mut rows: Vec<Vec<Inline>> = Vec::new();
    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::TableHead) => head = read_cells(events),
            Event::Start(Tag::TableRow) => rows.push(read_cells(events)),
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    Block::Table { align, head, rows }
}

fn read_cells(events: &mut Events) -> Vec<Inline> {
    let mut cells = Vec::new();
    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::TableCell) => {
                cells.push(read_inline(events));
                events.next();
            }
            Event::End(_) => break,
            _ => {}
        }
    }
    cells
}

struct Piece {
    text: String,
    style: Style,
    link: Option<String>,
}

fn plain(text: &str) -> Inline {
    finish(vec![Piece {
        text: text.to_string(),
        style: Style::default(),
        link: None,
    }])
}

fn read_inline(events: &mut Events) -> Inline {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut style = Style::default();
    let mut bold = 0u32;
    let mut italic = 0u32;
    let mut strike = 0u32;
    let mut links: Vec<Option<String>> = Vec::new();
    let mut push = |text: &str, style: Style, link: Option<String>| {
        pieces.push(Piece {
            text: text.to_string(),
            style,
            link,
        })
    };
    while let Some(event) = events.peek() {
        match event {
            Event::Text(text) => {
                let link = links.last().cloned().flatten();
                push(text, style, link);
            }
            Event::Code(text) => {
                let link = links.last().cloned().flatten();
                push(
                    text,
                    Style {
                        code: true,
                        ..style
                    },
                    link,
                );
            }
            Event::InlineHtml(text) | Event::Html(text) => {
                let link = links.last().cloned().flatten();
                push(text, style, link);
            }
            Event::SoftBreak | Event::HardBreak => push("\n", style, None),
            Event::Start(Tag::Emphasis) => {
                italic += 1;
                style.italic = true;
            }
            Event::Start(Tag::Strong) => {
                bold += 1;
                style.bold = true;
            }
            Event::Start(Tag::Strikethrough) => {
                strike += 1;
                style.strike = true;
            }
            Event::Start(Tag::Link { dest_url, .. }) => links.push(safe_href(dest_url)),
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => {}
            Event::End(TagEnd::Emphasis) => {
                italic = italic.saturating_sub(1);
                style.italic = italic > 0;
            }
            Event::End(TagEnd::Strong) => {
                bold = bold.saturating_sub(1);
                style.bold = bold > 0;
            }
            Event::End(TagEnd::Strikethrough) => {
                strike = strike.saturating_sub(1);
                style.strike = strike > 0;
            }
            Event::End(TagEnd::Link) => {
                links.pop();
            }
            _ => break,
        }
        events.next();
    }
    finish(pieces)
}

fn finish(pieces: Vec<Piece>) -> Inline {
    let mut text = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut add = |piece_text: &str, style: Style, link: Option<String>| {
        if piece_text.is_empty() {
            return;
        }
        let start = text.len();
        text.push_str(piece_text);
        let range = start..text.len();
        match spans.last_mut() {
            Some(last) if last.style == style && last.link == link => last.range.end = range.end,
            _ => spans.push(Span { range, style, link }),
        }
    };
    for piece in pieces {
        if piece.link.is_some() || piece.style.code {
            add(&piece.text, piece.style, piece.link);
            continue;
        }
        for (part, link) in split_bare_links(&piece.text) {
            add(part, piece.style, link);
        }
    }
    Inline { text, spans }
}

fn split_bare_links(text: &str) -> Vec<(&str, Option<String>)> {
    let mut parts = Vec::new();
    let mut cursor = 0;
    let mut search = 0;
    while let Some(found) = find_scheme(&text[search..]) {
        let start = search + found;
        let end = link_end(text, start);
        if end == start {
            search = start + 1;
            continue;
        }
        if cursor < start {
            parts.push((&text[cursor..start], None));
        }
        parts.push((&text[start..end], safe_href(&text[start..end])));
        cursor = end;
        search = end;
    }
    if cursor < text.len() {
        parts.push((&text[cursor..], None));
    }
    parts
}

fn find_scheme(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let at = |needle: &str| lower.find(needle);
    match (at("https://"), at("http://")) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn link_end(text: &str, start: usize) -> usize {
    let tail = &text[start..];
    let stop = tail
        .find(|c: char| c.is_whitespace() || c == '<')
        .unwrap_or(tail.len());
    let mut end = start + stop;
    loop {
        let url = &text[start..end];
        let trim = match url.chars().last() {
            Some('.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"') => true,
            Some(')') => url.matches('(').count() < url.matches(')').count(),
            _ => false,
        };
        if !trim {
            break;
        }
        end -= 1;
    }
    let scheme = if text[start..].to_ascii_lowercase().starts_with("https://") {
        "https://".len()
    } else {
        "http://".len()
    };
    if end - start <= scheme {
        start
    } else {
        end
    }
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

    fn styled(f: impl Fn(&mut Style)) -> Style {
        let mut s = PLAIN;
        f(&mut s);
        s
    }

    fn span(range: Range<usize>, style: Style, link: Option<&str>) -> Span {
        Span {
            range,
            style,
            link: link.map(String::from),
        }
    }

    fn only_paragraph(text: &str) -> Inline {
        match parse(text).as_slice() {
            [Block::Paragraph(inline)] => inline.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_web_and_mail_links_are_safe() {
        let table: &[(&str, Option<&str>)] = &[
            ("https://example.com/a", Some("https://example.com/a")),
            ("HTTP://example.com", Some("HTTP://example.com")),
            ("  mailto:a@b.c ", Some("mailto:a@b.c")),
            ("javascript:alert(1)", None),
            ("file:///etc/passwd", None),
            ("data:text/html,x", None),
            ("/relative/path", None),
            ("#anchor", None),
            ("", None),
        ];
        for (url, want) in table {
            assert_eq!(safe_href(url).as_deref(), *want, "{url}");
        }
    }

    #[test]
    fn a_paragraph_keeps_inline_styles_as_spans_covering_the_text() {
        let p = only_paragraph("a **b** *c* ~~d~~ `e` f");
        assert_eq!(p.text, "a b c d e f");
        assert_eq!(
            p.spans,
            [
                span(0..2, PLAIN, None),
                span(2..3, styled(|s| s.bold = true), None),
                span(3..4, PLAIN, None),
                span(4..5, styled(|s| s.italic = true), None),
                span(5..6, PLAIN, None),
                span(6..7, styled(|s| s.strike = true), None),
                span(7..8, PLAIN, None),
                span(8..9, styled(|s| s.code = true), None),
                span(9..11, PLAIN, None),
            ]
        );
    }

    #[test]
    fn nested_styles_combine() {
        let p = only_paragraph("***both***");
        assert_eq!(p.text, "both");
        assert_eq!(
            p.spans,
            [span(
                0..4,
                styled(|s| {
                    s.bold = true;
                    s.italic = true
                }),
                None
            )]
        );
    }

    #[test]
    fn soft_and_hard_breaks_are_line_breaks() {
        assert_eq!(only_paragraph("one\ntwo").text, "one\ntwo");
        assert_eq!(only_paragraph("one  \ntwo").text, "one\ntwo");
    }

    #[test]
    fn a_safe_link_carries_its_url_and_an_unsafe_one_is_plain_text() {
        let p = only_paragraph("see [docs](https://example.com/x) and [bad](javascript:alert(1))");
        assert_eq!(p.text, "see docs and bad");
        let linked: Vec<_> = p.spans.iter().filter(|s| s.link.is_some()).collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].range, 4..8);
        assert_eq!(linked[0].link.as_deref(), Some("https://example.com/x"));
    }

    #[test]
    fn a_bare_web_address_becomes_a_link_without_its_trailing_punctuation() {
        let p = only_paragraph("go to https://example.com/a, then stop");
        let linked: Vec<_> = p.spans.iter().filter(|s| s.link.is_some()).collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(&p.text[linked[0].range.clone()], "https://example.com/a");
        assert_eq!(linked[0].link.as_deref(), Some("https://example.com/a"));
    }

    #[test]
    fn an_autolink_in_angle_brackets_is_a_link() {
        let p = only_paragraph("<https://example.com>");
        assert_eq!(p.text, "https://example.com");
        assert_eq!(p.spans[0].link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn images_are_their_alt_text_or_nothing() {
        assert_eq!(
            only_paragraph("a ![logo](https://x/y.png) b").text,
            "a logo b"
        );
        assert_eq!(only_paragraph("a ![](https://x/y.png) b").text, "a  b");
    }

    #[test]
    fn raw_html_is_shown_as_text() {
        assert_eq!(only_paragraph("a <b>bold</b> c").text, "a <b>bold</b> c");
        match parse("<div>\nhi\n</div>").as_slice() {
            [Block::Paragraph(p)] => assert_eq!(p.text, "<div>\nhi\n</div>"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn headings_have_their_level() {
        match parse("# one\n\n### three").as_slice() {
            [Block::Heading { level: 1, text: a }, Block::Heading { level: 3, text: b }] => {
                assert_eq!((a.text.as_str(), b.text.as_str()), ("one", "three"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_fenced_code_block_keeps_its_language_and_text() {
        match parse("```rust\nfn main() {}\n    indented\n```").as_slice() {
            [Block::Code { lang, text }] => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(text, "fn main() {}\n    indented");
            }
            other => panic!("{other:?}"),
        }
        match parse("```\nplain\n```").as_slice() {
            [Block::Code { lang: None, text }] => assert_eq!(text, "plain"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_indented_code_block_is_code() {
        match parse("    four spaces").as_slice() {
            [Block::Code { lang: None, text }] => assert_eq!(text, "four spaces"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lists_are_bulleted_or_numbered_from_their_start_and_may_nest() {
        match parse("1. a\n2. b\n   - c").as_slice() {
            [Block::List {
                start: Some(1),
                items,
            }] => {
                assert_eq!(items.len(), 2);
                match items[1].blocks.as_slice() {
                    [Block::Paragraph(p), Block::List { start: None, items }] => {
                        assert_eq!(p.text, "b");
                        assert_eq!(items.len(), 1);
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
        match parse("3. x\n4. y").as_slice() {
            [Block::List { start: Some(3), .. }] => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn task_list_items_know_if_they_are_done() {
        match parse("- [x] done\n- [ ] todo\n- plain").as_slice() {
            [Block::List { items, .. }] => {
                let tasks: Vec<_> = items.iter().map(|i| i.task).collect();
                assert_eq!(tasks, [Some(true), Some(false), None]);
                match items[0].blocks.as_slice() {
                    [Block::Paragraph(p)] => assert_eq!(p.text, "done"),
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_table_has_alignment_a_head_and_rows() {
        let md = "| a | b | c |\n|:--|:-:|--:|\n| 1 | **2** | 3 |\n| 4 | 5 | 6 |";
        match parse(md).as_slice() {
            [Block::Table { align, head, rows }] => {
                assert_eq!(align, &[Align::Left, Align::Center, Align::Right]);
                let texts =
                    |cells: &Vec<Inline>| cells.iter().map(|c| c.text.clone()).collect::<Vec<_>>();
                assert_eq!(texts(head), ["a", "b", "c"]);
                assert_eq!(rows.len(), 2);
                assert_eq!(texts(&rows[0]), ["1", "2", "3"]);
                assert!(rows[0][1].spans[0].style.bold);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_quote_holds_blocks() {
        match parse("> one\n>\n> two").as_slice() {
            [Block::Quote(blocks)] => assert_eq!(blocks.len(), 2),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_rule_is_a_rule() {
        assert_eq!(parse("a\n\n---\n\nb").len(), 3);
        assert_eq!(parse("a\n\n---\n\nb")[1], Block::Rule);
    }

    #[test]
    fn text_without_markdown_is_one_plain_paragraph() {
        assert_eq!(
            only_paragraph("just words"),
            Inline {
                text: "just words".into(),
                spans: vec![span(0..10, PLAIN, None)],
            }
        );
        assert!(parse("").is_empty());
    }
}
