use super::{decide, Hits};
use crate::repo_infer::{Inference, Rule};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

fn hits(list: &[Option<&str>]) -> Hits {
    list.iter().map(|h| h.map(str::to_string)).collect()
}

fn hints() -> Vec<String> {
    names(&["README.md", "query.ts"])
}

#[test]
fn no_hits_decides_nothing() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[hits(&[None, None]), hits(&[None, None])],
    );
    assert_eq!(got, None);
}

#[test]
fn a_single_best_repo_wins_with_a_hit_the_others_lack() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[
            hits(&[Some("README.md"), None]),
            hits(&[Some("README.md"), Some("src/query.ts")]),
        ],
    );
    assert_eq!(
        got,
        Some(Inference::Found {
            repo: "b".into(),
            rule: Rule::Code,
            reason: "it has src/query.ts".into(),
        })
    );
}

#[test]
fn the_first_hit_is_the_reason_when_none_is_distinct() {
    let got = decide(
        &names(&["a"]),
        &hints(),
        &[hits(&[Some("README.md"), Some("src/query.ts")])],
    );
    assert_eq!(
        got,
        Some(Inference::Found {
            repo: "a".into(),
            rule: Rule::Code,
            reason: "it has README.md".into(),
        })
    );
}

#[test]
fn a_tie_asks_among_the_tied_with_the_shared_hint() {
    let got = decide(
        &names(&["a", "b", "c", "d"]),
        &hints(),
        &[
            hits(&[None, Some("x/query.ts")]),
            hits(&[None, None]),
            hits(&[None, Some("y/query.ts")]),
            hits(&[None, Some("query.ts")]),
        ],
    );
    assert_eq!(
        got,
        Some(Inference::Ask {
            candidates: names(&["a", "c", "d"]),
            reason: "three repos have query.ts. which one?".into(),
        })
    );
}

#[test]
fn a_tie_without_a_shared_hint_could_fit() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[
            hits(&[Some("README.md"), None]),
            hits(&[None, Some("q.ts")]),
        ],
    );
    assert_eq!(
        got,
        Some(Inference::Ask {
            candidates: names(&["a", "b"]),
            reason: "two repos could fit. which one?".into(),
        })
    );
}
