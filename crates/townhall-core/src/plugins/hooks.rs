use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::declared::read_inside;
use super::install::{cache_root, data_root};
use super::manifest::MAX_MANIFEST_BYTES;
use super::tokens;
use super::tree::PLUGIN_API_VERSION;
use crate::redact::redact;
use crate::store::Plugin;
use crate::{Context, Error, Result};

pub const PAUSE_AFTER: u32 = 5;
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const MAX_TIMEOUT_SECS: u64 = 120;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

pub const CHANGING_TIMEOUT_SECS: u64 = 10;
pub const MAX_AMEND_CHARS: usize = 8_000;
pub const MAX_AMEND_ENV: usize = 20;

const CHANGING: [&str; 5] = [
    "task.created",
    "task.before_start",
    "task.check",
    "pr.before_open",
    "message.before_turn",
];
const BLOCKING: [&str; 3] = ["task.created", "task.before_start", "pr.before_open"];
const FAIL_CLOSABLE: [&str; 2] = ["task.before_start", "pr.before_open"];
const PLUGINS_OFF: &str = "TOWNHALL_PLUGINS";
const TARGET_DEFAULT: [&str; 4] = ["PATH", "HOME", "LANG", "TZ"];
const NEVER_PASSED: [&str; 9] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GLAB_TOKEN",
    "GITLAB_TOKEN",
    "GITLAB_ACCESS_TOKEN",
    "GIT_ASKPASS",
    "SSH_AUTH_SOCK",
];
const REDACTED_FIELDS: [&str; 3] = ["question", "reason", "lastMessage"];
const KILL_GROUP: i32 = libc::SIGKILL;

#[derive(Debug, Default)]
pub struct State {
    failures: Mutex<HashMap<String, u32>>,
    tokens: Mutex<HashMap<String, String>>,
}

