use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Once;

use axum::http::StatusCode;
use serde_json::{json, Value};

mod common;
use common::{call, git, repo_at, unique, Env};

const TOKEN: &str = "tok-work-me";
const TOKEN_BASIC: &str = "eC1hY2Nlc3MtdG9rZW46dG9rLXdvcmstbWU=";

const FAKE_GIT: &str = r#"#!/bin/sh
here="$(dirname "$0")"
echo "argv=$* count=$GIT_CONFIG_COUNT key=$GIT_CONFIG_KEY_0 value=$GIT_CONFIG_VALUE_0" >> "$here/git.log"
n=$#
i=0
url=""
while [ $i -lt $n ]; do
  i=$((i+1))
  a="$1"
  shift
  if [ $i -eq $((n-1)) ]; then url="$a"; fi
  set -- "$@" "$a"
done
case "$url" in
  https://github.com/*)
    name="${url##*/}"
    name="${name%.git}"
    i=0
    while [ $i -lt $n ]; do
      i=$((i+1))
      a="$1"
      shift
      if [ $i -eq $((n-1)) ]; then a="$here/fixtures/$name"; fi
      set -- "$@" "$a"
    done;;
esac
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
    let dir = std::env::temp_dir().join(format!("townhall-plugins-{}", std::process::id()));
    ONCE.call_once(|| {
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        std::fs::create_dir_all(dir.join("template/hooks")).unwrap();
        for hook in ["post-checkout", "post-merge"] {
            script(
                &dir.join("template/hooks").join(hook),
                &format!("#!/bin/sh\ntouch \"{}/hook-ran\"\n", dir.display()),
            );
        }
        std::env::set_var("GIT_TEMPLATE_DIR", dir.join("template"));
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

fn hook_ran() -> bool {
    stubs().join("hook-ran").exists()
}

fn log_lines(needle: &str) -> Vec<String> {
    std::fs::read_to_string(stubs().join("git.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(needle))
        .map(str::to_string)
        .collect()
}

async fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]).await;
    git(
        dir,
        &[
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
        ],
    )
    .await;
}

fn manifest(name: &str, version: &str, permissions: Value) -> String {
    json!({"name": name, "version": version, "apiVersion": 1, "permissions": permissions})
        .to_string()
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn write_plugin(market: &Path, name: &str, version: &str, permissions: Value) {
    write(
        market,
        &format!("plugins/{name}/.townhall-plugin/plugin.json"),
        &manifest(name, version, permissions),
    );
}

fn write_index(market: &Path, name: &str, entries: Vec<Value>) {
    let index = json!({"name": name, "owner": {"name": "Acme"}, "plugins": entries});
    write(
        market,
        ".townhall-plugin/marketplace.json",
        &index.to_string(),
    );
}

fn entry(plugin: &str) -> Value {
    json!({"name": plugin, "source": format!("./plugins/{plugin}")})
}

async fn market_with(env: &Env, name: &str, plugins: &[(&str, Value)]) -> PathBuf {
    let dir = repo_at(env.root.path(), &unique("market")).await;
    for (plugin, permissions) in plugins {
        write_plugin(&dir, plugin, "1.0.0", permissions.clone());
    }
    write_index(&dir, name, plugins.iter().map(|(p, _)| entry(p)).collect());
    commit(&dir, "index").await;
    dir
}

async fn add_market(app: &axum::Router, dir: &Path) {
    let (status, body) = call(
        app,
        "POST",
        "/api/marketplaces",
        Some(json!({"source": dir.to_string_lossy()})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

async fn preview(app: &axum::Router, id: &str) -> Value {
    let (status, body) = call(
        app,
        "GET",
        &format!("/api/plugins/preview?plugin={id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"].clone()
}

async fn install(app: &axum::Router, id: &str) -> Value {
    let hash = preview(app, id).await["permissionsHash"].clone();
    let (status, body) = call(
        app,
        "POST",
        "/api/plugins",
        Some(json!({"plugin": id, "approve": hash})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["data"].clone()
}

async fn installed(app: &axum::Router) -> Vec<Value> {
    let (status, body) = call(app, "GET", "/api/plugins", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"]["installed"].as_array().unwrap().clone()
}

fn cache(env: &Env, market: &str, plugin: &str) -> PathBuf {
    env.home
        .root()
        .join("plugins/cache")
        .join(market)
        .join(plugin)
}

fn sha_dirs(env: &Env, market: &str, plugin: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(cache(env, market, plugin))
        .map(|dir| {
            dir.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

async fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"]).await
}

#[tokio::test]
async fn preview_lists_the_permissions_and_a_hash_and_install_with_that_hash_succeeds_without_enabling(
) {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(
        &env,
        "acme",
        &[(
            "notes",
            json!({"api": ["tasks:read"], "hooks": ["task.merged"]}),
        )],
    )
    .await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;

    let shown = preview(&app, "notes@acme").await;
    assert_eq!(shown["plugin"], "notes@acme");
    assert_eq!(shown["version"], "1.0.0");
    assert_eq!(shown["sha"], head(&market).await);
    assert_eq!(
        shown["permissions"],
        json!(["api:tasks:read", "hook:task.merged"])
    );
    let hash = shown["permissionsHash"].as_str().unwrap().to_string();
    assert!(hash.starts_with("sha256:"), "{hash}");
    assert!(installed(&app).await.is_empty());
    assert!(
        sha_dirs(&env, "acme", "notes").is_empty(),
        "a preview leaves nothing in the cache"
    );

    let (status, body) = call(
        &app,
        "POST",
        "/api/plugins",
        Some(json!({"plugin": "notes@acme", "approve": hash})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let plugin = &body["data"];
    assert_eq!(plugin["id"], "notes@acme");
    assert_eq!(plugin["enabled"], false);
    assert_eq!(plugin["pinned"], false);
    assert_eq!(plugin["sha"], shown["sha"]);
    assert_eq!(plugin["permissionsHash"], hash);
    assert_eq!(plugin["permissions"], shown["permissions"]);
    assert_eq!(plugin["error"], Value::Null);

    let sha = shown["sha"].as_str().unwrap();
    assert_eq!(sha_dirs(&env, "acme", "notes"), [sha]);
    let tree = cache(&env, "acme", "notes").join(sha);
    assert!(tree.join(".townhall-plugin/plugin.json").exists());
    assert!(!tree.join(".git").exists());
    assert!(env.home.root().join("plugins/data/acme/notes").is_dir());
    assert!(!hook_ran());

    let (status, body) = call(&app, "GET", "/api/plugins", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let available: Vec<&Value> = body["data"]["available"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["marketplace"] == "acme")
        .collect();
    assert_eq!(available.len(), 1, "{body}");
    assert_eq!(available[0]["id"], "notes@acme");
    assert_eq!(available[0]["installed"], true);
    let official = body["data"]["available"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["marketplace"] == "townhall-official")
        .unwrap();
    assert_eq!(official["installed"], false);
    assert_eq!(body["data"]["installed"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_wrong_or_missing_hash_is_a_400_and_nothing_is_kept() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({"api": ["tasks:read"]}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;

    for body in [
        json!({"plugin": "notes@acme", "approve": "sha256:00"}),
        json!({"plugin": "notes@acme", "approve": ""}),
        json!({"plugin": "notes@acme"}),
    ] {
        let (status, out) = call(&app, "POST", "/api/plugins", Some(body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {out}");
    }
    let (_, out) = call(
        &app,
        "POST",
        "/api/plugins",
        Some(json!({"plugin": "notes@acme", "approve": "sha256:00"})),
    )
    .await;
    assert!(
        out["error"].as_str().unwrap().contains("permissions"),
        "{out}"
    );
    assert!(installed(&app).await.is_empty());
    assert!(sha_dirs(&env, "acme", "notes").is_empty());
}

#[tokio::test]
async fn the_hash_of_a_list_that_moved_between_preview_and_install_is_refused() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({"api": ["tasks:read"]}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    let old = preview(&app, "notes@acme").await["permissionsHash"].clone();

    write_plugin(
        &market,
        "notes",
        "1.1.0",
        json!({"api": ["tasks:read", "tasks:create"]}),
    );
    commit(&market, "grow").await;
    let (status, out) = call(
        &app,
        "POST",
        "/api/plugins",
        Some(json!({"plugin": "notes@acme", "approve": old})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
    assert!(installed(&app).await.is_empty());
}

#[tokio::test]
async fn install_never_runs_a_file_from_the_plugin() {
    let stubs = stubs();
    let marker = stubs.join(unique("postinstall-marker"));
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({}))]).await;
    let script_text = format!("#!/bin/sh\ntouch \"{}\"\n", marker.display());
    let plugin = market.join("plugins/notes");
    write(
        &plugin,
        "package.json",
        &json!({"name": "notes", "scripts": {"postinstall": format!("touch {}", marker.display())}})
            .to_string(),
    );
    for file in ["postinstall", "postinstall.sh", "install.sh", "bin/setup"] {
        write(&plugin, file, &script_text);
        std::fs::set_permissions(plugin.join(file), std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }
    commit(&market, "postinstall").await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;

    install(&app, "notes@acme").await;

    let tree = cache(&env, "acme", "notes").join(head(&market).await);
    assert!(tree.join("postinstall.sh").exists());
    assert!(!marker.exists(), "a file of the plugin ran");
    assert!(
        !hook_ran(),
        "a git hook of the clone ran: install has to turn them off"
    );
}

#[tokio::test]
async fn a_symlink_out_of_the_plugin_or_a_submodule_refuses_the_install() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(
        &env,
        "acme",
        &[("linked", json!({})), ("modular", json!({}))],
    )
    .await;
    std::os::unix::fs::symlink("/etc", market.join("plugins/linked/docs")).unwrap();
    git(&market, &["add", "-A"]).await;
    git(
        &market,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,plugins/modular/vendor/sub",
        ],
    )
    .await;
    git(
        &market,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "bad ones",
        ],
    )
    .await;
    let listing = git(&market, &["ls-tree", "-r", "HEAD", "plugins/modular"]).await;
    assert!(listing.contains("160000 commit"), "{listing}");
    let (_state, app) = env.start();
    add_market(&app, &market).await;

    for (plugin, wanted) in [("linked", "outside"), ("modular", "submodule")] {
        let (status, out) = call(
            &app,
            "GET",
            &format!("/api/plugins/preview?plugin={plugin}@acme"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{plugin}: {out}");
        assert!(out["error"].as_str().unwrap().contains(wanted), "{out}");
        let (status, out) = call(
            &app,
            "POST",
            "/api/plugins",
            Some(json!({"plugin": format!("{plugin}@acme"), "approve": "sha256:x"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{plugin}: {out}");
        assert!(sha_dirs(&env, "acme", plugin).is_empty());
    }
    assert!(installed(&app).await.is_empty());
}

#[tokio::test]
async fn an_unknown_plugin_marketplace_or_second_install_is_refused() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;

    for id in ["ghost@acme", "notes@nowhere"] {
        let (status, out) = call(
            &app,
            "GET",
            &format!("/api/plugins/preview?plugin={id}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {out}");
    }
    let (status, out) = call(&app, "GET", "/api/plugins/preview?plugin=notes", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
    let (status, out) = call(&app, "GET", "/api/plugins/preview", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");

    install(&app, "notes@acme").await;
    let (status, out) = call(
        &app,
        "POST",
        "/api/plugins",
        Some(json!({"plugin": "notes@acme", "approve": "sha256:x"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{out}");
}

#[tokio::test]
async fn enable_disable_and_pin_change_only_what_was_sent() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    install(&app, "notes@acme").await;

    let (status, out) = call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["enabled"], true);
    assert_eq!(out["data"]["pinned"], false);

    let (status, out) = call(
        &app,
        "PATCH",
        "/api/plugins/Notes@ACME",
        Some(json!({"pinned": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["enabled"], true);
    assert_eq!(out["data"]["pinned"], true);

    let (status, out) = call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["enabled"], false);
    assert_eq!(out["data"]["pinned"], true);

    for (body, want) in [
        (json!({}), StatusCode::BAD_REQUEST),
        (json!({"enabled": "yes"}), StatusCode::BAD_REQUEST),
        (json!({"colour": 1}), StatusCode::BAD_REQUEST),
    ] {
        let (status, out) = call(&app, "PATCH", "/api/plugins/notes@acme", Some(body)).await;
        assert_eq!(status, want, "{out}");
    }
    let (status, _) = call(
        &app,
        "PATCH",
        "/api/plugins/ghost@acme",
        Some(json!({"enabled": true})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn update(app: &axum::Router, id: &str, approve: Option<&str>) -> Value {
    let uri = match approve {
        Some(hash) => format!("/api/plugins/{id}/update?approve={hash}"),
        None => format!("/api/plugins/{id}/update"),
    };
    let (status, body) = call(app, "POST", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["data"].clone()
}

#[tokio::test]
async fn an_update_with_the_same_permissions_switches_to_the_new_sha_and_stays_enabled() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({"api": ["tasks:read"]}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    let first = install(&app, "notes@acme").await["sha"].clone();
    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": true})),
    )
    .await;

    let same = update(&app, "notes@acme", None).await;
    assert_eq!(same["outcome"], "unchanged", "{same}");
    assert_eq!(same["plugin"]["sha"], first);

    write_plugin(&market, "notes", "1.1.0", json!({"api": ["tasks:read"]}));
    commit(&market, "1.1.0").await;
    let second = head(&market).await;
    let out = update(&app, "notes@acme", None).await;
    assert_eq!(out["outcome"], "updated", "{out}");
    assert_eq!(out["sha"], second);
    assert_eq!(out["plugin"]["sha"], second);
    assert_eq!(out["plugin"]["version"], "1.1.0");
    assert_eq!(out["plugin"]["enabled"], true);
    assert_eq!(out["plugin"]["error"], Value::Null);
    assert_eq!(sha_dirs(&env, "acme", "notes"), [second.as_str()]);
    assert_eq!(installed(&app).await[0]["sha"], second);
}

#[tokio::test]
async fn an_update_that_drops_a_permission_switches_too() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(
        &env,
        "acme",
        &[("notes", json!({"api": ["tasks:read", "tasks:create"]}))],
    )
    .await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    install(&app, "notes@acme").await;

    write_plugin(&market, "notes", "1.1.0", json!({"api": ["tasks:read"]}));
    commit(&market, "shrink").await;
    let out = update(&app, "notes@acme", None).await;
    assert_eq!(out["outcome"], "updated", "{out}");
    assert_eq!(out["plugin"]["permissions"], json!(["api:tasks:read"]));
    assert_eq!(
        out["plugin"]["permissionsHash"],
        preview(&app, "notes@acme").await["permissionsHash"]
    );
}

#[tokio::test]
async fn an_update_with_an_added_permission_stays_on_the_old_sha_until_it_is_approved() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({"api": ["tasks:read"]}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    let old = install(&app, "notes@acme").await;
    let old_sha = old["sha"].as_str().unwrap().to_string();
    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": true})),
    )
    .await;

    write_plugin(
        &market,
        "notes",
        "2.0.0",
        json!({"api": ["tasks:read", "tasks:create"]}),
    );
    commit(&market, "grow").await;
    let new_sha = head(&market).await;

    let out = update(&app, "notes@acme", None).await;
    assert_eq!(out["outcome"], "needsApproval", "{out}");
    assert_eq!(out["sha"], new_sha);
    assert_eq!(out["added"], json!(["api:tasks:create"]));
    assert_eq!(out["plugin"]["sha"], old_sha);
    assert_eq!(out["plugin"]["enabled"], true);
    assert_eq!(
        out["plugin"]["error"],
        format!("update to {new_sha} needs approval")
    );
    assert_eq!(out["plugin"]["permissionsHash"], old["permissionsHash"]);
    assert_eq!(sha_dirs(&env, "acme", "notes"), [old_sha.as_str()]);
    let listed = installed(&app).await;
    assert_eq!(listed[0]["sha"], old_sha);
    assert_eq!(
        listed[0]["error"],
        format!("update to {new_sha} needs approval")
    );

    let (status, body) = call(
        &app,
        "POST",
        "/api/plugins/notes@acme/update?approve=sha256:00",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(installed(&app).await[0]["sha"], old_sha);

    let hash = out["permissionsHash"].as_str().unwrap();
    let approved = update(&app, "notes@acme", Some(hash)).await;
    assert_eq!(approved["outcome"], "updated", "{approved}");
    assert_eq!(approved["plugin"]["sha"], new_sha);
    assert_eq!(approved["plugin"]["version"], "2.0.0");
    assert_eq!(approved["plugin"]["enabled"], true);
    assert_eq!(approved["plugin"]["error"], Value::Null);
    assert_eq!(
        approved["plugin"]["permissions"],
        json!(["api:tasks:create", "api:tasks:read"])
    );
    assert_eq!(sha_dirs(&env, "acme", "notes"), [new_sha.as_str()]);
}

#[tokio::test]
async fn a_pinned_plugin_is_not_updated() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({"api": ["tasks:read"]}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    let old_sha = install(&app, "notes@acme").await["sha"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"pinned": true})),
    )
    .await;

    write_plugin(&market, "notes", "1.1.0", json!({"api": ["tasks:read"]}));
    commit(&market, "1.1.0").await;
    let out = update(&app, "notes@acme", None).await;
    assert_eq!(out["outcome"], "pinned", "{out}");
    assert_eq!(out["plugin"]["sha"], old_sha);
    assert_eq!(installed(&app).await[0]["sha"], old_sha);
    assert_eq!(sha_dirs(&env, "acme", "notes"), [old_sha.as_str()]);

    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"pinned": false})),
    )
    .await;
    let out = update(&app, "notes@acme", None).await;
    assert_eq!(out["outcome"], "updated", "{out}");
}

#[tokio::test]
async fn uninstall_removes_the_cache_and_keeps_data_unless_purged() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({})), ("other", json!({}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    install(&app, "notes@acme").await;
    install(&app, "other@acme").await;
    call(
        &app,
        "PATCH",
        "/api/plugins/notes@acme",
        Some(json!({"enabled": true})),
    )
    .await;
    let data = env.home.root().join("plugins/data/acme/notes");
    write(&data, "state.json", "{}");
    let other_data = env.home.root().join("plugins/data/acme/other");
    write(&other_data, "state.json", "{}");

    let (status, out) = call(&app, "DELETE", "/api/plugins/notes@acme", None).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["id"], "notes@acme");
    assert_eq!(out["data"]["enabled"], false);
    assert!(!cache(&env, "acme", "notes").exists());
    assert!(data.join("state.json").exists());
    assert!(cache(&env, "acme", "other").exists());
    let ids: Vec<Value> = installed(&app)
        .await
        .iter()
        .map(|p| p["id"].clone())
        .collect();
    assert_eq!(ids, [json!("other@acme")]);

    install(&app, "notes@acme").await;
    let (status, out) = call(&app, "DELETE", "/api/plugins/notes@acme?purge=true", None).await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert!(!cache(&env, "acme", "notes").exists());
    assert!(!data.exists());
    assert!(other_data.join("state.json").exists());

    let (status, _) = call(&app, "DELETE", "/api/plugins/notes@acme", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_marketplace_with_an_installed_plugin_cannot_be_removed_until_it_is_uninstalled() {
    stubs();
    let env = Env::new("shop").await;
    let market = market_with(&env, "acme", &[("notes", json!({}))]).await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    install(&app, "notes@acme").await;
    let (status, _) = call(&app, "DELETE", "/api/marketplaces/acme", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    call(&app, "DELETE", "/api/plugins/notes@acme", None).await;
    let (status, _) = call(&app, "DELETE", "/api/marketplaces/acme", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_github_source_is_cloned_over_the_registered_account_without_hooks_or_submodules() {
    let stubs = stubs();
    let env = Env::new("shop").await;
    let repo = unique("remote-plugin");
    let remote = repo_at(&stubs.join("fixtures"), &repo).await;
    write(
        &remote,
        ".townhall-plugin/plugin.json",
        &manifest("remote", "3.0.0", json!({"api": ["tasks:read"]})),
    );
    commit(&remote, "plugin").await;
    let first = head(&remote).await;
    write(&remote, "NEXT", "later");
    commit(&remote, "later").await;
    let latest = head(&remote).await;

    let market = repo_at(env.root.path(), &unique("market")).await;
    let source = json!({"source": "github", "repo": format!("acme/{repo}")});
    let pinned = json!({"source": "github", "repo": format!("acme/{repo}"), "sha": first});
    write_index(
        &market,
        "acme",
        vec![
            json!({"name": "tip", "source": source}),
            json!({"name": "pinned", "source": pinned}),
        ],
    );
    commit(&market, "index").await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    let (status, body) = call(
        &app,
        "PUT",
        "/api/settings/forgeAccounts",
        Some(json!({"value": [{"forge": "github", "login": "work-me"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let tip = install(&app, "tip@acme").await;
    assert_eq!(tip["sha"], latest);
    assert_eq!(tip["version"], "3.0.0");
    let pin = install(&app, "pinned@acme").await;
    assert_eq!(pin["sha"], first);
    let tree = cache(&env, "acme", "pinned").join(&first);
    assert!(tree.join(".townhall-plugin/plugin.json").exists());
    assert!(!tree.join("NEXT").exists());

    let clones: Vec<String> = log_lines(&repo)
        .into_iter()
        .filter(|l| l.contains("clone"))
        .collect();
    assert!(clones.len() >= 2, "{clones:?}");
    for line in &clones {
        assert!(line.contains("core.hooksPath=/dev/null"), "{line}");
        assert!(line.contains("--no-recurse-submodules"), "{line}");
        assert!(line.contains("count=1 key=http.extraheader"), "{line}");
        assert!(
            line.ends_with(&format!("value=Authorization: Basic {TOKEN_BASIC}")),
            "{line}"
        );
        let argv = line.split(" count=").next().unwrap();
        assert!(
            !argv.contains(TOKEN) && !argv.contains(TOKEN_BASIC),
            "{argv}"
        );
    }
    for line in log_lines(&repo) {
        assert!(line.contains("core.hooksPath=/dev/null"), "{line}");
    }
}

#[tokio::test]
async fn a_private_plugin_repo_the_account_cannot_read_says_so_and_does_not_retry() {
    let stubs = stubs();
    let env = Env::new("shop").await;
    let market = repo_at(env.root.path(), &unique("market")).await;
    let missing = unique("no-such-plugin");
    write_index(
        &market,
        "acme",
        vec![
            json!({"name": "secret", "source": {"source": "github", "repo": format!("acme/{missing}")}}),
        ],
    );
    commit(&market, "index").await;
    let (_state, app) = env.start();
    add_market(&app, &market).await;
    call(
        &app,
        "PUT",
        "/api/settings/forgeAccounts",
        Some(json!({"value": [{"forge": "github", "login": "work-me"}]})),
    )
    .await;

    let (status, out) = call(&app, "GET", "/api/plugins/preview?plugin=secret@acme", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{out}");
    assert!(
        out["error"]
            .as_str()
            .unwrap()
            .contains("not found or not readable"),
        "{out}"
    );
    assert!(!out["error"].as_str().unwrap().contains(TOKEN));
    assert_eq!(
        log_lines(&missing)
            .iter()
            .filter(|l| l.contains("clone"))
            .count(),
        1
    );
    assert!(stubs.exists());
}
