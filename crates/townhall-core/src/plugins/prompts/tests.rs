use super::*;
use crate::Error;

const OFFICIAL: &str = include_str!("starters.json");

fn refused(result: Result<impl std::fmt::Debug>) -> String {
    match result {
        Err(Error::InvalidInput(m)) | Err(Error::TooLarge(m)) => m,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_official_starter_prompts_parse() {
    let starters = parse_starters(OFFICIAL).unwrap();
    assert_eq!(starters.len(), 6);
    assert_eq!(starters[0].title, "find dead code");
    assert!(starters[0].text.starts_with("find code that nothing calls"));
}

#[test]
fn starters_are_trimmed_and_must_be_a_list_of_title_and_text() {
    let got = parse_starters(r#"[{"title": "  a ", "text": " do a\n"}]"#).unwrap();
    assert_eq!(
        got,
        [Starter {
            title: "a".into(),
            text: "do a".into()
        }]
    );
    assert!(parse_starters("[]").unwrap().is_empty());
    for bad in [
        r#"{"title": "a", "text": "b"}"#,
        r#"[{"title": "a"}]"#,
        r#"[{"title": "a", "text": "b", "extra": 1}]"#,
        r#"["a"]"#,
        "not json",
    ] {
        assert!(refused(parse_starters(bad)).contains("starters"), "{bad}");
    }
}

#[test]
fn empty_and_oversize_starters_are_refused() {
    assert!(refused(parse_starters(r#"[{"title": " ", "text": "b"}]"#)).contains("title"));
    assert!(refused(parse_starters(r#"[{"title": "a", "text": " "}]"#)).contains("text"));
    let title = "t".repeat(MAX_STARTER_TITLE_CHARS + 1);
    let long = format!(r#"[{{"title": "{title}", "text": "b"}}]"#);
    assert!(refused(parse_starters(&long)).contains("title"));
    let text = "t".repeat(MAX_STARTER_TEXT_CHARS + 1);
    let long = format!(r#"[{{"title": "a", "text": "{text}"}}]"#);
    assert!(refused(parse_starters(&long)).contains("text"));
    let many = format!(
        "[{}]",
        vec![r#"{"title": "a", "text": "b"}"#; MAX_STARTERS + 1].join(",")
    );
    assert!(refused(parse_starters(&many)).contains("at most"));
}

#[test]
fn markdown_is_trimmed_and_capped_in_characters() {
    assert_eq!(
        parse_markdown("brief", "\n  be brief\n\n").unwrap(),
        "be brief"
    );
    assert!(refused(parse_markdown("brief", " \n ")).contains("empty"));
    let exact = "é".repeat(MAX_PROMPT_CHARS);
    assert_eq!(parse_markdown("brief", &exact).unwrap(), exact);
    let over = "é".repeat(MAX_PROMPT_CHARS + 1);
    let message = refused(parse_markdown("planner", &over));
    assert!(
        message.contains("planner") && message.contains("8000"),
        "{message}"
    );
    assert!(refused(parse_markdown("brief", "a\u{0}b")).contains("control"));
}