impl State {
    fn failed(&self, plugin: &str) -> u32 {
        let mut map = self.failures.lock().unwrap_or_else(|p| p.into_inner());
        let count = map.entry(plugin.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    fn succeeded(&self, plugin: &str) {
        let mut map = self.failures.lock().unwrap_or_else(|p| p.into_inner());
        map.remove(plugin);
    }

    fn token(&self, plugin: &str) -> Option<String> {
        let map = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        map.get(plugin).cloned()
    }

    fn remember_token(&self, plugin: &str, token: String) {
        let mut map = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        map.insert(plugin.to_string(), token);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Hook {
    command: String,
    timeout: Duration,
    fail_closed: bool,
}

struct Ran {
    code: i32,
    elapsed: Duration,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked {
    pub plugin: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Asked {
    pub blocked: Option<Blocked>,
    pub prompt: Vec<(String, String)>,
    pub env: BTreeMap<String, String>,
    pub checks: Vec<String>,
    pub context: Vec<(String, String)>,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Default)]
struct Budget {
    prompt_chars: usize,
    env: usize,
}

pub fn default_timeout(event: &str) -> Duration {
    match CHANGING.contains(&event) {
        true => Duration::from_secs(CHANGING_TIMEOUT_SECS),
        false => Duration::from_secs(DEFAULT_TIMEOUT_SECS),
    }
}

pub fn fire(ctx: &Context, event: &'static str, fields: Value) {
    let ctx = ctx.clone();
    tokio::spawn(async move { run(&ctx, event, fields).await });
}

pub async fn run(ctx: &Context, event: &str, fields: Value) {
    ask(ctx, event, fields).await;
}

pub async fn ask(ctx: &Context, event: &str, fields: Value) -> Asked {
    match run_event(ctx, event, fields).await {
        Ok(asked) => asked,
        Err(e) => {
            tracing::warn!(
                target: "townhall::plugin", event = "plugin.hooks_not_run", hook = event,
                error_kind = e.kind(), error = %redact(&e.to_string()),
                "plugin hooks could not be run"
            );
            Asked::default()
        }
    }
}

async fn run_event(ctx: &Context, event: &str, fields: Value) -> Result<Asked> {
    let mut asked = Asked::default();
    if std::env::var(PLUGINS_OFF).is_ok_and(|v| v.eq_ignore_ascii_case("off")) {
        return Ok(asked);
    }
    let wanted = format!("hook:{event}");
    let plugins: Vec<Plugin> = ctx
        .store()?
        .plugins()
        .await?
        .into_iter()
        .filter(|p| p.enabled && approved(p).contains(&wanted))
        .collect();
    for plugin in plugins {
        if let Err(e) = run_plugin(ctx, &plugin, event, &fields, &mut asked).await {
            tracing::warn!(
                target: "townhall::plugin", event = "plugin.hooks_not_run", plugin = %plugin.id,
                hook = event, error_kind = e.kind(), error = %redact(&e.to_string()),
                "plugin hooks could not be run"
            );
        }
        if asked.blocked.is_some() {
            break;
        }
    }
    Ok(asked)
}

fn approved(plugin: &Plugin) -> Vec<String> {
    serde_json::from_value(plugin.approved.clone()).unwrap_or_default()
}

async fn run_plugin(
    ctx: &Context,
    plugin: &Plugin,
    event: &str,
    fields: &Value,
    asked: &mut Asked,
) -> Result<()> {
    let home = ctx.home().clone();
    let (market, name, sha) = (
        plugin.marketplace.clone(),
        plugin.name.clone(),
        plugin.sha.clone(),
    );
    let root = cache_root(&home, &market, &name)?.join(&sha);
    let found = {
        let (root, event) = (root.clone(), event.to_string());
        tokio::task::spawn_blocking(move || hooks_of(&root, &event))
            .await
            .map_err(|e| Error::Unavailable(format!("reading hooks: {e}")))??
    };
    if found.is_empty() {
        return Ok(());
    }
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let data = data_root(&home, &market, &name)?;
    std::fs::create_dir_all(&data)
        .map_err(|e| Error::io(format!("create {}", data.display()), e))?;
    let token = token_for(ctx, &plugin.id).await?;
    let settings = ctx.store()?.plugin_settings(&plugin.id).await?;
    let declared: Vec<String> = approved(plugin)
        .iter()
        .filter_map(|a| a.strip_prefix("env:").map(str::to_string))
        .collect();
    let mut env = allowed_env(std::env::vars(), &declared);
    env.extend([
        ("TOWNHALL_EVENT".to_string(), event.to_string()),
        ("TOWNHALL_PLUGIN".to_string(), plugin.id.clone()),
        (
            "TOWNHALL_PLUGIN_ROOT".to_string(),
            root.display().to_string(),
        ),
        (
            "TOWNHALL_PLUGIN_DATA".to_string(),
            data.display().to_string(),
        ),
        ("TOWNHALL_API_URL".to_string(), ctx.base_url().to_string()),
        ("TOWNHALL_API_TOKEN".to_string(), token),
    ]);
    let mut budget = Budget::default();
    for hook in found {
        let command = hook
            .command
            .replace("${TOWNHALL_PLUGIN_ROOT}", &root.display().to_string())
            .replace("${TOWNHALL_PLUGIN_DATA}", &data.display().to_string());
        let payload = payload_of(plugin, event, &current_fields(fields, asked), &settings);
        let outcome = exec(&command, &data, &env, payload.to_string(), hook.timeout).await;
        let go = record(ctx, plugin, event, &outcome).await?;
        match outcome {
            Ok(ran) => apply(plugin, event, &ran, asked, &mut budget),
            Err(why) if hook.fail_closed && FAIL_CLOSABLE.contains(&event) => {
                asked.blocked = Some(Blocked {
                    plugin: plugin.id.clone(),
                    reason: format!("the hook failed and is marked failClosed: {}", redact(&why)),
                });
            }
            Err(_) => {}
        }
        if asked.blocked.is_some() || !go {
            break;
        }
    }
    Ok(())
}

fn current_fields(fields: &Value, asked: &Asked) -> Value {
    let mut fields = fields.clone();
    if let Value::Object(map) = &mut fields {
        for (key, value) in [("title", &asked.title), ("body", &asked.body)] {
            if let Some(value) = value {
                map.insert(key.into(), json!(value));
            }
        }
    }
    fields
}

fn ignored(plugin: &Plugin, event: &str, what: &str) {
    tracing::warn!(
        target: "townhall::plugin", event = "plugin.answer_ignored", plugin = %plugin.id,
        hook = event, what, "a hook's answer has a key this event does not allow"
    );
}

fn apply(plugin: &Plugin, event: &str, ran: &Ran, asked: &mut Asked, budget: &mut Budget) {
    let can_block = BLOCKING.contains(&event);
    if ran.code == 2 {
        match can_block {
            true => {
                let said = ran.stderr.trim();
                let reason = match said.is_empty() {
                    true => "blocked by a plugin hook".to_string(),
                    false => redact(said).into_owned(),
                };
                asked.blocked = Some(Blocked {
                    plugin: plugin.id.clone(),
                    reason,
                });
            }
            false => ignored(plugin, event, "exit 2"),
        }
        return;
    }
    let text = ran.stdout.trim();
    if text.is_empty() {
        return;
    }
    let Ok(Value::Object(answer)) = serde_json::from_str::<Value>(text) else {
        ignored(plugin, event, "stdout is not a JSON object");
        return;
    };
    if answer.get("decision").and_then(Value::as_str) == Some("block") {
        match can_block {
            true => {
                let reason = answer.get("reason").and_then(Value::as_str).unwrap_or("");
                let reason = match reason.trim().is_empty() {
                    true => "blocked by a plugin hook".to_string(),
                    false => redact(reason.trim()).into_owned(),
                };
                asked.blocked = Some(Blocked {
                    plugin: plugin.id.clone(),
                    reason,
                });
                return;
            }
            false => ignored(plugin, event, "decision"),
        }
    }
    if let Some(amend) = answer.get("amend").and_then(Value::as_object) {
        amend_of(plugin, event, amend, asked, budget);
    }
    match (answer.get("checks"), event) {
        (Some(Value::Array(list)), "task.check") => asked.checks.extend(
            list.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string),
        ),
        (Some(_), _) => ignored(plugin, event, "checks"),
        (None, _) => {}
    }
    match (answer.get("additionalContext"), event) {
        (Some(Value::String(text)), "message.before_turn") if !text.trim().is_empty() => {
            asked.context.push((plugin.id.clone(), text.trim().to_string()));
        }
        (Some(_), "message.before_turn") | (None, _) => {}
        (Some(_), _) => ignored(plugin, event, "additionalContext"),
    }
}

fn amend_of(
    plugin: &Plugin,
    event: &str,
    amend: &Map<String, Value>,
    asked: &mut Asked,
    budget: &mut Budget,
) {
    let pr = event == "pr.before_open";
    let brief = matches!(event, "task.created" | "task.before_start");
    for (key, value) in amend {
        match (key.as_str(), value) {
            ("prompt", Value::String(text)) if brief => {
                let room = MAX_AMEND_CHARS.saturating_sub(budget.prompt_chars);
                let kept: String = text.chars().take(room).collect();
                if kept.trim().is_empty() {
                    continue;
                }
                budget.prompt_chars += kept.chars().count();
                asked.prompt.push((plugin.id.clone(), kept));
            }
            ("env", Value::Object(vars)) if event == "task.before_start" => {
                for (name, value) in vars {
                    let Some(value) = value.as_str() else {
                        continue;
                    };
                    if name.is_empty() || name.contains('=') || budget.env >= MAX_AMEND_ENV {
                        continue;
                    }
                    budget.env += 1;
                    asked.env.insert(name.clone(), value.to_string());
                }
            }
            ("title", Value::String(text)) if pr => asked.title = Some(text.clone()),
            ("body", Value::String(text)) if pr => asked.body = Some(text.clone()),
            (other, _) => ignored(plugin, event, &format!("amend.{other}")),
        }
    }
}

async fn token_for(ctx: &Context, plugin: &str) -> Result<String> {
    let store = ctx.store()?;
    if let Some(token) = ctx.hooks().token(plugin) {
        if tokens::authenticate(store, &token).await?.is_some() {
            return Ok(token);
        }
    }
    let token = tokens::issue(store, plugin, None).await?;
    ctx.hooks().remember_token(plugin, token.clone());
    Ok(token)
}

fn payload_of(
    plugin: &Plugin,
    event: &str,
    fields: &Value,
    settings: &std::collections::BTreeMap<String, Value>,
) -> Value {
    let mut payload = Map::new();
    payload.insert("event".into(), json!(event));
    payload.insert("plugin".into(), json!(plugin.id));
    payload.insert("apiVersion".into(), json!(PLUGIN_API_VERSION));
    payload.insert("settings".into(), json!(settings));
    if let Value::Object(fields) = fields {
        for (key, value) in fields {
            let value = match value {
                Value::String(text) if REDACTED_FIELDS.contains(&key.as_str()) => {
                    Value::String(redact(text).into_owned())
                }
                other => other.clone(),
            };
            payload.insert(key.clone(), value);
        }
    }
    Value::Object(payload)
}

async fn record(
    ctx: &Context,
    plugin: &Plugin,
    event: &str,
    outcome: &std::result::Result<Ran, String>,
) -> Result<bool> {
    match outcome {
        Ok(ran) => {
            ctx.hooks().succeeded(&plugin.id);
            tracing::info!(
                target: "townhall::plugin", event = "plugin.hook_run", plugin = %plugin.id,
                hook = event, exit_code = ran.code,
                duration_ms = u64::try_from(ran.elapsed.as_millis()).unwrap_or(u64::MAX),
                "plugin hook ran"
            );
            Ok(true)
        }
        Err(why) => {
            let in_a_row = ctx.hooks().failed(&plugin.id);
            tracing::warn!(
                target: "townhall::plugin", event = "plugin.hook_failed", plugin = %plugin.id,
                hook = event, in_a_row, reason = %redact(why), "plugin hook failed"
            );
            if in_a_row < PAUSE_AFTER {
                return Ok(true);
            }
            pause(ctx, &plugin.id, why).await?;
            Ok(false)
        }
    }
}

async fn pause(ctx: &Context, id: &str, why: &str) -> Result<()> {
    ctx.hooks().succeeded(id);
    let store = ctx.store()?;
    let saved = store.plugin(id).await?;
    if !saved.enabled {
        return Ok(());
    }
    let error = format!(
        "paused after {PAUSE_AFTER} failed hooks in a row, the last: {}",
        redact(why)
    );
    store
        .update_plugin(Plugin {
            enabled: false,
            error: Some(error),
            ..saved
        })
        .await?;
    tracing::warn!(
        target: "townhall::plugin", event = "plugin.hooks_paused", plugin = %id,
        "plugin paused after repeated hook failures"
    );
    Ok(())
}

fn hooks_of(root: &Path, event: &str) -> Result<Vec<Hook>> {
    let manifest = root.join(".townhall-plugin/plugin.json");
    let text = match std::fs::metadata(&manifest) {
        Ok(meta) if meta.len() > MAX_MANIFEST_BYTES as u64 => {
            return Err(Error::TooLarge(format!(
                "{} is too large",
                manifest.display()
            )))
        }
        Ok(_) => std::fs::read_to_string(&manifest)
            .map_err(|e| Error::io(format!("read {}", manifest.display()), e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(format!("read {}", manifest.display()), e)),
    };
    let found = super::manifest::parse_townhall(&text)?;
    let Some(rel) = found.contributes.hooks else {
        return Ok(Vec::new());
    };
    let canon = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let file = read_inside(&canon, "contributes.hooks", &rel)?;
    let parsed: Value = serde_json::from_str(&file)
        .map_err(|e| Error::InvalidInput(format!("contributes.hooks `{rel}`: {e}")))?;
    let mut hooks = Vec::new();
    if let Some(entries) = parsed.get("hooks").and_then(|h| h.get(event)) {
        collect(event, entries, &mut hooks);
    }
    Ok(hooks)
}

fn collect(event: &str, value: &Value, out: &mut Vec<Hook>) {
    match value {
        Value::Array(items) => items.iter().for_each(|v| collect(event, v, out)),
        Value::Object(map) => {
            if let Some(Value::String(command)) = map.get("command") {
                let secs = map
                    .get("timeout")
                    .and_then(Value::as_u64)
                    .unwrap_or(default_timeout(event).as_secs())
                    .clamp(1, MAX_TIMEOUT_SECS);
                out.push(Hook {
                    command: command.clone(),
                    timeout: Duration::from_secs(secs),
                    fail_closed: map.get("failClosed").and_then(Value::as_bool) == Some(true),
                });
            }
            if let Some(inner) = map.get("hooks") {
                collect(event, inner, out);
            }
        }
        _ => {}
    }
}

fn allowed_env(
    daemon: impl IntoIterator<Item = (String, String)>,
    declared: &[String],
) -> Vec<(String, String)> {
    let blocked = |name: &str| name.starts_with("ANTHROPIC_") || NEVER_PASSED.contains(&name);
    daemon
        .into_iter()
        .filter(|(name, _)| {
            let base = TARGET_DEFAULT.contains(&name.as_str()) || name.starts_with("LC_");
            let asked = declared.iter().any(|d| d == name);
            (base || asked) && !blocked(name)
        })
        .collect()
}

fn kill_group(pid: Option<u32>) {
    if let Some(pid) = pid.and_then(|p| i32::try_from(p).ok()) {
        unsafe {
            libc::kill(-pid, KILL_GROUP);
        }
    }
}

struct Drained {
    bytes: Vec<u8>,
    over: bool,
}

async fn drain(mut reader: impl AsyncRead + Unpin) -> Drained {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => return Drained { bytes, over: false },
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() > MAX_OUTPUT_BYTES {
                    return Drained { bytes, over: true };
                }
            }
        }
    }
}

async fn feed(mut stdin: tokio::process::ChildStdin, payload: String) {
    match stdin.write_all(payload.as_bytes()).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => tracing::warn!(
            target: "townhall::plugin", event = "plugin.hook_stdin_failed",
            error = %redact(&e.to_string()), "a hook's payload could not be written"
        ),
    }
}

async fn exec(
    command: &str,
    cwd: &Path,
    env: &[(String, String)],
    payload: String,
    limit: Duration,
) -> std::result::Result<Ran, String> {
    let mut sh = Command::new("sh");
    sh.args(["-c", command])
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    let started = Instant::now();
    let mut child = sh.spawn().map_err(|e| format!("cannot start sh: {e}"))?;
    let pid = child.id();
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        kill_group(pid);
        return Err("the hook's pipes could not be opened".to_string());
    };
    let work = async {
        let child = &mut child;
        let (_, out, err) = tokio::join!(feed(stdin, payload), drain(stdout), drain(stderr));
        if out.over || err.over {
            kill_group(pid);
        }
        let status = child.wait().await;
        (status, out, err)
    };
    let Ok((status, out, err)) = tokio::time::timeout(limit, work).await else {
        kill_group(pid);
        return Err(format!(
            "timed out after {}s and was killed",
            limit.as_secs()
        ));
    };
    if out.over || err.over {
        return Err(format!(
            "wrote more than {MAX_OUTPUT_BYTES} bytes of output"
        ));
    }
    let status = status.map_err(|e| format!("cannot wait for the hook: {e}"))?;
    match status.code() {
        Some(code @ (0 | 2)) => Ok(Ran {
            code,
            elapsed: started.elapsed(),
            stdout: String::from_utf8_lossy(&out.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&err.bytes).into_owned(),
        }),
        Some(code) => {
            let said = String::from_utf8_lossy(&err.bytes);
            let first = said.lines().next().unwrap_or("").trim();
            Err(format!("exited with {code}: {first}"))
        }
        None => Err("was ended by a signal".to_string()),
    }
}

#[cfg(test)]
pub(crate) mod tests;
