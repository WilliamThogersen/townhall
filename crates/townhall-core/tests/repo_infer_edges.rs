use std::time::{Duration, Instant};

use tempfile::TempDir;
use townhall_core::repo_infer::{event_text, infer, Inference, Rule};

mod common;
mod infer_support;
use infer_support::{ask, candidate, found, missing_path, repo, shop_and_townhall, Repo};

#[tokio::test]
async fn a_tracked_file_beats_a_mention_in_docs() {
    let hive = repo(
        "hive",
        &[(
            "src/shared/conversationTasks.ts",
            "export const tasks = 1\n",
        )],
    )
    .await;
    let townhall = repo(
        "townhall",
        &[(
            "docs/style-guide/build-brief.md",
            "port conversationTasks.ts from the other app\n",
        )],
    )
    .await;
    let candidates = [townhall.candidate(), hive.candidate()];
    let got = infer("fix the conversationTasks.ts sorting", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        found("hive", Rule::Code, "it has src/shared/conversationTasks.ts")
    );
}

#[tokio::test]
async fn content_hits_need_whole_tokens() {
    let shop = repo(
        "shop",
        &[(
            "src/one.ts",
            "import q from './subquery.tsx'\nfetch('./info')\nmedia.ts\n",
        )],
    )
    .await;
    let store = repo(
        "store",
        &[("src/two.ts", "import q from 'query.ts'\nfetch('/info')\n")],
    )
    .await;
    let candidates = [shop.candidate(), store.candidate()];
    let got = infer("the /info call in query.ts", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        found("store", Rule::Code, "it has /info in src/two.ts")
    );
    let got = infer("see a.ts", &candidates).await.unwrap();
    assert_eq!(
        got,
        ask(&["shop", "store"], "two repos could fit. which one?")
    );
}

#[tokio::test]
async fn control_characters_in_a_hint_do_not_fail_the_call() {
    let (shop, townhall) = shop_and_townhall().await;
    let candidates = [shop.candidate(), townhall.candidate()];
    let got = infer("rename `a\0b.ts` and `x\u{1b}y`", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        ask(&["shop", "townhall"], "two repos could fit. which one?")
    );
}

const TEN: [&str; 10] = ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"];

#[tokio::test]
async fn a_huge_message_stays_fast() {
    let mut repos: Vec<Repo> = Vec::new();
    for name in TEN {
        repos.push(repo(name, &[]).await);
    }
    let candidates: Vec<_> = repos.iter().map(Repo::candidate).collect();
    let message: String = (0..60_000)
        .map(|i| format!("/no/such/dir{i} ~/no/such/{i} "))
        .collect();
    assert!(message.len() > 1_000_000);
    let start = Instant::now();
    let got = infer(&message, &candidates).await.unwrap();
    let took = start.elapsed();
    assert!(matches!(got, Inference::Ask { .. }), "{got:?}");
    assert!(took < Duration::from_secs(3), "took {took:?}");
}

#[tokio::test]
async fn naming_an_unreachable_repo_asks_instead_of_guessing() {
    let (shop, _townhall) = shop_and_townhall().await;
    let plain = TempDir::new().unwrap();
    let candidates = [candidate("townhall", &missing_path()), shop.candidate()];
    let got = infer("townhall needs a settings page", &candidates)
        .await
        .unwrap();
    assert_eq!(got, ask(&["shop"], "townhall is not reachable. which one?"));
    let got = infer("move the login from shop to townhall", &candidates)
        .await
        .unwrap();
    assert_eq!(got, ask(&["shop"], "townhall is not reachable. which one?"));
    let candidates = [
        candidate("gone", &missing_path()),
        candidate("plain", plain.path()),
        shop.candidate(),
    ];
    let got = infer("gone and plain need work", &candidates)
        .await
        .unwrap();
    assert_eq!(
        got,
        ask(&["shop"], "gone and plain are not reachable. which one?")
    );
}

#[tokio::test]
async fn names_come_back_trimmed() {
    let (shop, townhall) = shop_and_townhall().await;
    let one = [candidate("  shop ", shop.dir.path())];
    let got = infer("make it faster", &one).await.unwrap();
    assert_eq!(
        event_text(&got, "main").as_deref(),
        Some("~ repo shop · main, only repo")
    );
    let two = [
        candidate(" shop", shop.dir.path()),
        candidate("townhall  ", townhall.dir.path()),
    ];
    let got = infer("make it faster", &two).await.unwrap();
    assert_eq!(
        got,
        ask(&["shop", "townhall"], "two repos could fit. which one?")
    );
    let got = infer("shop needs a cart", &two).await.unwrap();
    assert_eq!(got, found("shop", Rule::Named, "you named it"));
    let got = infer("see query.ts", &two).await.unwrap();
    assert_eq!(got, found("shop", Rule::Code, "it has src/search/query.ts"));
}

#[tokio::test]
async fn a_backtick_name_used_after_a_dot_hits() {
    let app = repo(
        "app",
        &[(
            "code.js",
            "child.kill_on_drop(true); React.useState(0); this.handleClick();\n",
        )],
    )
    .await;
    let other = repo("other", &[("README.md", "other\n")]).await;
    let candidates = [other.candidate(), app.candidate()];
    for (message, name) in [
        ("rename `kill_on_drop`", "kill_on_drop"),
        ("replace `useState` here", "useState"),
        ("fix `handleClick`", "handleClick"),
    ] {
        let got = infer(message, &candidates).await.unwrap();
        let reason = format!("it has {name} in code.js");
        assert_eq!(got, found("app", Rule::Code, &reason), "{message}");
    }
}
