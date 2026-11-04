use std::path::Path;
use std::time::Duration;

use tempfile::TempDir;
use tokio::time::Instant;

use super::{grep_first, is_work_tree, ls_files, token_pattern, Grep, BATCH};
use crate::cmd::run;
use crate::Error;

fn soon() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

async fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    run("git", &["init", "-b", "main"], Some(dir.path()))
        .await
        .unwrap();
    std::fs::write(dir.path().join("a.txt"), "needle here\n").unwrap();
    run("git", &["add", "a.txt"], Some(dir.path()))
        .await
        .unwrap();
    dir
}

#[tokio::test]
async fn work_tree_check() {
    let git = repo().await;
    let plain = TempDir::new().unwrap();
    assert!(is_work_tree(git.path(), soon()).await.unwrap());
    assert!(!is_work_tree(plain.path(), soon()).await.unwrap());
    assert!(!is_work_tree(&plain.path().join("gone"), soon())
        .await
        .unwrap());
    assert!(!is_work_tree(git.path(), Instant::now()).await.unwrap());
}

#[tokio::test]
async fn ls_files_lists_tracked_paths_or_runs_out_of_time() {
    let git = repo().await;
    let files = ls_files(git.path(), soon()).await.unwrap();
    assert_eq!(files, Some(vec!["a.txt".to_string()]));
    assert_eq!(ls_files(git.path(), Instant::now()).await.unwrap(), None);
    let plain = TempDir::new().unwrap();
    let err = ls_files(plain.path(), soon()).await.unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
}

#[tokio::test]
async fn grep_hit_miss_out_of_time_and_failure() {
    let git = repo().await;
    let found = grep_first(git.path(), "needle", soon()).await.unwrap();
    assert_eq!(found, Grep::Hit("a.txt".to_string()));
    let missed = grep_first(git.path(), "-absent-", soon()).await.unwrap();
    assert_eq!(missed, Grep::Miss);
    let late = grep_first(git.path(), "needle", Instant::now())
        .await
        .unwrap();
    assert_eq!(late, Grep::TimedOut);
    let plain = TempDir::new().unwrap();
    let err = grep_first(plain.path(), "needle", soon())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
}

async fn commit_file(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
    run("git", &["add", name], Some(dir)).await.unwrap();
}

#[tokio::test]
async fn grep_matches_whole_tokens_only() {
    let git = repo().await;
    commit_file(
        git.path(),
        "a.txt",
        "import x from 'subquery.tsx'\nconst a = './info'\nmedia.ts\nquery.ts.bak\n",
    )
    .await;
    for needle in ["query.ts", "/info", "a.ts"] {
        let got = grep_first(git.path(), needle, soon()).await.unwrap();
        assert_eq!(got, Grep::Miss, "{needle}");
    }
    commit_file(
        git.path(),
        "b.txt",
        "see query.ts.\nget('/info')\nload(\"a.ts\")\nconst r = useSearch()\n",
    )
    .await;
    for needle in ["query.ts", "/info", "a.ts", "useSearch()"] {
        let got = grep_first(git.path(), needle, soon()).await.unwrap();
        assert_eq!(got, Grep::Hit("b.txt".to_string()), "{needle}");
    }
}

const CODE: &str = "child.kill_on_drop(true); React.useState(0); this.handleClick();
import b from '@shop/ui/button'
router.get('/api/login/:id')
";

#[tokio::test]
async fn names_match_after_a_dot_and_tokens_before_a_slash() {
    let git = repo().await;
    commit_file(git.path(), "code.js", CODE).await;
    for needle in [
        "kill_on_drop",
        "useState",
        "handleClick",
        "@shop/ui",
        "/api/login",
    ] {
        let got = grep_first(git.path(), needle, soon()).await.unwrap();
        assert_eq!(got, Grep::Hit("code.js".to_string()), "{needle}");
    }
}

#[test]
fn token_patterns_escape_regex_characters() {
    assert_eq!(
        token_pattern("f(x)*.ts"),
        r"(^|[^[:alnum:]_@-])f\(x\)\*\.ts($|[^[:alnum:]_.@-]|[.]($|[^[:alnum:]_]))"
    );
    assert_eq!(
        token_pattern("/info"),
        r"(^|[^[:alnum:]_./@-])/info($|[^[:alnum:]_.@-]|[.]($|[^[:alnum:]_]))"
    );
}

/// The token regex over the whole repo, without the fixed-string prefilter.
async fn regex_only(dir: &Path, needle: &str) -> Grep {
    let pattern = token_pattern(needle);
    let args = ["grep", "-l", "-z", "-E", "-I", "-e", &pattern];
    match run("git", &args, Some(dir)).await {
        Ok(out) => Grep::Hit(out.split('\0').next().unwrap().to_string()),
        Err(Error::Command { code: Some(1), .. }) => Grep::Miss,
        Err(e) => panic!("{e}"),
    }
}

#[tokio::test]
async fn the_prefilter_gives_the_same_answers_as_the_regex_alone() {
    let git = repo().await;
    let decoy = "subquery.tsx ./info media.ts @shop/uikit mykill_on_drop React_useState\n";
    for i in 0..BATCH + 2 {
        std::fs::write(git.path().join(format!("d{i:03}.txt")), decoy).unwrap();
    }
    std::fs::write(git.path().join("z.txt"), format!("{CODE}see query.ts.\n")).unwrap();
    run("git", &["add", "-A"], Some(git.path())).await.unwrap();
    for needle in [
        "query.ts",
        "/info",
        "a.ts",
        "kill_on_drop",
        "useState",
        "@shop/ui",
        "/api/login",
        "needle",
        "absent-thing",
    ] {
        let got = grep_first(git.path(), needle, soon()).await.unwrap();
        assert_eq!(got, regex_only(git.path(), needle).await, "{needle}");
    }
    let got = grep_first(git.path(), "kill_on_drop", soon())
        .await
        .unwrap();
    assert_eq!(got, Grep::Hit("z.txt".to_string()));
}
