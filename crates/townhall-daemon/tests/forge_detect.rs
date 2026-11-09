use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use axum::http::StatusCode;
use serde_json::{json, Value};
use tokio::sync::Mutex;

mod common;
use common::{call, Env};

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

fn stub(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Points the daemon at stub programs and at config dirs that do not exist, so the machine's
/// real `gh` and `glab` config is never read; `glab_config` then provides one.
fn point_at(gh: &str, glab: &str) {
    std::env::set_var("TOWNHALL_GH_BIN", gh);
    std::env::set_var("TOWNHALL_GLAB_BIN", glab);
    std::env::set_var("GH_CONFIG_DIR", "/nonexistent-townhall-test/gh");
    std::env::set_var("GLAB_CONFIG_DIR", "/nonexistent-townhall-test/glab-cli");
}

fn glab_config(dir: &Path, text: &str) {
    let at = dir.join("cfg").join("glab-cli");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("config.yml"), text).unwrap();
    std::env::set_var("GLAB_CONFIG_DIR", &at);
}

async fn detected_body(env: &Env) -> Value {
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/forge-accounts/detected", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

async fn detected(env: &Env) -> Value {
    detected_body(env).await["data"].clone()
}

const GH_TEXT: &str = r#"cat <<'EOF'
github.com
  ✓ Logged in to github.com account work-me (keyring)
  - Active account: true
  - Git operations protocol: https
  - Token: gho_************************************
  - Token scopes: 'repo', 'read:org'

  ✓ Logged in to github.com account private-me (keyring)
  - Active account: false
  - Token: gho_************************************

github.corp.io
  ✓ Logged in to github.corp.io account corp-me (GH_ENTERPRISE_TOKEN)
  - Active account: true
EOF"#;

/// A glab whose `auth status` must never be called: it would hang, as it does on a dead host.
const GLAB_NO_STATUS: &str = "case \"$1\" in --version) echo 'glab 1.50';; api) exit 1;; *) echo 'auth status must not be called' >&2; sleep 60;; esac";

const GLAB_CONFIG: &str = "git_protocol: ssh\nhosts:\n    gitlab.com:\n        token: glpat-secret\n        user: gl-me\n";

#[tokio::test]
async fn every_logged_in_account_on_every_host_is_listed_and_registered_ones_are_marked() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &stub(dir, "gh", GH_TEXT),
        &stub(dir, "glab", GLAB_NO_STATUS),
    );
    glab_config(dir, GLAB_CONFIG);
    let (_state, app) = env.start();
    let put = json!({"value": [{"forge": "github", "login": "Work-Me"}]});
    let (s, _) = call(&app, "PUT", "/api/settings/forgeAccounts", Some(put)).await;
    assert_eq!(s, StatusCode::OK);
    let (s, v) = call(&app, "GET", "/api/forge-accounts/detected", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!([
            {"forge": "github", "login": "work-me", "host": "github.com", "registered": true},
            {"forge": "github", "login": "private-me", "host": "github.com", "registered": false},
            {"forge": "github", "login": "corp-me", "host": "github.corp.io", "registered": false},
            {"forge": "gitlab", "login": "gl-me", "host": "gitlab.com", "registered": false}
        ])
    );
    assert!(!v.to_string().contains("gho_"), "no token is returned: {v}");
    assert!(
        !v.to_string().contains("glpat"),
        "no token is returned: {v}"
    );
    assert!(v.get("warnings").is_none(), "nothing to warn about: {v}");
}

#[tokio::test]
async fn machine_readable_output_is_used_when_gh_has_it() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let json_out = r#"if [ "$3" = "--json" ]; then echo '{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"json-me","tokenSource":"keyring","scopes":"repo","gitProtocol":"https"},{"state":"error","active":false,"host":"github.com","login":"expired-me","tokenSource":"keyring","scopes":"","gitProtocol":"https"}],"ghe.io":[{"state":"success","active":true,"host":"ghe.io","login":"ghe-me","tokenSource":"GH_ENTERPRISE_TOKEN","scopes":"","gitProtocol":"https"}]}}'; exit 0; fi; echo "text mode used" >&2; exit 1"#;
    point_at(&stub(dir, "gh", json_out), &stub(dir, "glab", "exit 1"));
    let got = detected(&env).await;
    let mut logins: Vec<_> = got
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["login"].as_str().unwrap(), a["host"].as_str().unwrap()))
        .collect();
    logins.sort();
    assert_eq!(
        logins,
        vec![("ghe-me", "ghe.io"), ("json-me", "github.com")]
    );
}

#[tokio::test]
async fn older_gh_that_writes_to_stderr_and_says_as_is_understood() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let old = "if [ \"$3\" = \"--json\" ]; then echo \"unknown flag: --json\" >&2; exit 1; fi\necho 'github.com' >&2\necho '  ✓ Logged in to github.com as old-me (oauth_token)' >&2\necho '  ✓ Git operations for github.com configured to use https protocol.' >&2";
    point_at(&stub(dir, "gh", old), &stub(dir, "glab", "exit 1"));
    let got = detected(&env).await;
    assert_eq!(
        got,
        json!([{"forge": "github", "login": "old-me", "host": "github.com", "registered": false}])
    );
}

