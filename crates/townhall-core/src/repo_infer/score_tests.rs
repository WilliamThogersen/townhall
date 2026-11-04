use super::{decide, Hit, HitKind};
use crate::repo_infer::{Inference, Rule};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|n| n.to_string()).collect()
}

fn path(what: &str) -> Option<Hit> {
    Some(Hit {
        kind: HitKind::Path,
        what: what.to_string(),
    })
}

fn content(what: &str) -> Option<Hit> {
    Some(Hit {
        kind: HitKind::Content,
        what: what.to_string(),
    })
}

fn hints() -> Vec<String> {
    names(&["README.md", "query.ts"])
}

fn found(repo: &str, reason: &str) -> Option<Inference> {
    Some(Inference::Found {
        repo: repo.into(),
        rule: Rule::Code,
        reason: reason.into(),
    })
}

fn ask(candidates: &[&str], reason: &str) -> Option<Inference> {
    Some(Inference::Ask {
        candidates: names(candidates),
        reason: reason.into(),
    })
}

#[test]
fn no_hits_decides_nothing() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[vec![None, None], vec![None, None]],
    );
    assert_eq!(got, None);
}

#[test]
fn a_single_best_repo_wins_with_a_hit_the_others_lack() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[
            vec![path("README.md"), None],
            vec![path("README.md"), path("src/query.ts")],
        ],
    );
    assert_eq!(got, found("b", "it has src/query.ts"));
}

#[test]
fn the_first_hit_is_the_reason_when_none_is_distinct() {
    let got = decide(
        &names(&["a"]),
        &hints(),
        &[vec![path("README.md"), path("src/query.ts")]],
    );
    assert_eq!(got, found("a", "it has README.md"));
}

#[test]
fn a_path_hit_beats_content_hits() {
    let got = decide(
        &names(&["docs", "app"]),
        &hints(),
        &[
            vec![content("README.md in a.md"), content("query.ts in b.md")],
            vec![None, path("src/query.ts")],
        ],
    );
    assert_eq!(got, found("app", "it has src/query.ts"));
}

#[test]
fn the_reason_prefers_a_path_hit_that_beats_the_others() {
    let got = decide(
        &names(&["docs", "app"]),
        &hints(),
        &[
            vec![content("README.md in a.md"), None],
            vec![path("README.md"), content("query.ts in b.ts")],
        ],
    );
    assert_eq!(got, found("app", "it has README.md"));
}

#[test]
fn a_tie_within_the_same_weight_asks_among_the_tied() {
    let got = decide(
        &names(&["a", "b", "c", "d"]),
        &hints(),
        &[
            vec![None, path("x/query.ts")],
            vec![content("README.md in r.md"), None],
            vec![None, path("y/query.ts")],
            vec![None, path("query.ts")],
        ],
    );
    assert_eq!(
        got,
        ask(&["a", "c", "d"], "three repos have query.ts. which one?")
    );
}

#[test]
fn a_content_tie_asks_too() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[
            vec![None, content("query.ts in a.md")],
            vec![None, content("query.ts in b.md")],
        ],
    );
    assert_eq!(got, ask(&["a", "b"], "two repos have query.ts. which one?"));
}

#[test]
fn a_tie_without_a_shared_hint_could_fit() {
    let got = decide(
        &names(&["a", "b"]),
        &hints(),
        &[vec![path("README.md"), None], vec![None, path("q.ts")]],
    );
    assert_eq!(got, ask(&["a", "b"], "two repos could fit. which one?"));
}
