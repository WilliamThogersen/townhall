use std::path::{Path, PathBuf};

use super::named;
use crate::repo_infer::Candidate;

fn repos(list: &[(&str, &str)]) -> Vec<Candidate> {
    list.iter()
        .map(|(name, path)| Candidate {
            name: name.to_string(),
            path: PathBuf::from(path),
        })
        .collect()
}

fn two() -> Vec<Candidate> {
    repos(&[("townhall", "/r/townhall"), ("shop", "/r/clients/shop")])
}

#[test]
fn a_whole_word_names_a_repo_in_any_case() {
    let candidates = two();
    for message in [
        "Townhall needs a settings page",
        "fix TOWNHALL.",
        "(townhall)",
        "in `townhall`, please",
        "townhall's sidebar",
        "repo: townhall",
        "ship it to townhall",
    ] {
        assert_eq!(named(message, &candidates, None), vec![0], "{message}");
    }
}

#[test]
fn a_name_inside_another_word_or_path_does_not_count() {
    let candidates = two();
    for message in [
        "townhall-core needs a test",
        "townhalls everywhere",
        "edit crates/townhall/lib.rs",
        "look in ~/.townhall",
        "open townhall.rs",
        "mytownhall is different",
        "workshop day",
        "bump @shop/ui",
        "snake_shop_case",
    ] {
        assert!(named(message, &candidates, None).is_empty(), "{message}");
    }
}

#[test]
fn several_names_come_back_in_candidate_order() {
    let candidates = two();
    assert_eq!(
        named("move it from shop to townhall", &candidates, None),
        vec![0, 1]
    );
}

#[test]
fn a_repo_path_or_a_path_inside_it_names_it() {
    let candidates = two();
    let home = Some(Path::new("/r"));
    for message in [
        "work in /r/clients/shop",
        "work in /r/clients/shop/ now",
        "see `/r/clients/shop/src/query.ts`.",
        "cd ~/clients/shop",
    ] {
        assert_eq!(named(message, &candidates, home), vec![1], "{message}");
    }
    assert!(named("cd ~/clients/shop", &candidates, None).is_empty());
    assert!(named("see /r/clients/shopping", &candidates, home).is_empty());
    assert!(named("see ./r/clients/shop", &candidates, home).is_empty());
}

#[test]
fn the_deepest_repo_wins_for_a_nested_path() {
    let candidates = repos(&[("work", "/r"), ("shop", "/r/clients/shop")]);
    assert_eq!(
        named("see /r/clients/shop/src/a.ts", &candidates, None),
        vec![1]
    );
    assert_eq!(named("see /r/notes.md", &candidates, None), vec![0]);
}

#[test]
fn only_the_first_distinct_path_tokens_are_checked() {
    let candidates = two();
    let filler: String = (0..20).map(|i| format!("/tmp/x{i} /tmp/x0 ")).collect();
    let late = format!("{filler} /r/clients/shop");
    assert!(named(&late, &candidates, None).is_empty());
    let repeated = format!("{} /r/clients/shop", "/tmp/x0 ".repeat(1000));
    assert_eq!(named(&repeated, &candidates, None), vec![1]);
}
