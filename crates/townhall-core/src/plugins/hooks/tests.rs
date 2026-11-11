use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{allowed_env, fire, run, PAUSE_AFTER};
use crate::plugins::install::{cache_root, data_root};
use crate::store::{Marketplace, Plugin};
use crate::{Context, ContextOptions, Home, Workspace};

const SHA: &str = "abc123";
const ID: &str = "lint@acme";

struct Fixture {
    _dir: tempfile::TempDir,
    ctx: Context,
    data: PathBuf,
}

impl Fixture {
    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.data.join(file)).unwrap()
    }

    fn has(&self, file: &str) -> bool {
        self.data.join(file).exists()
    }

    async fn plugin(&self) -> Plugin {
        self.ctx.store().unwrap().plugin(ID).await.unwrap()
    }
}

fn hooks_file(event: &str, command: &str, timeout: u64) -> String {
    json!({"hooks": {event: [{"command": command, "timeout": timeout}]}}).to_string()
}

async fn install_with(hooks: &str, approved: Value, enabled: bool) -> Fixture {
    let dir = tempfile::TempDir::new().unwrap();
    let home = Home::at(dir.path().join("home"));
    let ws = Workspace::new(home.clone(), dir.path()).unwrap();
    let ctx = Context::open(ws, ContextOptions::default()).unwrap();
    let store = ctx.store().unwrap();
    store
        .add_marketplace(Marketplace {
            name: "acme".into(),
            source: json!({"source": "github", "repo": "acme/market"}),
            account: None,
            enabled: true,
            official: false,
            head_sha: None,
            fetched_at: None,
            error: None,
        })
        .await
        .unwrap();
    store
        .add_plugin(Plugin {
            id: String::new(),
            marketplace: "acme".into(),
            name: "lint".into(),
            version: Some("1.0.0".into()),
            sha: SHA.into(),
            source: json!({"source": "git-subdir", "path": "plugins/lint"}),
            enabled,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved,
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        })
        .await
        .unwrap();
    let root = cache_root(&home, "acme", "lint").unwrap().join(SHA);
    std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
    let manifest = json!({
        "name": "lint", "version": "1.0.0", "apiVersion": 1,
        "permissions": {"hooks": ["task.merged"]},
        "contributes": {"hooks": "townhall-hooks.json"}
    });
    std::fs::write(root.join(".townhall-plugin/plugin.json"), manifest.to_string()).unwrap();
    std::fs::write(root.join("townhall-hooks.json"), hooks).unwrap();
    let data = data_root(&home, "acme", "lint").unwrap();
    std::fs::create_dir_all(&data).unwrap();
    Fixture {
        _dir: dir,
        ctx,
        data,
    }
}

async fn install(command: &str, timeout: u64) -> Fixture {
    let hooks = hooks_file("task.merged", command, timeout);
    install_with(&hooks, json!(["hook:task.merged"]), true).await
}

fn merged() -> Value {
    json!({"task": "job", "base": "main"})
}

#[tokio::test]
async fn a_hook_receives_the_payload_on_stdin() {
    let f = install("cat > payload.json", 10).await;
    run(&f.ctx, "task.merged", merged()).await;
    let got: Value = serde_json::from_str(&f.read("payload.json")).unwrap();
    assert_eq!(got["event"], "task.merged");
    assert_eq!(got["plugin"], ID);
    assert_eq!(got["apiVersion"], 1);
    assert_eq!(got["task"], "job");
    assert_eq!(got["base"], "main");
    assert!(got["settings"].is_object(), "{got}");
}

#[tokio::test]
async fn a_hook_sees_its_own_variables_and_no_ambient_secrets() {
    let f = install("env > env.txt", 10).await;
    run(&f.ctx, "task.merged", merged()).await;
    let seen = f.read("env.txt");
    for wanted in [
        "TOWNHALL_EVENT=task.merged",
        "TOWNHALL_PLUGIN=lint@acme",
        "TOWNHALL_API_URL=http://127.0.0.1:7777",
    ] {
        assert!(seen.lines().any(|l| l == wanted), "{wanted} missing in {seen}");
    }
    for name in ["TOWNHALL_API_TOKEN", "TOWNHALL_PLUGIN_ROOT", "TOWNHALL_PLUGIN_DATA"] {
        assert!(seen.contains(&format!("{name}=")), "{name} missing in {seen}");
    }
    assert!(!seen.contains("ANTHROPIC_"), "{seen}");
    let data_line = format!("TOWNHALL_PLUGIN_DATA={}", f.data.display());
    assert!(seen.lines().any(|l| l == data_line), "{seen}");
}

