use super::{extract, Hint, HintKind};

fn hint(text: &str, kind: HintKind) -> Hint {
    Hint {
        text: text.to_string(),
        kind,
    }
}

fn texts(message: &str) -> Vec<String> {
    extract(message).into_iter().map(|h| h.text).collect()
}

#[test]
fn backticks_come_first_and_keep_their_text() {
    assert_eq!(
        extract("rename useSearch in query.ts, see `useSearch()` and `npm run dev`"),
        vec![
            hint("useSearch()", HintKind::Quoted),
            hint("npm run dev", HintKind::Quoted),
            hint("query.ts", HintKind::File),
        ]
    );
}

#[test]
fn fenced_and_unclosed_backticks_are_ignored() {
    assert!(extract("```\nfn main() {}\n```").is_empty());
    assert!(extract("an `unclosed span").is_empty());
}

#[test]
fn files_paths_routes_and_packages() {
    assert_eq!(
        extract("see (src/search/query.ts), Cargo.toml. the /api/login route and @scope/pkg-ui"),
        vec![
            hint("src/search/query.ts", HintKind::Path),
            hint("Cargo.toml", HintKind::File),
            hint("/api/login", HintKind::Route),
            hint("@scope/pkg-ui", HintKind::Package),
        ]
    );
}

#[test]
fn crate_like_words_come_last() {
    assert_eq!(
        extract("repo_infer breaks townhall-core, see main.rs"),
        vec![
            hint("main.rs", HintKind::File),
            hint("repo_infer", HintKind::Word),
            hint("townhall-core", HintKind::Word),
        ]
    );
}

#[test]
fn common_words_and_non_code_are_ignored() {
    let noise = "e.g. i.e. and/or w/o https://x.com/a ~/dev/x node.js example.com v1.2 3.14 \
                 2024-01-01 follow-up built-in --force a-b one_ / // @scope hello world.";
    assert_eq!(texts(noise), Vec::<String>::new());
}

#[test]
fn duplicates_are_dropped_and_the_list_is_capped() {
    assert_eq!(
        texts("a.ts A.TS `a.ts` b.ts c.ts d.ts e.ts f.ts g.ts"),
        vec!["a.ts", "b.ts", "c.ts", "d.ts", "e.ts", "f.ts"]
    );
}

#[test]
fn path_needles() {
    let needle = |text: &str, kind| hint(text, kind).path_needle();
    assert_eq!(
        needle("/api/login", HintKind::Route).as_deref(),
        Some("api/login")
    );
    assert_eq!(
        needle("./Src/Search/", HintKind::Path).as_deref(),
        Some("src/search")
    );
    assert_eq!(
        needle("Query.ts", HintKind::File).as_deref(),
        Some("query.ts")
    );
    assert_eq!(needle("@scope/pkg", HintKind::Package), None);
    assert_eq!(needle("npm run dev", HintKind::Quoted), None);
    assert_eq!(needle("/", HintKind::Quoted), None);
}
