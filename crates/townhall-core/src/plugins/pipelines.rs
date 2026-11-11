use std::path::Path;

use super::agent_types::{joined, read_manifest};
use super::check;
use super::install::cache_root;
use super::market::blocking;
use crate::definitions::{check_pipeline, KnownAgent};
use crate::model::{Origin, Workflow};
use crate::store::{Plugin, Store};
use crate::{clock, Error, Home, Result};

struct Found {
    plugin: Plugin,
    text: String,
}

fn skipped(plugin: &str, file: &str, reason: &str) {
    tracing::warn!(
        target: "townhall::plugins", event = "plugins.pipeline_skipped",
        plugin, file, reason = %crate::redact::redact(reason),
        "plugin pipeline skipped"
    );
}

fn contributed_by(home: &Home, plugin: &Plugin) -> Result<Vec<Found>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    if !root.join(".townhall-plugin/plugin.json").is_file() {
        return Ok(Vec::new());
    }
    let canon = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let manifest = read_manifest(&canon)?;
    let mut found = Vec::new();
    for rel in &manifest.contributes.pipelines {
        match read(&canon, rel) {
            Ok(text) => found.push(Found {
                plugin: plugin.clone(),
                text,
            }),
            Err(e) => skipped(&plugin.id, rel, &e.to_string()),
        }
    }
    Ok(found)
}

fn read(root: &Path, rel: &str) -> Result<String> {
    check::rel_path("contributes.pipelines", rel)?;
    let path = root
        .join(rel)
        .canonicalize()
        .map_err(|e| Error::io(format!("read {rel}"), e))?;
    if !path.starts_with(root) {
        return Err(check::invalid(
            "contributes.pipelines",
            format!("`{rel}` leads outside the plugin"),
        ));
    }
    std::fs::read_to_string(&path).map_err(|e| Error::io(format!("read {}", path.display()), e))
}

fn build(found: Found, agents: &[KnownAgent]) -> Result<Workflow> {
    let parsed = check_pipeline(&found.text, agents)
        .map_err(|errors| Error::InvalidInput(joined(&errors)))?;
    let mut def = parsed.def;
    if def.default {
        return Err(Error::InvalidInput(
            "a plugin pipeline may not set default = true: copy the pipeline and edit it".into(),
        ));
    }
    def.name = format!("{}-{}", found.plugin.name, def.name);
    Ok(Workflow {
        def,
        source: parsed.source,
        updated_at: clock::now_rfc3339(),
        origin: Origin::Plugin,
        plugin: Some(found.plugin.id),
    })
}

pub(crate) async fn enabled_pipelines(store: &Store) -> Result<Vec<Workflow>> {
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
    let files = blocking(move || {
        let mut all = Vec::new();
        for plugin in &plugins {
            match contributed_by(&home, plugin) {
                Ok(found) => all.extend(found),
                Err(e) => skipped(&plugin.id, "", &e.to_string()),
            }
        }
        Ok(all)
    })
    .await?;
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let types = store.agent_types().await?;
    let agents: Vec<KnownAgent> = types
        .iter()
        .map(|t| KnownAgent::new(&t.def.name, &t.def.requires))
        .collect();
    let mut all = Vec::new();
    for found in files {
        let id = found.plugin.id.clone();
        match build(found, &agents) {
            Ok(pipeline) => all.push(pipeline),
            Err(e) => skipped(&id, "", &e.to_string()),
        }
    }
    Ok(all)
}