#[tokio::test]
async fn not_logged_in_is_an_empty_list() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let out = "echo 'You are not logged into any GitHub hosts. To log in, run: gh auth login' >&2; exit 1";
    let glab = "echo 'No token found for gitlab.com' >&2; exit 1";
    point_at(&stub(dir, "gh", out), &stub(dir, "glab", glab));
    assert_eq!(detected(&env).await, json!([]));
}

#[tokio::test]
async fn a_missing_binary_is_an_empty_list_not_an_error() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &dir.join("no-gh").to_string_lossy(),
        &dir.join("no-glab").to_string_lossy(),
    );
    assert_eq!(detected(&env).await, json!([]));
}

#[tokio::test]
async fn one_missing_binary_does_not_hide_the_other() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &dir.join("no-gh").to_string_lossy(),
        &stub(dir, "glab", GLAB_NO_STATUS),
    );
    glab_config(dir, GLAB_CONFIG);
    let got = detected(&env).await;
    assert_eq!(got[0]["login"], "gl-me");
    assert_eq!(got.as_array().unwrap().len(), 1);
}

const REAL_GH_JSON: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"WilliamThogersen","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"},{"state":"success","active":false,"host":"github.com","login":"wexowgt","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}]}}"#;

const REAL_GH_TEXT: &str = "github.com
  ✓ Logged in to github.com account WilliamThogersen (keyring)
  - Active account: true
  - Git operations protocol: https

  ✓ Logged in to github.com account wexowgt (keyring)
  - Active account: false
  - Git operations protocol: https
";

fn both_real_accounts() -> Value {
    json!([
        {"forge": "github", "login": "WilliamThogersen", "host": "github.com", "registered": false},
        {"forge": "github", "login": "wexowgt", "host": "github.com", "registered": false}
    ])
}

#[tokio::test]
async fn real_gh_2_101_json_lists_the_active_and_the_inactive_account() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let gh = format!(
        "if [ \"$1 $2 $3 $4\" = \"auth status --json hosts\" ]; then cat <<'EOF'\n{REAL_GH_JSON}\nEOF\nexit 0; fi; exit 1"
    );
    point_at(&stub(dir, "gh", &gh), &stub(dir, "glab", "exit 1"));
    assert_eq!(detected(&env).await, both_real_accounts());
}

#[tokio::test]
async fn real_gh_2_101_text_lists_both_accounts_when_json_is_unavailable() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let gh = format!(
        "if [ \"$3\" = \"--json\" ]; then echo 'unknown flag: --json' >&2; exit 1; fi\ncat <<'EOF'\n{REAL_GH_TEXT}EOF"
    );
    point_at(&stub(dir, "gh", &gh), &stub(dir, "glab", "exit 1"));
    assert_eq!(detected(&env).await, both_real_accounts());
}

/// The user's machine: gitlab.wexo.io has a user in the config, gitlab.com (logged out, dead)
/// has none and `glab api user` fails there.
const REAL_GLAB_CONFIG: &str = "git_protocol: ssh\nhosts:\n    gitlab.com:\n        api_host: gitlab.com\n    gitlab.wexo.io:\n        api_host: gitlab.wexo.io\n        git_protocol: ssh\n        user: wexowgt\n";

#[tokio::test]
async fn glab_is_read_from_its_config_and_a_dead_host_only_costs_a_warning() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &stub(dir, "gh", "exit 1"),
        &stub(dir, "glab", GLAB_NO_STATUS),
    );
    glab_config(dir, REAL_GLAB_CONFIG);
    let body = detected_body(&env).await;
    assert_eq!(
        body["data"],
        json!([{"forge": "gitlab", "login": "wexowgt", "host": "gitlab.wexo.io", "registered": false}])
    );
    let warnings = body["warnings"].to_string();
    assert!(
        warnings.contains("could not tell who is logged in on gitlab.com"),
        "{warnings}"
    );
    assert!(warnings.contains("gh:"), "{warnings}");
}

#[tokio::test]
async fn a_missing_glab_config_says_why_nothing_was_found() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &stub(dir, "gh", GH_TEXT),
        &stub(dir, "glab", GLAB_NO_STATUS),
    );
    let body = detected_body(&env).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 3, "{body}");
    assert_eq!(body["success"], true);
    let warnings = body["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{body}");
    assert!(
        warnings[0]
            .as_str()
            .unwrap()
            .starts_with("glab: no glab config at"),
        "{body}"
    );
}

#[tokio::test]
async fn a_hanging_glab_does_not_hide_gh_and_is_cut_off_at_the_timeout() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &stub(dir, "gh", GH_TEXT),
        &stub(dir, "glab", "exec sleep 60"),
    );
    let started = std::time::Instant::now();
    let got = detected(&env).await;
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert_eq!(got.as_array().unwrap().len(), 3, "{got}");
}

#[tokio::test]
async fn glab_status_on_stderr_with_two_hosts_lists_both_even_when_it_exits_non_zero() {
    let _guard = ENV_LOCK.lock().await;
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &stub(dir, "gh", "exit 1"),
        &stub(dir, "glab", GLAB_NO_STATUS),
    );
    glab_config(
        dir,
        "hosts:\n    gitlab.com:\n        user: gl-me\n    gitlab.corp.io:\n        user: corp-me\n",
    );
    assert_eq!(
        detected(&env).await,
        json!([
            {"forge": "gitlab", "login": "gl-me", "host": "gitlab.com", "registered": false},
            {"forge": "gitlab", "login": "corp-me", "host": "gitlab.corp.io", "registered": false}
        ])
    );
}
