use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::agent_types;
use super::check;
use super::manifest::{self, McpTools, Permissions, MAX_MANIFEST_BYTES};
use super::marketplace::Entry;
use super::prompts;
use super::schema;
use super::sha256;
use super::skills::{self, Declared};
use super::theme;
use crate::{Error, Result};

pub const PLUGIN_API_VERSION: u32 = 1;

const TOWNHALL_MANIFEST: &str = ".townhall-plugin/plugin.json";
const CLAUDE_MANIFEST: &str = ".claude-plugin/plugin.json";
const DEFAULT_CLAUDE_HOOKS: &str = "hooks/hooks.json";
const DEFAULT_CLAUDE_MCP: &str = ".mcp.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub version: Option<String>,
    pub permissions: Vec<String>,
    pub hash: String,
    pub declared: Declared,
}

pub struct Limits {
    pub file: u64,
    pub total: u64,
}

pub const LIMITS: Limits = Limits {
    file: 1024 * 1024,
    total: 50 * 1024 * 1024,
};

pub fn validate(root: &Path, entry: &Entry) -> Result<Checked> {
    validate_with(root, entry, &LIMITS)
}

pub fn added(old: &[String], new: &[String]) -> Vec<String> {
    new.iter()
        .filter(|item| !old.contains(item))
        .cloned()
        .collect()
}

pub fn hash_of(permissions: &[String]) -> String {
    format!("sha256:{}", sha256::hex(permissions.join("\n").as_bytes()))
}

