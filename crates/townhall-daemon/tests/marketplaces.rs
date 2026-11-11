use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Once;

use axum::http::StatusCode;
use serde_json::{json, Value};

mod common;
use common::{call, git, repo_at, unique, Env};

const OFFICIAL_SHA: &str = "d999cc02bb20351f320ca3c180e31927cda4871d";
const TOKEN: &str = "tok-work-me";
const TOKEN_BASIC: &str = "eC1hY2Nlc3MtdG9rZW46dG9rLXdvcmstbWU=";

const FAKE_GIT: &str = r#"#!/bin/sh
here="$(dirname "$0")"
echo "argv=$* count=$GIT_CONFIG_COUNT key=$GIT_CONFIG_KEY_0 value=$GIT_CONFIG_VALUE_0" >> "$here/git.log"
case "$*" in
  *townhall-plugins*)
    echo "fatal: unable to access 'https://github.com/': Could not resolve host: github.com" >&2
    exit 128;;
esac
if [ "$1" = clone ]; then
  case "$4" in
    https://github.com/*|git@github.com:*)
      name="${4##*/}"
      name="${name%.git}"
      cp -R "$here/fixtures/$name" "$5" || exit 1
      exit 0;;
  esac
fi
exec git "$@"
"#;

const FAKE_GH: &str = r#"#!/bin/sh
if [ "$2" = token ]; then
  case "$4" in
    work-me) echo "tok-work-me"; exit 0;;
  esac
  echo "no login for $4" >&2
  exit 1
fi
exit 0
"#;

