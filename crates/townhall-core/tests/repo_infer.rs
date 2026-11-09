use std::path::Path;

use serde::de::value::{Error as ValueError, MapDeserializer, StrDeserializer};
use serde::de::IntoDeserializer;
use serde::Deserialize;
use tempfile::TempDir;
use townhall_core::repo_infer::{event_text, infer, Candidate, Inference, Rule};
use townhall_core::Error;

mod common;
mod infer_support;
use infer_support::{ask, candidate, found, missing_path, repo, shop_and_townhall};

#[tokio::test]
async fn named_by_word_wins_over_code() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [shop.candidate(), townhall.candidate()];
    let got = infer("Townhall needs a settings page, like query.ts", &candidates)
        .await
        .unwrap();
    assert_eq!(got, found("townhall", Rule::Named, "you named it"));
}

#[tokio::test]
async fn named_by_absolute_path() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let message = format!("work in {}/ please", shop.dir.path().display());
    let got = infer(&message, &candidates).await.unwrap();
    assert_eq!(got, found("shop", Rule::Named, "you named it"));
}

#[tokio::test]
async fn named_by_a_path_inside_the_repo() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let file = shop.dir.path().join("src/search/query.ts");
    let message = format!("look at `{}`", file.display());
    let got = infer(&message, &candidates).await.unwrap();
    assert_eq!(got, found("shop", Rule::Named, "you named it"));
}

#[tokio::test]
async fn several_named_asks_among_them() {
    let (shop, townhall) = shop_and_townhall().await;
    let other = repo("store", &[]).await;
    let candidates = [shop.candidate(), other.candidate(), townhall.candidate()];
    let got = infer("move the login from shop to townhall", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        ask(&["shop", "townhall"], "you named two repos. which one?")
    );
}

#[tokio::test]
async fn code_file_name_finds_the_full_path() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let got = infer("search is slow, see query.ts", &candidates)
        .await
        .unwrap();
    assert_eq!(got, found("shop", Rule::Code, "it has src/search/query.ts"));
}

#[tokio::test]
async fn code_route_found_in_content() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let got = infer("the /api/login route returns 500", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        found("shop", Rule::Code, "it has /api/login in src/routes.ts")
    );
}

#[tokio::test]
async fn code_package_name_found_in_content() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let got = infer("bump @shop/ui to 2", &candidates).await.unwrap();
    assert_eq!(
        got,
        found("shop", Rule::Code, "it has @shop/ui in package.json")
    );
}

#[tokio::test]
async fn code_crate_name_matches_a_folder() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [shop.candidate(), townhall.candidate()];
    let got = infer("add a module to townhall-core", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        found("townhall", Rule::Code, "it has crates/townhall-core/")
    );
}

#[tokio::test]
async fn code_reason_prefers_a_hit_the_others_lack() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let got = infer("update README.md for query.ts", &candidates)
        .await
        .unwrap();
    assert_eq!(got, found("shop", Rule::Code, "it has src/search/query.ts"));
}

#[tokio::test]
async fn code_tie_asks_among_the_tied() {
    let a = repo("shop", &[("src/checkout.ts", "a\n")]).await;
    let b = repo("store", &[("web/checkout.ts", "b\n")]).await;
    let c = repo("docs", &[("index.md", "c\n")]).await;
    let candidates = [a.candidate(), c.candidate(), b.candidate()];
    let got = infer("fix the bug in checkout.ts", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        ask(&["shop", "store"], "two repos have checkout.ts. which one?")
    );
}

#[tokio::test]
async fn only_repo_when_nothing_else_fits() {
    let (shop, _townhall) = shop_and_townhall().await;
    let got = infer("make it faster", &[shop.candidate()]).await.unwrap();
    assert_eq!(got, found("shop", Rule::Only, "only repo"));
}

#[tokio::test]
async fn no_signal_when_nothing_fits() {
    let (shop, townhall) = shop_and_townhall().await;
    let other = repo("store", &[]).await;
    let candidates = [shop.candidate(), townhall.candidate(), other.candidate()];
    let got = infer("make it faster", &candidates).await.unwrap();
    assert_eq!(got, Inference::NoSignal);
}

