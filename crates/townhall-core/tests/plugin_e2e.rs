use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use townhall_core::model::NewTask;
use townhall_core::plugins::install::{InstallPlugin, PatchPlugin, PluginRegistry};
use townhall_core::plugins::market::{AddMarketplace, MarketplaceRegistry};
use townhall_core::plugins::{tokens, ui};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{commit, git, unique, Sched};

const MARKET: &str = "sample-market";
const ID: &str = "notes@sample-market";
const TYPE: &str = "notes-note-taker";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/sample-marketplace")
}

fn plugin_dir() -> PathBuf {
    fixture().join("plugins/notes")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

async fn market_repo(s: &Sched) -> PathBuf {
    let dir = s.root.path().join(unique("market"));
    copy_tree(&fixture(), &dir);
    git(&dir, &["init", "-q", "-b", "main"]).await;
    git(&dir, &["add", "-A"]).await;
    commit(&dir, "sample marketplace").await;
    dir
}

async fn install_from_market(s: &Sched) {
    let repo = market_repo(s).await;
    let markets = MarketplaceRegistry::new(s.store().clone());
    let added = AddMarketplace {
        source: repo.to_string_lossy().into_owned(),
        name: None,
        account: None,
    };
    let view = markets.add(&added).await.unwrap();
    assert_eq!(view.name, MARKET);
    assert_eq!(view.plugins.len(), 1);

    let plugins = PluginRegistry::new(s.store().clone());
    let preview = plugins.preview(ID).await.unwrap();
    for wanted in [
        "api:tasks:read",
        "api:storage",
        "hook:task.started",
        "ui:session.section",
    ] {
        assert!(preview.permissions.iter().any(|p| p == wanted), "{wanted}");
    }
    let request = InstallPlugin {
        plugin: ID.into(),
        approve: preview.permissions_hash,
    };
    let installed = plugins.install(&request).await.unwrap();
    assert!(!installed.enabled);
    let on = PatchPlugin {
        enabled: Some(true),
        pinned: None,
    };
    assert!(plugins.patch(ID, &on).await.unwrap().enabled);
}

async fn create_note_taker(s: &Sched, name: &str) {
    let new = NewTask {
        agent: TYPE.into(),
        ..s.new_task(name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

async fn wait_for(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{} was never written", path.display());
}

fn arg_value(args: &str, flag: &str) -> String {
    let prefix = format!("{flag}=");
    let found = args.lines().find_map(|line| line.strip_prefix(&prefix));
    found
        .unwrap_or_else(|| panic!("no {flag} in:\n{args}"))
        .to_string()
}

#[tokio::test]
async fn the_sample_plugin_installs_from_a_git_marketplace_and_its_agent_type_appears() {
    let s = Sched::new().await;
    install_from_market(&s).await;
    let types = s.store().agent_types().await.unwrap();
    let found = types.iter().find(|t| t.def.name == TYPE).unwrap();
    assert_eq!(found.plugin.as_deref(), Some(ID));
}

#[tokio::test]
async fn a_task_of_the_plugins_agent_type_gets_the_mcp_config_and_a_task_token() {
    let s = Sched::new().await;
    install_from_market(&s).await;
    let name = unique("noted");
    create_note_taker(&s, &name).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");

    let args = s.args(&name).await;
    assert!(
        args.starts_with("--model=haiku\n--permission-mode=plan\n"),
        "{args}"
    );
    assert!(args.contains("--strict-mcp-config"), "{args}");
    assert!(
        args.lines()
            .any(|l| l.contains("mcp__plugin_notes_notes__search_notes")),
        "{args}"
    );

    let config_path = arg_value(&args, "--mcp-config");
    assert!(
        config_path.ends_with(&format!("tasks/{name}/mcp.json")),
        "{config_path}"
    );
    let config: Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    let server = &config["mcpServers"]["plugin_notes_notes"];
    assert_eq!(server["command"], "node");
    let script = server["args"][0].as_str().unwrap();
    assert!(script.ends_with("mcp/server.mjs"), "{script}");
    assert!(Path::new(script).is_file(), "{script}");
    assert_eq!(server["env"]["TOWNHALL_API_URL"], s.ctx().base_url());

    let token = server["env"]["TOWNHALL_TOKEN"].as_str().unwrap();
    assert!(!args.contains(token), "the token is on the command line");
    let caller = tokens::authenticate(s.store(), token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(caller.plugin, ID);
    assert_eq!(caller.task.as_deref(), Some(name.as_str()));
    let mut scopes = caller.scopes.clone();
    scopes.sort();
    assert_eq!(scopes, ["storage", "tasks:read"]);
}

#[tokio::test]
async fn the_hook_fires_when_a_task_of_the_plugin_starts() {
    let s = Sched::new().await;
    install_from_market(&s).await;
    let name = unique("hooked");
    create_note_taker(&s, &name).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.failed.is_empty(), "{pass:?}");

    let data =
        s.ws.home()
            .plugins_dir()
            .unwrap()
            .join("data")
            .join(MARKET)
            .join("notes");
    let payload: Value = serde_json::from_str(&wait_for(&data.join("started.json")).await).unwrap();
    assert_eq!(payload["event"], "task.started");
    assert_eq!(payload["plugin"], ID);
    assert_eq!(payload["task"]["name"], name.as_str());
}

#[tokio::test]
async fn a_disabled_plugin_lists_and_serves_no_ui() {
    let s = Sched::new().await;
    install_from_market(&s).await;
    let off = PatchPlugin {
        enabled: Some(false),
        pinned: None,
    };
    PluginRegistry::new(s.store().clone())
        .patch(ID, &off)
        .await
        .unwrap();
    assert!(ui::entries(s.store()).await.unwrap().is_empty());
    assert!(ui::file(s.store(), ID, "ui/session.html").await.is_err());
}

#[tokio::test]
async fn the_ui_section_is_listed_and_served_with_its_sdk_and_nothing_else() {
    let s = Sched::new().await;
    install_from_market(&s).await;
    let entries = ui::entries(s.store()).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].plugin, ID);
    assert_eq!(entries[0].slot, "session.section");
    assert_eq!(entries[0].entry, "ui/session.html");

    let page = ui::file(s.store(), ID, "ui/session.html").await.unwrap();
    let html = std::fs::read_to_string(&page.path).unwrap();
    assert!(html.contains("townhall-plugin-sdk.js"), "{html}");
    for served in ["ui/session.js", "ui/townhall-plugin-sdk.js"] {
        ui::file(s.store(), ID, served).await.unwrap();
    }
    for hidden in [
        ".townhall-plugin/plugin.json",
        "townhall-hooks.json",
        "mcp/server.mjs",
        "bin/on-started.sh",
    ] {
        assert!(ui::file(s.store(), ID, hidden).await.is_err(), "{hidden}");
    }
}

#[test]
fn the_fixtures_sdk_is_the_one_the_web_app_tests() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/src/plugins/sdk/townhall-plugin-sdk.js");
    let shipped = plugin_dir().join("ui/townhall-plugin-sdk.js");
    assert_eq!(
        std::fs::read_to_string(shipped).unwrap(),
        std::fs::read_to_string(web).unwrap()
    );
}

#[test]
fn the_mcp_server_lists_and_runs_its_tool_over_stdio() {
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "search_notes", "arguments": {"query": "changelog"}}}),
    ];
    let mut child = Command::new("node")
        .arg(plugin_dir().join("mcp/server.mjs"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in &requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let replies: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "notes");
    assert_eq!(replies[1]["result"]["tools"][0]["name"], "search_notes");
    assert_eq!(
        replies[2]["result"]["content"][0]["text"],
        "write the changelog line"
    );
}