#[test]
fn the_env_allow_list_never_carries_anthropic_or_forge_tokens() {
    let daemon = [
        ("PATH", "/bin"),
        ("HOME", "/home/u"),
        ("LANG", "en_US.UTF-8"),
        ("LC_ALL", "C"),
        ("TZ", "UTC"),
        ("ANTHROPIC_API_KEY", "sk-ant-secret"),
        ("GH_TOKEN", "ghp_secret"),
        ("GITHUB_TOKEN", "ghp_secret"),
        ("GITLAB_TOKEN", "glpat-secret"),
        ("SHELL", "/bin/zsh"),
        ("DECLARED", "yes"),
        ("UNDECLARED", "no"),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()));
    let declared = [
        "DECLARED".to_string(),
        "ANTHROPIC_API_KEY".to_string(),
        "ANTHROPIC_BASE_URL".to_string(),
        "GH_TOKEN".to_string(),
        "NOT_SET".to_string(),
    ];
    let got = allowed_env(daemon, &declared);
    let names: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        names,
        ["PATH", "HOME", "LANG", "LC_ALL", "TZ", "DECLARED"],
        "{got:?}"
    );
}

#[tokio::test]
async fn a_hanging_hook_is_killed_with_its_children_at_the_timeout() {
    let f = install("sleep 60 & echo $! > child.pid; wait", 1).await;
    let started = Instant::now();
    run(&f.ctx, "task.merged", merged()).await;
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    let pid: i32 = f.read("child.pid").trim().parse().unwrap();
    let mut alive = true;
    for _ in 0..50 {
        alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!alive, "the hook's child {pid} survived the timeout");
}

#[tokio::test]
async fn firing_returns_at_once_while_a_slow_hook_runs() {
    let f = install("exec sleep 60", 60).await;
    let started = Instant::now();
    fire(&f.ctx, "task.merged", merged());
    assert!(started.elapsed() < Duration::from_millis(200), "{:?}", started.elapsed());
}

#[tokio::test]
async fn five_failures_in_a_row_pause_the_plugin() {
    let f = install("exit 1", 10).await;
    for _ in 1..PAUSE_AFTER {
        run(&f.ctx, "task.merged", merged()).await;
        assert!(f.plugin().await.enabled);
    }
    run(&f.ctx, "task.merged", merged()).await;
    let paused = f.plugin().await;
    assert!(!paused.enabled);
    assert!(paused.error.unwrap().contains("paused"));
}

#[tokio::test]
async fn a_success_resets_the_count_of_failures() {
    let f = install("test -f ok", 10).await;
    for _ in 1..PAUSE_AFTER {
        run(&f.ctx, "task.merged", merged()).await;
    }
    std::fs::write(f.data.join("ok"), "").unwrap();
    run(&f.ctx, "task.merged", merged()).await;
    std::fs::remove_file(f.data.join("ok")).unwrap();
    for _ in 1..PAUSE_AFTER {
        run(&f.ctx, "task.merged", merged()).await;
    }
    assert!(f.plugin().await.enabled);
}

#[tokio::test]
async fn output_over_the_cap_is_a_failure() {
    let f = install("yes | head -c 300000", 10).await;
    for _ in 0..PAUSE_AFTER {
        run(&f.ctx, "task.merged", merged()).await;
    }
    assert!(!f.plugin().await.enabled);
}

#[tokio::test]
async fn a_disabled_plugin_or_an_unapproved_event_runs_nothing() {
    let hooks = hooks_file("task.merged", "touch ran", 10);
    let off = install_with(&hooks, json!(["hook:task.merged"]), false).await;
    run(&off.ctx, "task.merged", merged()).await;
    assert!(!off.has("ran"));
    let unapproved = install_with(&hooks, json!(["hook:task.ended"]), true).await;
    run(&unapproved.ctx, "task.merged", merged()).await;
    assert!(!unapproved.has("ran"));
    let on = install_with(&hooks, json!(["hook:task.merged"]), true).await;
    run(&on.ctx, "task.ended", merged()).await;
    assert!(!on.has("ran"));
    run(&on.ctx, "task.merged", merged()).await;
    assert!(on.has("ran"));
}

#[tokio::test]
async fn with_no_plugin_an_event_does_nothing_and_does_not_fail() {
    let dir = tempfile::TempDir::new().unwrap();
    let ws = Workspace::new(Home::at(dir.path().join("home")), Path::new(dir.path())).unwrap();
    let ctx = Context::open(ws, ContextOptions::default()).unwrap();
    run(&ctx, "task.merged", merged()).await;
}
