use std::path::{Path, PathBuf};

use serde::Serialize;

use super::agent_types::read_manifest;
use super::install::cache_root;
use super::market::blocking;
use crate::store::Store;
use crate::{Error, Result};

pub const MAX_UI_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiFile {
    pub path: PathBuf,
    pub mime: &'static str,
}

fn mime_of(rel: &str) -> Option<&'static str> {
    let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

fn plain_parts(rel: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = rel.split('/').collect();
    let plain = parts.iter().all(|p| {
        !p.is_empty()
            && *p != "."
            && *p != ".."
            && !p.contains('\\')
            && !p.chars().any(|c| c.is_control())
    });
    plain.then_some(parts)
}

fn listed(entries: &[&str], rel: &str) -> bool {
    entries.iter().any(|entry| {
        entry == &rel
            || entry
                .rsplit_once('/')
                .is_some_and(|(dir, _)| rel.starts_with(&format!("{dir}/")))
    })
}

fn resolve_in(root: &Path, entries: &[&str], rel: &str) -> Option<UiFile> {
    let parts = plain_parts(rel)?;
    if !listed(entries, rel) {
        return None;
    }
    let mime = mime_of(rel)?;
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
        if std::fs::symlink_metadata(&path).ok()?.is_symlink() {
            return None;
        }
    }
    let meta = std::fs::metadata(&path).ok()?;
    (meta.is_file() && meta.len() <= MAX_UI_FILE_BYTES).then_some(UiFile { path, mime })
}

pub async fn file(store: &Store, id: &str, rel: &str) -> Result<UiFile> {
    let plugin = store.plugin(id).await?;
    let missing = || Error::NotFound(format!("ui file `{rel}` of plugin `{id}`"));
    if !plugin.enabled {
        return Err(missing());
    }
    let root = cache_root(store.home(), &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    let rel = rel.to_string();
    let found = blocking(move || {
        let root = root
            .canonicalize()
            .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
        let manifest = read_manifest(&root)?;
        let entries: Vec<&str> = manifest
            .contributes
            .ui
            .iter()
            .map(|ui| ui.entry.as_str())
            .collect();
        Ok(resolve_in(&root, &entries, &rel))
    })
    .await?;
    found.ok_or_else(missing)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotEntry {
    pub plugin: String,
    pub slot: String,
    pub title: String,
    pub entry: String,
    pub height: Option<u32>,
}

pub async fn entries(store: &Store) -> Result<Vec<SlotEntry>> {
    let plugins: Vec<_> = store
        .plugins()
        .await?
        .into_iter()
        .filter(|p| p.enabled)
        .collect();
    let home = store.home().clone();
    blocking(move || {
        let mut all = Vec::new();
        for plugin in plugins {
            let root = cache_root(&home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
            let read = root
                .canonicalize()
                .map_err(|e| Error::io(format!("read {}", root.display()), e))
                .and_then(|root| read_manifest(&root));
            let manifest = match read {
                Ok(manifest) => manifest,
                Err(e) => {
                    skipped(&plugin.id, &e.to_string());
                    continue;
                }
            };
            all.extend(manifest.contributes.ui.into_iter().map(|ui| SlotEntry {
                plugin: plugin.id.clone(),
                slot: ui.slot,
                title: ui.title,
                entry: ui.entry,
                height: ui.height,
            }));
        }
        Ok(all)
    })
    .await
}

fn skipped(plugin: &str, reason: &str) {
    tracing::warn!(
        target: "townhall::plugins", event = "plugins.ui_skipped",
        plugin, reason = %crate::redact::redact(reason),
        "plugin ui entries skipped"
    );
}
