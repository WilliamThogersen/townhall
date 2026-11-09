use serde_json::json;
use townhall_core::definitions::{check_agent, parse_agent};

use crate::support::agent_errors;

const MINIMAL: &str = "name = \"a\"\nadapter = \"claude\"\n";

const GRID: &str = "grid = [\n  \"....#....\",\n  \"....#....\",\n  \".#######.\",\n  \"#########\",\n  \"##o###o##\",\n  \"#########\",\n  \"##oooo###\",\n  \".#######.\",\n  \"..#####..\",\n]\n";

fn with_avatar(body: &str) -> String {
    format!("{MINIMAL}\n[avatar]\n{body}")
}

#[test]
fn a_missing_avatar_section_is_none() {
    assert_eq!(parse_agent(MINIMAL).unwrap().def.avatar, None);
    let value = serde_json::to_value(parse_agent(MINIMAL).unwrap().def).unwrap();
    assert_eq!(value["avatar"], json!(null));
}

#[test]
fn every_avatar_key_is_read() {
    let source = with_avatar(
        "head = \"round\"\neyes = \"visor\"\nmouth = \"smile\"\naccessory = \"antenna\"\ncolor = \"fg-muted\"\n",
    );
    let def = check_agent(&source).unwrap().def;
    assert_eq!(
        serde_json::to_value(&def).unwrap()["avatar"],
        json!({"head": "round", "eyes": "visor", "mouth": "smile", "accessory": "antenna",
               "color": "fg-muted", "grid": null})
    );
}

#[test]
fn an_avatar_key_may_be_left_out() {
    let def = check_agent(&with_avatar("color = \"ok\"\n")).unwrap().def;
    let avatar = serde_json::to_value(&def).unwrap()["avatar"].clone();
    assert_eq!(avatar["color"], json!("ok"));
    assert_eq!(avatar["head"], json!(null));
}

#[test]
fn an_empty_avatar_section_is_allowed() {
    assert!(check_agent(&with_avatar("")).is_ok());
}

#[test]
fn an_unknown_avatar_key_is_an_error_naming_it() {
    let errors = agent_errors(&with_avatar("hat = \"top\"\n"));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("unknown field `hat`"), "{errors:?}");
    assert!(errors[0].contains("line 5"), "{errors:?}");
}

#[test]
fn an_unknown_avatar_value_is_an_error_listing_the_options() {
    for (key, value) in [
        ("head", "cube"),
        ("eyes", "laser"),
        ("mouth", "beak"),
        ("accessory", "crown"),
        ("color", "pink"),
    ] {
        let errors = agent_errors(&with_avatar(&format!("{key} = \"{value}\"\n")));
        assert_eq!(errors.len(), 1, "{key}: {errors:?}");
        assert!(
            errors[0].contains(&format!("unknown variant `{value}`")),
            "{errors:?}"
        );
    }
    let errors = agent_errors(&with_avatar("head = \"cube\"\n"));
    assert!(errors[0].contains("`square`") && errors[0].contains("`tapered`"));
}

#[test]
fn a_literal_grid_is_read_back_as_written() {
    let def = check_agent(&with_avatar(&format!("color = \"info\"\n{GRID}")))
        .unwrap()
        .def;
    let grid = serde_json::to_value(&def).unwrap()["avatar"]["grid"].clone();
    assert_eq!(grid.as_array().unwrap().len(), 9);
    assert_eq!(grid[0], json!("....#...."));
    assert_eq!(grid[8], json!("..#####.."));
}

#[test]
fn a_grid_must_have_nine_rows_of_nine_cells() {
    let short = with_avatar("grid = [\"#########\"]\n");
    let errors = agent_errors(&short);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("avatar.grid") && errors[0].contains("9 rows"),
        "{errors:?}"
    );

    let narrow = GRID.replace("\"#########\"", "\"########\"");
    let errors = agent_errors(&with_avatar(&narrow));
    assert!(
        errors
            .iter()
            .any(|e| e.contains("row 4") && e.contains("9 cells")),
        "{errors:?}"
    );
}

#[test]
fn a_grid_may_only_use_dot_hash_and_o() {
    let bad = GRID.replace(
        "\"....#....\",\n  \"....#....\"",
        "\"....x....\",\n  \"....#....\"",
    );
    let errors = agent_errors(&with_avatar(&bad));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("row 1") && errors[0].contains("`x`"),
        "{errors:?}"
    );
}

#[test]
fn the_avatar_is_saved_inside_the_source_as_written() {
    let source = with_avatar("color = \"ok\"\n");
    assert_eq!(check_agent(&source).unwrap().source, source);
}
