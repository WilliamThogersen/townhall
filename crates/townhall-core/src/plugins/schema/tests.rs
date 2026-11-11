use serde_json::json;

use super::*;
use crate::Error;

fn refused(value: serde_json::Value) -> String {
    match parse(&value.to_string()) {
        Err(Error::InvalidInput(m)) | Err(Error::TooLarge(m)) => m,
        other => panic!("expected a refusal for {value}, got {other:?}"),
    }
}

fn one(field: serde_json::Value) -> serde_json::Value {
    json!({ "fields": [field] })
}

fn field(kind: &str, default: serde_json::Value) -> serde_json::Value {
    json!({ "key": "k", "label": "a label", "type": kind, "default": default })
}

#[test]
fn a_schema_with_every_kind_parses_in_order() {
    let text = json!({ "fields": [
        { "key": "verbose", "label": "verbose", "description": "say more", "type": "boolean", "default": false },
        { "key": "name", "label": "name", "type": "string", "default": "", "secret": false },
        { "key": "token", "label": "token", "type": "string", "default": "", "secret": true },
        { "key": "retries", "label": "retries", "type": "integer", "default": 3, "min": 1, "max": 9 },
        { "key": "mode", "label": "mode", "type": "enum", "default": "fast", "choices": ["fast", "slow"] }
    ]})
    .to_string();
    let fields = parse(&text).unwrap();
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, ["verbose", "name", "token", "retries", "mode"]);
    assert_eq!(fields[0].description.as_deref(), Some("say more"));
    assert!(fields[2].secret && !fields[1].secret);
    assert_eq!((fields[3].min, fields[3].max), (Some(1), Some(9)));
    assert_eq!(fields[4].kind, FieldKind::Enum);
}

#[test]
fn unknown_keys_and_a_bad_shape_are_refused() {
    assert!(refused(json!({ "fields": [], "extra": 1 })).contains("settings"));
    let mut odd = field("boolean", json!(true));
    odd["colour"] = json!("red");
    assert!(refused(one(odd)).contains("settings"));
    assert!(refused(json!([])).contains("settings"));
    assert!(refused(json!({ "fields": "x" })).contains("settings"));
    assert!(refused(one(field("float", json!(1)))).contains("settings"));
    assert!(parse("nope").is_err());
}

#[test]
fn keys_must_be_plain_and_unique_ignoring_case() {
    for key in ["", "a b", "-a", "a/b", &"k".repeat(65)] {
        let mut f = field("boolean", json!(true));
        f["key"] = json!(key);
        assert!(refused(one(f)).contains("key"), "{key:?}");
    }
    let text = json!({ "fields": [
        { "key": "Mode", "label": "a", "type": "boolean", "default": true },
        { "key": "mode", "label": "b", "type": "boolean", "default": true }
    ]});
    assert!(refused(text).contains("twice"));
}

#[test]
fn labels_and_descriptions_must_have_text_and_stay_short() {
    let mut f = field("boolean", json!(true));
    f["label"] = json!("  ");
    assert!(refused(one(f)).contains("label"));
    let mut f = field("boolean", json!(true));
    f["label"] = json!("l".repeat(81));
    assert!(refused(one(f)).contains("label"));
    let mut f = field("boolean", json!(true));
    f["description"] = json!("d".repeat(501));
    assert!(refused(one(f)).contains("description"));
}

#[test]
fn the_default_must_fit_the_type() {
    assert!(refused(one(field("boolean", json!("yes")))).contains("default"));
    assert!(refused(one(field("string", json!(3)))).contains("default"));
    assert!(refused(one(field("integer", json!(1.5)))).contains("default"));
    assert!(refused(one(field("integer", json!("1")))).contains("default"));
    let mut f = field("enum", json!("c"));
    f["choices"] = json!(["a", "b"]);
    assert!(refused(one(f)).contains("default"));
    let f = field("string", json!("s".repeat(MAX_STRING_CHARS + 1)));
    assert!(refused(one(f)).contains("default"));
}

