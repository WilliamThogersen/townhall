use std::path::Path;

use super::check;
use super::install::cache_root;
use super::manifest::{self, MAX_MANIFEST_BYTES};
use super::market::blocking;
use crate::definitions::{validate_agent, DefinitionError, PermissionMode};
use crate::model::{
    AgentType, Origin, ADVISER_AGENT_TYPE, CONVERSATION_AGENT_TYPE, DEFAULT_AGENT_TYPE,
};
use crate::store::{Plugin, Store};
use crate::{clock, Error, Home, Result};

const RESERVED: [&str; 6] = [
    CONVERSATION_AGENT_TYPE,
    DEFAULT_AGENT_TYPE,
    ADVISER_AGENT_TYPE,
    "reflector",
    "tester",
    "reviewer",
];

pub(super) fn joined(errors: &[DefinitionError]) -> String {
    errors
        .iter()
        .map(|e| {
            let text = e.to_string();
            text.strip_prefix("error: ").unwrap_or(&text).to_string()
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn skipped(plugin: &str, file: &str, reason: &str) {
    tracing::warn!(
        target: "townhall::plugins", event = "plugins.agent_type_skipped",
        plugin, file, reason = %crate::redact::redact(reason),
        "plugin agent type skipped"
    );
}

pub(super) fn read_manifest(root: &Path) -> Result<manifest::TownhallManifest> {
    let path = root.join(".townhall-plugin/plugin.json");
    let meta =
        std::fs::metadata(&path).map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    if meta.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(Error::TooLarge(format!("{} is too large", path.display())));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    manifest::parse_townhall(&text)
}

fn contributed_by(home: &Home, plugin: &Plugin) -> Result<Vec<AgentType>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    if !root.join(".townhall-plugin/plugin.json").is_file() {
        return Ok(Vec::new());
    }
    let canon = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let found = read_manifest(&canon)?;
    let mut types = Vec::new();
    for rel in &found.contributes.agent_types {
        match one(&canon, plugin, rel) {
            Ok(agent) => types.push(agent),
            Err(e) => skipped(&plugin.id, rel, &e.to_string()),
        }
    }
    Ok(types)
}

fn one(root: &Path, plugin: &Plugin, rel: &str) -> Result<AgentType> {
    check::rel_path("contributes.agentTypes", rel)?;
    let path = root
        .join(rel)
        .canonicalize()
        .map_err(|e| Error::io(format!("read {rel}"), e))?;
    if !path.starts_with(root) {
        return Err(check::invalid(
            "contributes.agentTypes",
            format!("`{rel}` leads outside the plugin"),
        ));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    let parsed =
        Store::check_agent_type(&text).map_err(|errors| Error::InvalidInput(joined(&errors)))?;
    let mut def = parsed.def;
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&def.name)) {
        return Err(Error::InvalidInput(format!(
            "`{}` is a name townhall keeps for its own types",
            def.name
        )));
    }
    refuse_bypass(def.permission_mode)?;
    def.name = format!("{}-{}", plugin.name, def.name);
    validate_agent(&def).map_err(|errors| Error::InvalidInput(joined(&errors)))?;
    Ok(AgentType {
        def,
        source: parsed.source,
        updated_at: clock::now_rfc3339(),
        origin: Origin::Plugin,
        plugin: Some(plugin.id.clone()),
    })
}

pub(super) fn refuse_skip_all_at_install(rel: &str, path: &Path) -> Result<()> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    match Store::check_agent_type(&text) {
        Ok(parsed) => refuse_bypass(parsed.def.permission_mode)
            .map_err(|e| check::invalid("contributes.agentTypes", format!("`{rel}`: {e}"))),
        Err(_) => Ok(()),
    }
}

fn refuse_bypass(mode: Option<PermissionMode>) -> Result<()> {
    match mode {
        Some(PermissionMode::BypassPermissions) => Err(Error::InvalidInput(
            "a plugin type may not set permission_mode = \"bypassPermissions\": copy the type and edit it".into(),
        )),
        _ => Ok(()),
    }
}

pub(crate) async fn enabled_types(store: &Store) -> Result<Vec<AgentType>> {
    let plugins: Vec<Plugin> = store
        .plugins()
        .await?
        .into_iter()
        .filter(|p| p.enabled)
        .collect();
    if plugins.is_empty() {
        return Ok(Vec::new());
    }
    let home = store.home().clone();
    blocking(move || {
        let mut all = Vec::new();
        for plugin in &plugins {
            match contributed_by(&home, plugin) {
                Ok(types) => all.extend(types),
                Err(e) => skipped(&plugin.id, "", &e.to_string()),
            }
        }
        Ok(all)
    })
    .await
}

pub(crate) async fn owner_of(store: &Store, type_name: &str) -> Result<Option<Plugin>> {
    let lower = type_name.to_ascii_lowercase();
    Ok(store
        .plugins()
        .await?
        .into_iter()
        .filter(|p| lower.starts_with(&format!("{}-", p.name.to_ascii_lowercase())))
        .max_by_key(|p| p.name.len()))
}
