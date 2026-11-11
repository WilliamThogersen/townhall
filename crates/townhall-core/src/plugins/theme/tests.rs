use super::*;
use crate::Error;

const SLATE: &str = include_str!("slate.css");

fn refused(text: &str) -> String {
    match parse(text) {
        Err(Error::InvalidInput(message)) => message,
        Err(Error::TooLarge(message)) => message,
        other => panic!("expected a refusal for {text:?}, got {other:?}"),
    }
}

fn block(body: &str) -> String {
    format!(":root {{\n  {body}\n}}\n")
}

#[test]
fn the_official_slate_theme_is_accepted_and_keeps_every_variable() {
    let css = parse(SLATE).unwrap();
    assert_eq!(css.matches("--th-").count(), SLATE.matches("--th-").count());
    assert!(css.contains(":root, [data-theme=\"dark\"] {"));
    assert!(css.contains("--th-accent-tint: rgba(124, 196, 255, 0.1);"));
    assert!(css.contains("[data-theme=\"light\"] {"));
    assert_eq!(parse(&css).unwrap(), css);
}

#[test]
fn comments_are_dropped_from_what_is_kept() {
    let css = parse(":root { /* the page */ --th-bg: #000; /* x */ }").unwrap();
    assert_eq!(css, ":root {\n  --th-bg: #000;\n}\n");
}

#[test]
fn a_selector_other_than_the_three_allowed_is_refused() {
    for selector in [
        "body",
        ".card",
        "#app",
        "*",
        ":root .card",
        "html[data-theme=\"dark\"]",
        "[data-theme=\"sepia\"]",
        "[data-theme='dark']",
        ":root, body",
        ":hover",
        "",
    ] {
        let message = refused(&format!("{selector} {{ --th-bg: #000; }}"));
        assert!(message.contains("selector"), "{selector}: {message}");
    }
}

#[test]
fn at_rules_are_refused_including_import() {
    for text in [
        "@import \"https://evil.example/x.css\";",
        "@import url(\"x.css\");",
        "@media (min-width: 1px) { :root { --th-bg: #000; } }",
        "@font-face { font-family: x; }",
        "@charset \"utf-8\";\n:root { --th-bg: #000; }",
        ":root { --th-bg: #000; }\n@import \"x.css\";",
    ] {
        let message = refused(text);
        assert!(message.contains('@'), "{text}: {message}");
    }
}

#[test]
fn url_in_any_value_is_refused() {
    for value in [
        "url(https://evil.example/x.png)",
        "URL(x)",
        "url( \"x\" )",
        "\"url(x)\"",
        "rgba(0, 0, 0, 0.5) url(x)",
    ] {
        let message = refused(&block(&format!("--th-bg: {value};")));
        assert!(message.contains("url("), "{value}: {message}");
    }
}

#[test]
fn expression_in_any_value_is_refused() {
    for value in [
        "expression(alert(1))",
        "EXPRESSION(1)",
        "calc(expression(1))",
    ] {
        let message = refused(&block(&format!("--th-bg: {value};")));
        assert!(message.contains("expression"), "{value}: {message}");
    }
}

#[test]
fn a_property_that_is_not_a_th_custom_property_is_refused() {
    for name in [
        "color",
        "background",
        "background-image",
        "--bg",
        "--other-bg",
        "--thx-bg",
        "--th-",
        "--TH-bg",
        "-th-bg",
        "--th-bg!",
        "--th-b g",
    ] {
        let message = refused(&block(&format!("{name}: #000;")));
        assert!(message.contains("--th-"), "{name}: {message}");
    }
}

#[test]
fn a_function_that_is_not_a_plain_color_or_math_function_is_refused() {
    for value in [
        "image-set(\"x\" 1x)",
        "src(\"x\")",
        "element(#a)",
        "paint(x)",
        "env(x)",
    ] {
        let message = refused(&block(&format!("--th-bg: {value};")));
        assert!(message.contains("not allowed"), "{value}: {message}");
    }
    let ok = parse(&block(
        "--th-a: color-mix(in srgb, var(--th-bg) 50%, #fff); --th-b: calc(1px + 2px); --th-c: \"Inter\", sans-serif;",
    ));
    assert!(ok.is_ok(), "{ok:?}");
}

#[test]
fn a_nested_rule_is_refused() {
    for text in [
        ":root { .card { --th-bg: #000; } }",
        ":root { --th-bg: #000; [data-theme=\"dark\"] { --th-fg: #fff; } }",
        ":root { & .x { --th-bg: #000; } }",
    ] {
        let message = refused(text);
        assert!(message.contains("nested"), "{text}: {message}");
    }
}

#[test]
fn characters_that_could_smuggle_something_are_refused() {
    for value in [
        "#000 !important",
        "#000\\31",
        "<#000>",
        "#000 } body { color: red",
        "{#000}",
        "a@b",
        "#000 \u{0}",
    ] {
        let text = block(&format!("--th-bg: {value};"));
        assert!(
            matches!(parse(&text), Err(Error::InvalidInput(_))),
            "{value:?}"
        );
    }
    assert!(refused(":root { --th-bg: \\75rl(x); }").contains("backslash"));
}

#[test]
fn broken_structure_is_refused_with_a_reason() {
    assert!(refused(":root { --th-bg: #000;").contains("closing"));
    assert!(refused(":root --th-bg: #000; }").contains('{'));
    assert!(refused(":root { --th-bg #000; }").contains("`:`"));
    assert!(refused(":root { --th-bg: ; }").contains("no value"));
    assert!(refused(":root { --th-bg: rgb(0, 0, 0; }").contains("parenthes"));
    assert!(refused(":root { --th-bg: #000; } }").contains('}'));
    assert!(refused(":root { --th-bg: #000; } /* open").contains("comment"));
    assert!(refused(":root { --th-a: \"open; }").contains("string"));
    assert!(refused(":root { --th-a: \"two\nlines\"; }").contains("string"));
}

#[test]
fn an_empty_theme_or_one_with_no_declarations_is_refused() {
    assert!(refused("").contains("no rules"));
    assert!(refused("  /* nothing */  ").contains("no rules"));
    assert!(refused(":root { }").contains("no variables"));
}

#[test]
fn an_oversize_theme_is_refused() {
    let line = "  --th-bg: #000;\n";
    let text = format!(
        ":root {{\n{}}}\n",
        line.repeat(MAX_THEME_BYTES / line.len() + 1)
    );
    let err = parse(&text).unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");
}