#[test]
fn integers_need_a_sane_range_and_other_kinds_must_not_have_one() {
    let mut f = field("integer", json!(5));
    f["min"] = json!(6);
    let message = refused(one(f));
    assert!(
        message.contains("default") && message.contains("at least 6"),
        "{message}"
    );
    let mut f = field("integer", json!(5));
    f["max"] = json!(4);
    let message = refused(one(f));
    assert!(
        message.contains("default") && message.contains("at most 4"),
        "{message}"
    );
    let mut f = field("integer", json!(5));
    f["min"] = json!(7);
    f["max"] = json!(3);
    assert!(refused(one(f)).contains("min"));
    let mut f = field("string", json!(""));
    f["min"] = json!(1);
    assert!(refused(one(f)).contains("min"));
    let mut f = field("boolean", json!(true));
    f["max"] = json!(1);
    assert!(refused(one(f)).contains("max"));
}

#[test]
fn choices_belong_to_enums_only_and_must_be_usable() {
    assert!(refused(one(field("enum", json!("a")))).contains("choices"));
    let mut f = field("enum", json!("a"));
    f["choices"] = json!([]);
    assert!(refused(one(f)).contains("choices"));
    let mut f = field("enum", json!("a"));
    f["choices"] = json!(["a", "a"]);
    assert!(refused(one(f)).contains("choices"));
    let mut f = field("enum", json!("a"));
    f["choices"] = json!(["a", " "]);
    assert!(refused(one(f)).contains("choices"));
    let mut f = field("enum", json!("a"));
    f["choices"] = json!((0..=MAX_CHOICES)
        .map(|n| format!("c{n}"))
        .collect::<Vec<_>>());
    assert!(refused(one(f)).contains("choices"));
    let mut f = field("string", json!(""));
    f["choices"] = json!(["a"]);
    assert!(refused(one(f)).contains("choices"));
}

#[test]
fn only_strings_may_be_secret() {
    for kind in [("boolean", json!(true)), ("integer", json!(1))] {
        let mut f = field(kind.0, kind.1);
        f["secret"] = json!(true);
        assert!(refused(one(f)).contains("secret"));
    }
}

#[test]
fn too_many_fields_or_too_much_text_is_refused() {
    let many: Vec<_> = (0..=MAX_FIELDS)
        .map(
            |n| json!({ "key": format!("k{n}"), "label": "l", "type": "boolean", "default": true }),
        )
        .collect();
    assert!(refused(json!({ "fields": many })).contains("at most"));
    let big = format!("{{\"fields\": [], \"pad\": \"{}\"}}", "x".repeat(70 * 1024));
    assert!(matches!(parse(&big), Err(Error::TooLarge(_))));
}

#[test]
fn a_value_is_checked_against_its_field() {
    let fields = parse(
        &json!({ "fields": [
            { "key": "b", "label": "b", "type": "boolean", "default": false },
            { "key": "s", "label": "s", "type": "string", "default": "" },
            { "key": "i", "label": "i", "type": "integer", "default": 2, "min": 1, "max": 5 },
            { "key": "e", "label": "e", "type": "enum", "default": "x", "choices": ["x", "y"] }
        ]})
        .to_string(),
    )
    .unwrap();
    let [b, s, i, e] = [&fields[0], &fields[1], &fields[2], &fields[3]];
    assert_eq!(b.check(&json!(true)).unwrap(), json!(true));
    assert_eq!(s.check(&json!("hi")).unwrap(), json!("hi"));
    assert_eq!(i.check(&json!(5)).unwrap(), json!(5));
    assert_eq!(e.check(&json!("y")).unwrap(), json!("y"));
    for (field, bad) in [
        (b, json!("true")),
        (b, json!(null)),
        (s, json!(1)),
        (s, json!("s".repeat(MAX_STRING_CHARS + 1))),
        (i, json!(0)),
        (i, json!(6)),
        (i, json!(2.5)),
        (i, json!("2")),
        (e, json!("z")),
        (e, json!(1)),
    ] {
        let err = field.check(&bad).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{bad}: {err}");
        assert!(err.to_string().contains(&field.key), "{err}");
    }
}
