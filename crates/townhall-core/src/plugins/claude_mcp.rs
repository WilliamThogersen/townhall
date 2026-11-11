use std::path::Path;

use serde_json::{json, Value};

use super::agent_types::read_manifest;
use super::check;
use super::declared::read_inside;
use super::install::cache_root;
use super::manifest::{McpTools, TownhallManifest};
use super::market::blocking;
use crate::store::{Plugin, Store};
use crate::{Error, Home, Result};

const DEFAULT_MCP_FILE: &str = ".mcp.json";
const ROOT_VAR: &str = "${CLAUDE_PLUGIN_ROOT}";
const RESERVED_KEY: &str = "townhall";

#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    pub plugin: String,
    pub servers: Vec<(String, Value)>,
    pub tools: Vec<String>,
}

impl Contribution {
    pub fn with_token(&mut self, api_url: &str, token: &str) {
        for (_, server) in &mut self.servers {
            let Some(map) = server.as_object_mut() else {
                continue;
            };
            let env = map.entry("env").or_insert_with(|| json!({}));
            if let Some(env) = env.as_object_mut() {
                env.insert("TOWNHALL_API_URL".into(), json!(api_url));
                env.insert("TOWNHALL_TOKEN".into(), json!(token));
            }
        }
    }
}

pub(super) fn with_root(value: &mut Value, root: &str) {
    match value {
        Value::String(text) => *text = text.replace(ROOT_VAR, root),
        Value::Array(items) => items.iter_mut().for_each(|v| with_root(v, root)),
        Value::Object(map) => map.values_mut().for_each(|v| with_root(v, root)),
        _ => {}
    }
}

fn declared_env_only(server: &mut Value, declared: &[String]) {
    if let Some(env) = server.get_mut("env").and_then(Value::as_object_mut) {
        env.retain(|name, _| declared.contains(name));
    }
}

fn approved_tools(plugin: &str, server: &str, tools: &McpTools) -> Vec<String> {
    let key = format!("plugin_{plugin}_{server}");
    let names = match tools {
        McpTools::List(names) => names,
        McpTools::Detailed(detail) => &detail.tools,
    };
    if names.iter().any(|n| n == "*") {
        return vec![format!("mcp__{key}")];
    }
    names
        .iter()
        .map(|tool| format!("mcp__{key}__{tool}"))
        .collect()
}

pub fn contribution(
    plugin: &str,
    root: &Path,
    manifest: &TownhallManifest,
    mcp_json: &str,
) -> Result<Contribution> {
    let parsed: Value =
        serde_json::from_str(mcp_json).map_err(|e| check::invalid("claude.mcp", e))?;
    let found = parsed
        .get("mcpServers")
        .and_then(Value::as_object)
        .ok_or_else(|| check::invalid("claude.mcp", "has no `mcpServers` object"))?;
    let approved = manifest.permissions.claude.mcp.as_ref();
    let root = root.display().to_string();
    let mut out = Contribution {
        plugin: String::new(),
        servers: Vec::new(),
        tools: Vec::new(),
    };
    for (name, server) in found {
        if name.eq_ignore_ascii_case(RESERVED_KEY) {
            return Err(check::invalid(
                "claude.mcp",
                "the key `townhall` is reserved for townhall's own server",
            ));
        }
        let tools = approved
            .and_then(|mcp| mcp.servers.get(name))
            .ok_or_else(|| {
                check::invalid(
                    "claude.mcp",
                    format!("server `{name}` is not declared under permissions.claude.mcp.servers"),
                )
            })?;
        let mut server = server.clone();
        with_root(&mut server, &root);
        declared_env_only(&mut server, &manifest.permissions.env);
        out.servers
            .push((format!("plugin_{plugin}_{name}"), server));
        out.tools.extend(approved_tools(plugin, name, tools));
    }
    Ok(out)
}

pub fn merge(base: Option<&str>, parts: &[Contribution]) -> String {
    let mut servers = base
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|mut v| v.get_mut("mcpServers").map(Value::take))
        .and_then(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default();
    for (key, server) in parts.iter().flat_map(|p| &p.servers) {
        servers.insert(key.clone(), server.clone());
    }
    json!({ "mcpServers": Value::Object(servers) }).to_string()
}

pub(super) fn failed(plugin: &Plugin, why: impl std::fmt::Display) -> Error {
    Error::InvalidInput(format!("plugin {}: {why}", plugin.id))
}

fn load(home: &Home, plugin: &Plugin) -> Result<Option<Contribution>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    if !root.join(".townhall-plugin/plugin.json").is_file() {
        return Ok(None);
    }
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let manifest = read_manifest(&root)?;
    if manifest.permissions.claude.mcp.is_none() {
        return Ok(None);
    }
    let rel = manifest.claude.mcp.as_deref().unwrap_or(DEFAULT_MCP_FILE);
    let text = read_inside(&root, "claude.mcp", rel)?;
    let mut found = contribution(&plugin.name, &root, &manifest, &text)?;
    found.plugin = plugin.id.clone();
    Ok(Some(found))
}

pub(super) fn selects(selected: &[String], plugin: &Plugin) -> bool {
    selected
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&plugin.id) || s.eq_ignore_ascii_case(&plugin.name))
}

pub async fn selected(store: &Store, names: &[String]) -> Result<Vec<Contribution>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let plugins: Vec<Plugin> = store
        .plugins()
        .await?
        .into_iter()
        .filter(|p| p.enabled && selects(names, p))
        .collect();
    if plugins.is_empty() {
        return Ok(Vec::new());
    }
    let home = store.home().clone();
    blocking(move || {
        let mut all = Vec::new();
        for plugin in &plugins {
            if let Some(found) = load(&home, plugin).map_err(|e| failed(plugin, e))? {
                all.push(found);
            }
        }
        Ok(all)
    })
    .await
}

#[cfg(test)]
mod tests;