struct Files {
    townhall_hooks: Option<String>,
    claude_hooks: Option<String>,
    claude_mcp: Option<String>,
    others: Vec<(&'static str, String)>,
}

pub fn validate_with(root: &Path, entry: &Entry, limits: &Limits) -> Result<Checked> {
    let canon = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let mut total = 0;
    walk(&canon, &canon, &mut total, limits)?;

    let tree = Tree {
        root: canon,
        limits,
    };
    let mut declared = Declared {
        skills: entry.skills.clone(),
        commands: entry.commands.clone(),
    };
    let (version, mut permissions, files, derive) = match tree.optional(TOWNHALL_MANIFEST)? {
        Some(text) => {
            let found = manifest::parse_townhall(&text)?;
            api_version(found.api_version)?;
            let files = Files {
                townhall_hooks: found.contributes.hooks.clone(),
                claude_hooks: found.claude.hooks.clone(),
                claude_mcp: found.claude.mcp.clone(),
                others: others(&found.contributes),
            };
            (Some(found.version), found.permissions, files, false)
        }
        None => {
            let version = match tree.optional(CLAUDE_MANIFEST)? {
                Some(text) => {
                    let claude = manifest::parse_claude(&text)?;
                    declared.skills.extend(claude.skills);
                    declared.commands.extend(claude.commands);
                    claude.version
                }
                None => None,
            };
            let permissions = match &entry.townhall {
                Some(wrapped) => {
                    api_version(wrapped.api_version)?;
                    wrapped.permissions.clone()
                }
                None => Permissions::default(),
            };
            let files = Files {
                townhall_hooks: None,
                claude_hooks: None,
                claude_mcp: None,
                others: Vec::new(),
            };
            (version, permissions, files, true)
        }
    };
    let version = version.or_else(|| entry.version.clone());

    for (what, rel) in &files.others {
        let path = tree.file(what, rel)?;
        match *what {
            "contributes.agentTypes" => agent_types::refuse_skip_all_at_install(rel, &path)?,
            "contributes.theme" => {
                theme::parse(&tree.read(what, rel)?)?;
            }
            "contributes.settings" => {
                schema::parse(&tree.read(what, rel)?)?;
            }
            "contributes.prompts.starters" => {
                prompts::parse_starters(&tree.read(what, rel)?)?;
            }
            "contributes.prompts.brief" => {
                prompts::parse_markdown("brief", &tree.read(what, rel)?)?;
            }
            "contributes.prompts.planner" => {
                prompts::parse_markdown("planner", &tree.read(what, rel)?)?;
            }
            _ => {}
        }
    }

    let mut runs = BTreeSet::new();
    if let Some(rel) = &files.townhall_hooks {
        tree.hook_events("contributes.hooks", rel, &mut |event, commands| {
            if !permissions.hooks.contains(&event.to_string()) {
                return Err(check::invalid(
                    "contributes.hooks",
                    format!("event `{event}` needs to be listed under permissions.hooks"),
                ));
            }
            runs.extend(commands.iter().map(|c| format!("runs.hook:{event}:{c}")));
            Ok(())
        })?;
    }

    let claude_hooks = match &files.claude_hooks {
        Some(rel) => (rel.as_str(), true),
        None => (DEFAULT_CLAUDE_HOOKS, false),
    };
    {
        let (rel, required) = claude_hooks;
        if required || tree.exists(rel)? {
            let declared: Vec<String> = permissions
                .claude
                .hooks
                .as_ref()
                .map(|h| h.events.clone())
                .unwrap_or_default();
            let mut found = Vec::new();
            tree.hook_events("claude.hooks", rel, &mut |event, commands| {
                if !derive && !declared.contains(&event.to_string()) {
                    return Err(check::invalid(
                        "claude.hooks",
                        format!(
                            "event `{event}` needs to be listed under permissions.claude.hooks.events"
                        ),
                    ));
                }
                found.push(event.to_string());
                runs.extend(commands.iter().map(|c| format!("runs.hook:{event}:{c}")));
                Ok(())
            })?;
            if derive && !found.is_empty() {
                let hooks = permissions
                    .claude
                    .hooks
                    .get_or_insert_with(Default::default);
                for event in found {
                    if !hooks.events.contains(&event) {
                        hooks.events.push(event);
                    }
                }
            }
        }
    }

    let claude_mcp = match &files.claude_mcp {
        Some(rel) => (rel.as_str(), true),
        None => (DEFAULT_CLAUDE_MCP, false),
    };
    if claude_mcp.1 || tree.exists(claude_mcp.0)? {
        let servers = tree.mcp_servers("claude.mcp", claude_mcp.0)?;
        let declared = permissions.claude.mcp.get_or_insert_with(Default::default);
        for (name, how) in servers {
            if !declared.servers.contains_key(&name) {
                if !derive {
                    return Err(check::invalid(
                        "claude.mcp",
                        format!(
                            "server `{name}` needs to be listed under permissions.claude.mcp.servers"
                        ),
                    ));
                }
                declared
                    .servers
                    .insert(name.clone(), McpTools::List(vec!["*".to_string()]));
            }
            runs.insert(how.line(&name));
        }
    }

    declared.skills.dedup();
    declared.commands.dedup();
    let shipped = skills::discover(&tree.root, &declared)?;
    let mut list = items(&permissions);
    list.extend(runs);
    list.extend(shipped.skills.iter().map(|s| format!("claude.skill:{s}")));
    list.extend(shipped.commands.iter().map(|c| format!("claude.command:{c}")));
    list.sort();
    list.dedup();
    let hash = hash_of(&list);
    Ok(Checked {
        version,
        permissions: list,
        hash,
        declared,
    })
}

fn api_version(wanted: u32) -> Result<()> {
    if wanted > PLUGIN_API_VERSION {
        return Err(check::invalid(
            "apiVersion",
            format!("needs plugin api {wanted}, this townhall serves {PLUGIN_API_VERSION}"),
        ));
    }
    Ok(())
}

fn others(contributes: &manifest::Contributes) -> Vec<(&'static str, String)> {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    for path in &contributes.agent_types {
        found.push(("contributes.agentTypes", path.clone()));
    }
    for path in &contributes.pipelines {
        found.push(("contributes.pipelines", path.clone()));
    }
    for (kind, path) in &contributes.prompts {
        let what = match kind.as_str() {
            "starters" => "contributes.prompts.starters",
            "brief" => "contributes.prompts.brief",
            "planner" => "contributes.prompts.planner",
            _ => "contributes.prompts",
        };
        found.push((what, path.clone()));
    }
    if let Some(path) = &contributes.settings {
        found.push(("contributes.settings", path.clone()));
    }
    if let Some(path) = &contributes.theme {
        found.push(("contributes.theme", path.clone()));
    }
    for ui in &contributes.ui {
        found.push(("contributes.ui.entry", ui.entry.clone()));
    }
    found
}