fn script(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn stubs() -> PathBuf {
    static ONCE: Once = Once::new();
    let dir = std::env::temp_dir().join(format!("townhall-marketplaces-{}", std::process::id()));
    ONCE.call_once(|| {
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        script(&dir.join("git"), FAKE_GIT);
        script(&dir.join("gh"), FAKE_GH);
        std::env::set_var("TOWNHALL_GIT_BIN", dir.join("git"));
        std::env::set_var("TOWNHALL_GH_BIN", dir.join("gh"));
        std::env::set_var("TOWNHALL_GLAB_BIN", dir.join("missing-glab"));
    });
    dir
}

fn git_log() -> String {
    std::fs::read_to_string(stubs().join("git.log")).unwrap_or_default()
}

fn log_lines(needle: &str) -> Vec<String> {
    git_log()
        .lines()
        .filter(|l| l.contains(needle))
        .map(str::to_string)
        .collect()
}

fn write_index(dir: &Path, name: &str, plugins: &[&str]) {
    let entries: Vec<Value> = plugins
        .iter()
        .map(|p| json!({"name": p, "description": format!("the {p} plugin"), "source": format!("./plugins/{p}")}))
        .collect();
    let index = json!({"name": name, "owner": {"name": "Acme"}, "plugins": entries});
    std::fs::create_dir_all(dir.join(".townhall-plugin")).unwrap();
    std::fs::write(
        dir.join(".townhall-plugin/marketplace.json"),
        index.to_string(),
    )
    .unwrap();
}

async fn commit(dir: &Path, message: &str, amend: bool) {
    git(dir, &["add", "-A"]).await;
    let mut args = vec![
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "-m",
        message,
    ];
    if amend {
        args.push("--amend");
    }
    git(dir, &args).await;
}

async fn market_at(parent: &Path, folder: &str, name: &str, plugins: &[&str]) -> PathBuf {
    let dir = repo_at(parent, folder).await;
    write_index(&dir, name, plugins);
    commit(&dir, "index", false).await;
    dir
}

fn plugin_names(market: &Value) -> Vec<String> {
    market["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

fn market_names(list: &Value) -> Vec<String> {
    list["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap().to_string())
        .collect()
}

async fn list(app: &axum::Router) -> Value {
    let (status, body) = call(app, "GET", "/api/marketplaces", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn add(app: &axum::Router, body: Value) -> (StatusCode, Value) {
    call(app, "POST", "/api/marketplaces", Some(body)).await
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        match entry.file_type().unwrap() {
            t if t.is_dir() => files_under(&path, out),
            t if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[tokio::test]
async fn an_offline_start_lists_the_official_marketplace_from_its_snapshot() {
    stubs();
    let env = Env::new("shop").await;
    let (_state, app) = env.start();
    let body = list(&app).await;
    let markets = body["data"].as_array().unwrap();
    assert_eq!(markets.len(), 1, "{body}");
    let official = &markets[0];
    assert_eq!(official["name"], "townhall-official");
    assert_eq!(official["official"], true);
    assert_eq!(official["enabled"], true);
    assert_eq!(official["account"], Value::Null);
    assert_eq!(official["headSha"], OFFICIAL_SHA);
    assert_eq!(official["fetchedAt"], Value::Null);
    assert_eq!(official["error"], Value::Null);
    assert_eq!(
        official["source"],
        json!({"source": "github", "repo": "WilliamThogersen/townhall-plugins"})
    );
    assert_eq!(
        plugin_names(official),
        [
            "theme-slate",
            "starter-prompts",
            "agent-types",
            "sample-full"
        ]
    );
    let root = env.root.path().to_string_lossy().into_owned();
    assert!(
        log_lines(&root).is_empty(),
        "git ran: {:?}",
        log_lines(&root)
    );
}

#[tokio::test]
async fn a_local_fixture_repo_is_added_listed_and_removed_without_touching_the_folder() {
    stubs();
    let env = Env::new("shop").await;
    let fixture = market_at(
        env.root.path(),
        "acme-src",
        "acme-plugins",
        &["lint", "fmt"],
    )
    .await;
    let (_state, app) = env.start();

    let (status, body) = add(&app, json!({"source": fixture.to_string_lossy()})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let added = &body["data"];
    assert_eq!(added["name"], "acme-plugins");
    assert_eq!(added["official"], false);
    assert_eq!(added["enabled"], true);
    assert_eq!(
        added["source"],
        json!({"source": "directory", "path": fixture.to_string_lossy()})
    );
    assert_eq!(added["headSha"].as_str().unwrap().len(), 40, "{added}");
    assert_eq!(plugin_names(added), ["lint", "fmt"]);

    let body = list(&app).await;
    assert_eq!(market_names(&body), ["townhall-official", "acme-plugins"]);

    let (status, body) = call(&app, "DELETE", "/api/marketplaces/acme-plugins", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["name"], "acme-plugins");
    assert_eq!(market_names(&list(&app).await), ["townhall-official"]);
    assert!(fixture.join(".townhall-plugin/marketplace.json").exists());
}

#[tokio::test]
async fn a_plain_folder_with_a_claude_index_is_added_and_the_name_can_be_chosen() {
    stubs();
    let env = Env::new("shop").await;
    let plain = env.root.path().join("plain");
    std::fs::create_dir_all(plain.join(".claude-plugin")).unwrap();
    let index = json!({"name": "from-claude", "owner": {"name": "Acme"}, "plugins": []});
    std::fs::write(
        plain.join(".claude-plugin/marketplace.json"),
        index.to_string(),
    )
    .unwrap();
    let (_state, app) = env.start();
    let (status, body) = add(
        &app,
        json!({"source": plain.to_string_lossy(), "name": "mine"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["name"], "mine");
    assert_eq!(body["data"]["headSha"], Value::Null);
    assert_eq!(plugin_names(&body["data"]), Vec::<String>::new());
}

#[tokio::test]
async fn a_second_add_of_the_same_name_is_a_conflict_ignoring_case() {
    stubs();
    let env = Env::new("shop").await;
    let one = market_at(env.root.path(), "one", "acme-plugins", &["lint"]).await;
    let two = market_at(env.root.path(), "two", "other", &["fmt"]).await;
    let (_state, app) = env.start();
    let (status, body) = add(&app, json!({"source": one.to_string_lossy()})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = add(&app, json!({"source": one.to_string_lossy()})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(body["error"].as_str().unwrap().contains("acme-plugins"));

    let (status, body) = add(
        &app,
        json!({"source": two.to_string_lossy(), "name": "ACME-Plugins"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(market_names(&list(&app).await).len(), 2);
}

#[tokio::test]
async fn townhall_official_cannot_be_removed_or_reused() {
    stubs();
    let env = Env::new("shop").await;
    let squatter = market_at(env.root.path(), "squat", "townhall-official", &["evil"]).await;
    let honest = market_at(env.root.path(), "honest", "acme-plugins", &["lint"]).await;
    let (_state, app) = env.start();

    let (status, body) = add(&app, json!({"source": squatter.to_string_lossy()})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = add(
        &app,
        json!({"source": honest.to_string_lossy(), "name": "Townhall-Official"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    for name in ["townhall-official", "TOWNHALL-OFFICIAL"] {
        let (status, body) = call(&app, "DELETE", &format!("/api/marketplaces/{name}"), None).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert!(
            body["error"].as_str().unwrap().contains("built in"),
            "{body}"
        );
    }
    let body = list(&app).await;
    assert_eq!(market_names(&body), ["townhall-official"]);
    assert_eq!(plugin_names(&body["data"][0]).len(), 4);
}

#[tokio::test]
async fn a_git_url_is_cloned_under_the_plugins_folder_and_refresh_follows_even_a_rewritten_history()
{
    stubs();
    let env = Env::new("shop").await;
    let fixture = market_at(env.root.path(), "acme-src", "acme-plugins", &["lint"]).await;
    let (_state, app) = env.start();
    let url = format!("file://{}", fixture.display());

    let (status, body) = add(&app, json!({"source": url})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["source"], json!({"source": "url", "url": url}));
    let first = body["data"]["headSha"].as_str().unwrap().to_string();
    assert!(body["data"]["fetchedAt"].as_str().is_some());
    let clone = env.home.root().join("plugins/marketplaces/acme-plugins");
    assert!(clone.join(".git").exists());
    assert_eq!(plugin_names(&body["data"]), ["lint"]);

    write_index(&fixture, "acme-plugins", &["lint", "fmt"]);
    commit(&fixture, "add fmt", false).await;
    let (status, body) = call(&app, "POST", "/api/marketplaces/acme-plugins/refresh", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(plugin_names(&body["data"]), ["lint", "fmt"]);
    let second = body["data"]["headSha"].as_str().unwrap().to_string();
    assert_ne!(first, second);

    write_index(&fixture, "acme-plugins", &["fmt"]);
    commit(&fixture, "rewritten", true).await;
    let (status, body) = call(&app, "POST", "/api/marketplaces/acme-plugins/refresh", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(plugin_names(&body["data"]), ["fmt"]);
    assert_ne!(body["data"]["headSha"].as_str().unwrap(), second);

    let (status, _) = call(&app, "DELETE", "/api/marketplaces/acme-plugins", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!clone.exists());
    assert!(fixture.join(".git").exists());
}

#[tokio::test]
async fn refreshing_a_local_folder_reads_its_index_again() {
    stubs();
    let env = Env::new("shop").await;
    let fixture = market_at(env.root.path(), "acme-src", "acme-plugins", &["lint"]).await;
    let (_state, app) = env.start();
    let (status, body) = add(&app, json!({"source": fixture.to_string_lossy()})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    write_index(&fixture, "acme-plugins", &["lint", "fmt"]);
    let (status, body) = call(&app, "POST", "/api/marketplaces/acme-plugins/refresh", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(plugin_names(&body["data"]), ["lint", "fmt"]);
    let (status, body) = call(&app, "POST", "/api/marketplaces/nope/refresh", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn a_refresh_that_cannot_reach_the_network_keeps_the_snapshot_and_says_why() {
    stubs();
    let env = Env::new("shop").await;
    let (_state, app) = env.start();
    let (status, body) = call(
        &app,
        "POST",
        "/api/marketplaces/townhall-official/refresh",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Could not resolve host"),
        "{body}"
    );
    let body = list(&app).await;
    let official = &body["data"][0];
    assert_eq!(plugin_names(official).len(), 4);
    assert!(official["error"]
        .as_str()
        .unwrap()
        .contains("Could not resolve host"));
    assert_eq!(official["headSha"], OFFICIAL_SHA);
}

#[tokio::test]
async fn a_refused_add_stores_nothing() {
    stubs();
    let env = Env::new("shop").await;
    let empty = env.root.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let broken = env.root.path().join("broken");
    std::fs::create_dir_all(broken.join(".townhall-plugin")).unwrap();
    std::fs::write(broken.join(".townhall-plugin/marketplace.json"), "{ nope").unwrap();
    let (_state, app) = env.start();
    for (source, wanted) in [
        (empty.to_string_lossy().into_owned(), "marketplace.json"),
        (broken.to_string_lossy().into_owned(), "marketplace.json"),
        ("/no/such/folder".to_string(), "cannot open"),
        ("./relative/folder".to_string(), "owner/repo"),
        ("relative".to_string(), "owner/repo"),
    ] {
        let (status, body) = add(&app, json!({"source": source})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{source}: {body}");
        assert!(body["error"].as_str().unwrap().contains(wanted), "{body}");
    }
    let (status, body) = add(&app, json!({"source": "/x", "nope": 1})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(market_names(&list(&app).await), ["townhall-official"]);
}

#[tokio::test]
async fn a_private_https_remote_gets_its_token_through_git_config_env_and_never_argv_or_files() {
    let stubs = stubs();
    let env = Env::new("shop").await;
    let repo = unique("private-market");
    let fixture = market_at(&stubs.join("fixtures"), &repo, "secret-plugins", &["lint"]).await;
    assert!(fixture.exists());
    let (_state, app) = env.start();
    let (status, body) = call(
        &app,
        "PUT",
        "/api/settings/forgeAccounts",
        Some(json!({"value": [{"forge": "github", "login": "work-me"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let url = format!("https://github.com/acme/{repo}.git");
    let (status, body) = add(&app, json!({"source": url})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["name"], "secret-plugins");
    assert_eq!(body["data"]["account"], Value::Null);
    assert_eq!(plugin_names(&body["data"]), ["lint"]);

    let lines = log_lines(&repo);
    let clone = lines
        .iter()
        .find(|l| l.contains("argv=clone"))
        .unwrap_or_else(|| panic!("no clone in {lines:?}"));
    assert!(clone.contains("count=1 key=http.extraheader"), "{clone}");
    assert!(
        clone.ends_with(&format!("value=Authorization: Basic {TOKEN_BASIC}")),
        "{clone}"
    );
    for line in &lines {
        let argv = line.split(" count=").next().unwrap();
        assert!(!argv.contains(TOKEN), "token in argv: {argv}");
        assert!(!argv.contains(TOKEN_BASIC), "token in argv: {argv}");
    }

    let mut files = Vec::new();
    files_under(env.home.root(), &mut files);
    assert!(!files.is_empty());
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        assert!(
            !contains(&bytes, TOKEN),
            "{} holds the token",
            file.display()
        );
        assert!(
            !contains(&bytes, TOKEN_BASIC),
            "{} holds the token",
            file.display()
        );
    }
}

#[tokio::test]
async fn the_account_is_the_markets_own_pick_and_ssh_remotes_ignore_it() {
    let stubs = stubs();
    let env = Env::new("shop").await;
    let repo = unique("ssh-market");
    market_at(&stubs.join("fixtures"), &repo, "ssh-plugins", &["lint"]).await;
    let (_state, app) = env.start();
    call(
        &app,
        "PUT",
        "/api/settings/forgeAccounts",
        Some(json!({"value": [{"forge": "github", "login": "work-me"}]})),
    )
    .await;

    let (status, body) = add(
        &app,
        json!({"source": format!("git@github.com:acme/{repo}.git"), "account": "work-me"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["data"]["account"], "work-me");
    let lines = log_lines(&repo);
    let clone = lines.iter().find(|l| l.contains("argv=clone")).unwrap();
    assert!(clone.contains("count= key= value="), "{clone}");

    let (status, body) = add(
        &app,
        json!({"source": "https://github.com/acme/never.git", "name": "never", "account": "ghost"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"].as_str().unwrap().contains("ghost"), "{body}");
    assert!(log_lines("acme/never").is_empty());
    assert_eq!(
        market_names(&list(&app).await),
        ["townhall-official", "ssh-plugins"]
    );
}
