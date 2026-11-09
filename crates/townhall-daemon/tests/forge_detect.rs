use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Mutex;

use axum::http::StatusCode;
use serde_json::{json, Value};

mod common;
use common::{call, Env};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn stub(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

fn point_at(gh: &str, glab: &str) {
    std::env::set_var("TOWNHALL_GH_BIN", gh);
    std::env::set_var("TOWNHALL_GLAB_BIN", glab);
}

async fn detected(env: &Env) -> Value {
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/forge-accounts/detected", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["data"].clone()
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

const GLAB_TEXT: &str = r#"cat >&2 <<'EOF'
gitlab.com
  ✓ Logged in to gitlab.com as gl-me (/home/me/.config/glab-cli/config.yml)
  ✓ Git operations for gitlab.com configured to use ssh protocol.
  ✓ API calls for gitlab.com are made over https protocol.
  ✓ REST API Endpoint: https://gitlab.com/api/v4/
  ✓ Token found: **************************
EOF"#;

#[tokio::test]
async fn every_logged_in_account_on_every_host_is_listed_and_registered_ones_are_marked() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(&stub(dir, "gh", GH_TEXT), &stub(dir, "glab", GLAB_TEXT));
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
}

#[tokio::test]
async fn machine_readable_output_is_used_when_gh_has_it() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let json_out = r#"if [ "$3" = "--json" ]; then echo '{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"json-me","tokenSource":"keyring","scopes":"repo","gitProtocol":"https"},{"state":"error","active":false,"host":"github.com","login":"expired-me","tokenSource":"keyring","scopes":"","gitProtocol":"https"}],"ghe.io":[{"state":"success","active":true,"host":"ghe.io","login":"ghe-me","tokenSource":"GH_ENTERPRISE_TOKEN","scopes":"","gitProtocol":"https"}]}}'; exit 0; fi; echo "text mode used" >&2; exit 1"#;
    point_at(&stub(dir, "gh", json_out), &stub(dir, "glab", "exit 1"));
    let got = detected(&env).await;
    let logins: Vec<_> = got
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["login"].as_str().unwrap(), a["host"].as_str().unwrap()))
        .collect();
    assert_eq!(logins, vec![("json-me", "github.com"), ("ghe-me", "ghe.io")]);
}

#[tokio::test]
async fn older_gh_that_writes_to_stderr_and_says_as_is_understood() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    let out = "echo 'You are not logged into any GitHub hosts. To log in, run: gh auth login' >&2; exit 1";
    let glab = "echo 'No token found for gitlab.com' >&2; exit 1";
    point_at(&stub(dir, "gh", out), &stub(dir, "glab", glab));
    assert_eq!(detected(&env).await, json!([]));
}

#[tokio::test]
async fn a_missing_binary_is_an_empty_list_not_an_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let env = Env::new("alpha").await;
    let dir = env.root.path();
    point_at(
        &dir.join("no-gh").to_string_lossy(),
        &stub(dir, "glab", GLAB_TEXT),
    );
    let got = detected(&env).await;
    assert_eq!(got[0]["login"], "gl-me");
    assert_eq!(got.as_array().unwrap().len(), 1);
}