fn items(permissions: &Permissions) -> Vec<String> {
    let mut list = Vec::new();
    let mut push = |prefix: &str, values: &[String]| {
        list.extend(values.iter().map(|v| format!("{prefix}:{v}")));
    };
    push("api", &permissions.api);
    push("hook", &permissions.hooks);
    push("prompt", &permissions.prompts);
    push("env", &permissions.env);
    push("ui", &permissions.ui);
    push("claude.api", &permissions.claude.api);
    if let Some(hooks) = &permissions.claude.hooks {
        push("claude.hooks", &hooks.events);
        push("claude.hooks.agent-type", &hooks.agent_types);
    }
    if let Some(mcp) = &permissions.claude.mcp {
        for (server, tools) in &mcp.servers {
            let (names, read_only) = match tools {
                McpTools::List(names) => (names, false),
                McpTools::Detailed(detail) => (&detail.tools, detail.read_only),
            };
            for tool in names {
                list.push(format!("claude.mcp:{server}:{tool}"));
                if read_only {
                    list.push(format!("claude.mcp.read-only:{server}:{tool}"));
                }
            }
        }
    }
    if permissions.claude.planner {
        list.push("claude.planner".to_string());
    }
    list
}

enum Launch {
    Command(String),
    Url(String),
}

impl Launch {
    fn line(&self, server: &str) -> String {
        match self {
            Launch::Command(text) => format!("runs.mcp:{server}:{text}"),
            Launch::Url(url) => format!("connects.mcp:{server}:{url}"),
        }
    }
}

struct Tree<'a> {
    root: PathBuf,
    limits: &'a Limits,
}