#[tokio::test]
async fn hints_that_hit_nothing_fall_back() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [shop.candidate(), townhall.candidate()];
    let got = infer("rename `NothingLikeThis` in payment.go", &candidates)
        .await
        .unwrap();
    assert_eq!(got, Inference::NoSignal);
}

#[tokio::test]
async fn no_candidates_is_no_repos() {
    assert_eq!(infer("anything", &[]).await.unwrap(), Inference::NoRepos);
}

#[tokio::test]
async fn missing_and_non_git_paths_are_skipped() {
    let (shop, _townhall) = shop_and_townhall().await;
    let plain = TempDir::new().unwrap();
    let candidates = [
        candidate("gone", &missing_path()),
        candidate("plain", plain.path()),
        shop.candidate(),
    ];
    let got = infer("make it faster", &candidates).await.unwrap();
    assert_eq!(got, found("shop", Rule::Only, "only repo"));
}

#[tokio::test]
async fn only_unusable_candidates_is_no_repos() {
    let plain = TempDir::new().unwrap();
    let candidates = [
        candidate("gone", &missing_path()),
        candidate("plain", plain.path()),
    ];
    let got = infer("gone needs work", &candidates).await.unwrap();
    assert_eq!(got, Inference::NoRepos);
}

#[tokio::test]
async fn duplicate_or_empty_names_are_invalid() {
    let (shop, townhall) = shop_and_townhall().await;
    let twice = [shop.candidate(), candidate("Shop", townhall.dir.path())];
    let err = infer("x", &twice).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    let empty = [candidate(" ", shop.dir.path())];
    let err = infer("x", &empty).await.unwrap_err();
    assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
}

#[tokio::test]
async fn event_text_for_found_only() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [townhall.candidate(), shop.candidate()];
    let code = infer("see src/search/query.ts", &candidates).await.unwrap();
    assert_eq!(
        event_text(&code, "develop").as_deref(),
        Some("~ repo shop · develop, it has src/search/query.ts")
    );
    let named = infer("townhall needs a settings page", &candidates)
        .await
        .unwrap();
    assert_eq!(
        event_text(&named, "main").as_deref(),
        Some("~ repo townhall · main, you named it")
    );
    let asked = infer("make it faster", &candidates).await.unwrap();
    assert_eq!(event_text(&asked, "main"), None);
    let tie = ask(&["shop", "townhall"], "two repos could fit. which one?");
    assert_eq!(event_text(&tie, "main"), None);
    assert_eq!(event_text(&Inference::NoRepos, "main"), None);
}

fn from_pairs<T: for<'de> Deserialize<'de>>(pairs: &[(&'static str, &'static str)]) -> T {
    let map = MapDeserializer::<_, ValueError>::new(pairs.iter().copied());
    T::deserialize(map).unwrap()
}

#[test]
fn wire_names_are_camel_case_and_tagged() {
    let rule = |s: &'static str| {
        let de: StrDeserializer<ValueError> = s.into_deserializer();
        Rule::deserialize(de).unwrap()
    };
    assert_eq!(
        [rule("named"), rule("code"), rule("only")],
        [Rule::Named, Rule::Code, Rule::Only]
    );
    let got: Inference = from_pairs(&[
        ("kind", "found"),
        ("repo", "shop"),
        ("rule", "code"),
        ("reason", "it has x"),
    ]);
    assert_eq!(got, found("shop", Rule::Code, "it has x"));
    let none: Inference = from_pairs(&[("kind", "noRepos")]);
    assert_eq!(none, Inference::NoRepos);
    let quiet: Inference = from_pairs(&[("kind", "noSignal")]);
    assert_eq!(quiet, Inference::NoSignal);
    let cand: Candidate = from_pairs(&[("name", "shop"), ("path", "/x/shop")]);
    assert_eq!(cand, candidate("shop", Path::new("/x/shop")));
}
