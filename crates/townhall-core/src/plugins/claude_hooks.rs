use std::path::Path;

use serde_json::{json, Map, Value};

use super::agent_types::read_manifest;
use super::check;
use super::claude_mcp::{failed, selects, with_root};
use super::declared::read_inside;
use super::install::cache_root;
use super::manifest::TownhallManifest;
use super::market::blocking;
use crate::store::{Plugin, Store};
use crate::{Home, Result};

const DEFAULT_HOOKS_FILE: &str = "hooks/hooks.json";
const NOT_OFFERED: &str = "PreToolUse";

#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    pub events: Vec<(String, Vec<Value>)>,
}

pub fn contribution(
    root: &Path,
    manifest: &TownhallManifest,
    hooks_json: &str,
) -> Result<Contribution> {
    let parsed: Value =
        serde_json::from_str(hooks_json).map_err(|e| check::invalid("claude.hooks", e))?;
    let found = parsed
        .get("hooks")
        .and_then(Value::as_object)
        .ok_or_else(|| check::invalid("claude.hooks", "has no `hooks` object"))?;
    let declared: &[String] = manifest
        .permissions
        .claude
        .hooks
        .as_ref()
        .map(|h| h.events.as_slice())
        .unwrap_or_default();
    if declared.iter().any(|e| e == NOT_OFFERED) {
        return Err(check::invalid(
            "claude.hooks",
            format!("event `{NOT_OFFERED}` is not offered: it can approve or block tool calls"),
        ));
    }
    let root = root.display().to_string();
    let mut events = Vec::new();
    for (event, entries) in found {
        if !declared.contains(event) {
            return Err(check::invalid(
                "claude.hooks",
                format!("event `{event}` needs to be listed under permissions.claude.hooks.events"),
            ));
        }
        let mut entries = entries.clone();
        with_root(&mut entries, &root);
        match entries {
            Value::Array(list) if !list.is_empty() => events.push((event.clone(), list)),
            Value::Array(_) => {}
            _ => {
                return Err(check::invalid(
                    "claude.hooks",
                    format!("event `{event}` must be a list"),
                ))
            }
        }
    }
    Ok(Contribution { events })
}

pub fn merge(parts: &[Contribution]) -> Option<String> {
    let mut hooks = Map::new();
    for (event, entries) in parts.iter().flat_map(|p| &p.events) {
        let slot = hooks
            .entry(event.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(list) = slot {
            list.extend(entries.iter().cloned());
        }
    }
    (!hooks.is_empty())
        .then(|| json!({ "autoMemoryEnabled": false, "hooks": Value::Object(hooks) }).to_string())
}

fn load(home: &Home, plugin: &Plugin) -> Result<Option<Contribution>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    if !root.join(".townhall-plugin/plugin.json").is_file() {
        return Ok(None);
    }
    let root = root
        .canonicalize()
        .map_err(|e| crate::Error::io(format!("read {}", root.display()), e))?;
    let manifest = read_manifest(&root)?;
    if manifest.permissions.claude.hooks.is_none() {
        return Ok(None);
    }
    let rel = manifest.claude.hooks.as_deref().unwrap_or(DEFAULT_HOOKS_FILE);
    let text = read_inside(&root, "claude.hooks", rel)?;
    contribution(&root, &manifest, &text).map(Some)
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