impl Tree<'_> {
    fn locate(&self, what: &str, rel: &str) -> Result<Option<PathBuf>> {
        check::rel_path(what, rel)?;
        let path = self.root.join(rel);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::io(format!("read {}", path.display()), e)),
        }
        let real = path
            .canonicalize()
            .map_err(|_| check::invalid(what, format!("`{rel}` points nowhere")))?;
        if !real.starts_with(&self.root) {
            return Err(check::invalid(
                what,
                format!("`{rel}` leads outside the plugin"),
            ));
        }
        Ok(Some(real))
    }

    fn exists(&self, rel: &str) -> Result<bool> {
        Ok(self.locate("plugin", rel)?.is_some())
    }

    fn file(&self, what: &str, rel: &str) -> Result<PathBuf> {
        let path = self
            .locate(what, rel)?
            .ok_or_else(|| check::invalid(what, format!("`{rel}` does not exist in the plugin")))?;
        let meta = std::fs::metadata(&path)
            .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
        if !meta.is_file() {
            return Err(check::invalid(what, format!("`{rel}` is not a file")));
        }
        if meta.len() > self.limits.file {
            return Err(Error::TooLarge(format!(
                "{what}: `{rel}` is {} bytes, the limit for one file is {}",
                meta.len(),
                self.limits.file
            )));
        }
        Ok(path)
    }

    fn read(&self, what: &str, rel: &str) -> Result<String> {
        let path = self.file(what, rel)?;
        std::fs::read_to_string(&path)
            .map_err(|e| check::invalid(what, format!("`{rel}` cannot be read as text: {e}")))
    }

    fn optional(&self, rel: &str) -> Result<Option<String>> {
        let Some(path) = self.locate("plugin manifest", rel)? else {
            return Ok(None);
        };
        let meta = std::fs::metadata(&path)
            .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
        if meta.len() > MAX_MANIFEST_BYTES as u64 {
            return Err(Error::TooLarge(format!(
                "{rel} is {} bytes, the limit is {MAX_MANIFEST_BYTES}",
                meta.len()
            )));
        }
        self.read("plugin manifest", rel).map(Some)
    }

    fn json(&self, what: &str, rel: &str) -> Result<Map<String, Value>> {
        let text = self.read(what, rel)?;
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(map),
            Ok(_) => Err(check::invalid(
                what,
                format!("`{rel}` must be a JSON object"),
            )),
            Err(e) => Err(check::invalid(
                what,
                format!("`{rel}` is not valid JSON: {e}"),
            )),
        }
    }

    fn hook_events(
        &self,
        what: &str,
        rel: &str,
        each: &mut dyn FnMut(&str, &[String]) -> Result<()>,
    ) -> Result<()> {
        let map = self.json(what, rel)?;
        let Some(Value::Object(events)) = map.get("hooks") else {
            return Err(check::invalid(
                what,
                format!("`{rel}` must look like {{\"hooks\": {{\"<event>\": [...]}}}}"),
            ));
        };
        for (event, body) in events {
            let mut commands = Vec::new();
            commands_in(body, &mut commands);
            each(event, &commands)?;
        }
        Ok(())
    }

    fn mcp_servers(&self, what: &str, rel: &str) -> Result<Vec<(String, Launch)>> {
        let map = self.json(what, rel)?;
        let servers = match map.get("mcpServers") {
            Some(Value::Object(servers)) => servers.clone(),
            Some(_) => {
                return Err(check::invalid(
                    what,
                    format!("`{rel}`: mcpServers must be an object"),
                ))
            }
            None => map,
        };
        let mut found = Vec::new();
        for (name, server) in servers {
            let launch = match (server.get("command"), server.get("url")) {
                (Some(Value::String(command)), _) => {
                    let args = server
                        .get("args")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                        .unwrap_or_default();
                    let mut line = command.clone();
                    for arg in args {
                        line.push(' ');
                        line.push_str(arg);
                    }
                    Launch::Command(line)
                }
                (_, Some(Value::String(url))) => Launch::Url(url.clone()),
                _ => {
                    return Err(check::invalid(
                        what,
                        format!("`{rel}`: server `{name}` has neither a command nor a url"),
                    ))
                }
            };
            found.push((name, launch));
        }
        Ok(found)
    }
}

fn commands_in(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Array(items) => items.iter().for_each(|v| commands_in(v, out)),
        Value::Object(map) => {
            if let Some(Value::String(command)) = map.get("command") {
                out.push(command.clone());
            }
            if let Some(inner) = map.get("hooks") {
                commands_in(inner, out);
            }
        }
        _ => {}
    }
}

fn walk(dir: &Path, root: &Path, total: &mut u64, limits: &Limits) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| Error::io(format!("read {}", dir.display()), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(format!("read {}", dir.display()), e))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let kind = entry
            .file_type()
            .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
        if kind.is_symlink() {
            let real = path
                .canonicalize()
                .map_err(|_| check::invalid("plugin", format!("symlink `{rel}` points nowhere")))?;
            if !real.starts_with(root) {
                return Err(check::invalid(
                    "plugin",
                    format!("symlink `{rel}` leads outside the plugin"),
                ));
            }
        } else if kind.is_dir() {
            walk(&path, root, total, limits)?;
        } else if kind.is_file() {
            let len = entry
                .metadata()
                .map_err(|e| Error::io(format!("read {}", path.display()), e))?
                .len();
            *total += len;
            if *total > limits.total {
                return Err(Error::TooLarge(format!(
                    "the plugin is over {} bytes",
                    limits.total
                )));
            }
        } else {
            return Err(check::invalid(
                "plugin",
                format!("`{rel}` is not a plain file, folder or symlink"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
