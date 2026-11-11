use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::agent_types::read_manifest;
use super::check;
use super::install::cache_root;
use super::market::blocking;
use super::prompts::{self, Starter};
use super::schema::{self, Field};
use super::theme;
use super::tree::LIMITS;
use crate::store::{Plugin, Store};
use crate::{Error, Home, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Declared {
    pub id: String,
    pub name: String,
    pub starters: Vec<Starter>,
    pub brief: Option<String>,
    pub planner: Option<String>,
    pub fields: Vec<Field>,
    pub theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterView {
    pub plugin: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeChoice {
    pub plugin: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTheme {
    pub plugin: String,
    pub css: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    #[serde(flatten)]
    pub field: Field,
    pub value: Value,
    pub set: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGroup {
    pub plugin: String,
    pub name: String,
    pub fields: Vec<FieldView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributions {
    pub starters: Vec<StarterView>,
    pub themes: Vec<ThemeChoice>,
    pub theme: Option<ActiveTheme>,
    pub settings: Vec<SettingsGroup>,
}

fn skipped(plugin: &str, what: &str, reason: &str) {
    tracing::warn!(
        target: "townhall::plugins", event = "plugins.contribution_skipped",
        plugin, what, reason = %crate::redact::redact(reason),
        "plugin contribution skipped"
    );
}

fn read_inside(root: &Path, what: &str, rel: &str) -> Result<String> {
    check::rel_path(what, rel)?;
    let path = root
        .join(rel)
        .canonicalize()
        .map_err(|e| Error::io(format!("read {rel}"), e))?;
    if !path.starts_with(root) {
        return Err(check::invalid(
            what,
            format!("`{rel}` leads outside the plugin"),
        ));
    }
    let meta =
        std::fs::metadata(&path).map_err(|e| Error::io(format!("read {}", path.display()), e))?;
    if !meta.is_file() {
        return Err(check::invalid(what, format!("`{rel}` is not a file")));
    }
    if meta.len() > LIMITS.file {
        return Err(Error::TooLarge(format!(
            "{what}: `{rel}` is {} bytes, the limit for one file is {}",
            meta.len(),
            LIMITS.file
        )));
    }
    std::fs::read_to_string(&path).map_err(|e| Error::io(format!("read {}", path.display()), e))
}

fn contribution<T>(
    plugin: &Plugin,
    root: &Path,
    what: &str,
    rel: Option<&String>,
    parse: impl Fn(&str) -> Result<T>,
) -> Option<T> {
    let rel = rel?;
    match read_inside(root, what, rel).and_then(|text| parse(&text)) {
        Ok(found) => Some(found),
        Err(e) => {
            skipped(&plugin.id, what, &e.to_string());
            None
        }
    }
}

fn load(home: &Home, plugin: &Plugin) -> Result<Option<Declared>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    if !root.join(".townhall-plugin/plugin.json").is_file() {
        return Ok(None);
    }
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let found = read_manifest(&root)?;
    let c = &found.contributes;
    let starters = contribution(
        plugin,
        &root,
        "contributes.prompts.starters",
        c.prompts.get("starters"),
        prompts::parse_starters,
    );
    let brief = contribution(
        plugin,
        &root,
        "contributes.prompts.brief",
        c.prompts.get("brief"),
        |text| prompts::parse_markdown("brief", text),
    );
    let planner = contribution(
        plugin,
        &root,
        "contributes.prompts.planner",
        c.prompts.get("planner"),
        |text| prompts::parse_markdown("planner", text),
    );
    let fields = contribution(
        plugin,
        &root,
        "contributes.settings",
        c.settings.as_ref(),
        schema::parse,
    );
    let theme = contribution(
        plugin,
        &root,
        "contributes.theme",
        c.theme.as_ref(),
        theme::parse,
    );
    Ok(Some(Declared {
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        starters: starters.unwrap_or_default(),
        brief,
        planner,
        fields: fields.unwrap_or_default(),
        theme,
    }))
}

pub async fn enabled(store: &Store) -> Result<Vec<Declared>> {
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
            match load(&home, plugin) {
                Ok(Some(found)) => all.push(found),
                Ok(None) => {}
                Err(e) => skipped(&plugin.id, "", &e.to_string()),
            }
        }
        Ok(all)
    })
    .await
}

fn selects(selected: &[String], plugin: &Declared) -> bool {
    selected
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&plugin.id) || s.eq_ignore_ascii_case(&plugin.name))
}

pub async fn brief_sections(store: &Store, selected: &[String]) -> Result<Vec<(String, String)>> {
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    Ok(enabled(store)
        .await?
        .into_iter()
        .filter(|p| selects(selected, p))
        .filter_map(|p| p.brief.map(|text| (p.id, text)))
        .collect())
}

pub async fn planner_notes(store: &Store) -> Result<Vec<(String, String)>> {
    Ok(enabled(store)
        .await?
        .into_iter()
        .filter_map(|p| p.planner.map(|text| (p.id, text)))
        .collect())
}

fn view_of(field: &Field, stored: Option<&Value>) -> FieldView {
    let kept = stored.and_then(|value| field.check(value).ok());
    let set = kept.is_some();
    let value = match (field.secret, kept) {
        (true, _) => Value::Null,
        (false, Some(kept)) => kept,
        (false, None) => field.default.clone(),
    };
    FieldView {
        field: field.clone(),
        value,
        set,
    }
}

pub async fn choose_theme(store: &Store, plugin: Option<&str>) -> Result<Contributions> {
    match plugin {
        None => store.set_plugin_theme(None).await?,
        Some(id) => {
            let row = store.plugin(id).await?;
            let ships_one = row.enabled
                && enabled(store)
                    .await?
                    .iter()
                    .any(|p| p.id == row.id && p.theme.is_some());
            if !ships_one {
                return Err(Error::Conflict(format!(
                    "plugin `{}` has no theme to apply: it must be enabled and ship a valid theme",
                    row.id
                )));
            }
            store.set_plugin_theme(Some(&row.id)).await?;
        }
    }
    contributions(store).await
}

async fn field_of(store: &Store, id: &str, key: &str) -> Result<(String, Field)> {
    let all = enabled(store).await?;
    let plugin = all
        .into_iter()
        .find(|p| p.id.eq_ignore_ascii_case(id) && !p.fields.is_empty())
        .ok_or_else(|| Error::missing("plugin settings", id))?;
    let field = plugin
        .fields
        .into_iter()
        .find(|f| f.key == key)
        .ok_or_else(|| Error::missing("setting", format!("{}/{key}", plugin.id)))?;
    Ok((plugin.id, field))
}

pub async fn set_setting(
    store: &Store,
    id: &str,
    key: &str,
    value: Value,
) -> Result<Contributions> {
    let (id, field) = field_of(store, id, key).await?;
    let value = field.check(&value)?;
    store.set_plugin_setting(&id, key, value).await?;
    contributions(store).await
}

pub async fn reset_setting(store: &Store, id: &str, key: &str) -> Result<Contributions> {
    let (id, _) = field_of(store, id, key).await?;
    store.reset_plugin_setting(&id, key).await?;
    contributions(store).await
}

pub async fn contributions(store: &Store) -> Result<Contributions> {
    let all = enabled(store).await?;
    let chosen = store.plugin_theme().await?;
    let mut out = Contributions {
        starters: Vec::new(),
        themes: Vec::new(),
        theme: None,
        settings: Vec::new(),
    };
    for plugin in &all {
        out.starters
            .extend(plugin.starters.iter().map(|s| StarterView {
                plugin: plugin.id.clone(),
                title: s.title.clone(),
                text: s.text.clone(),
            }));
        if let Some(css) = &plugin.theme {
            out.themes.push(ThemeChoice {
                plugin: plugin.id.clone(),
                name: plugin.name.clone(),
            });
            if chosen
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case(&plugin.id))
            {
                out.theme = Some(ActiveTheme {
                    plugin: plugin.id.clone(),
                    css: css.clone(),
                });
            }
        }
        if !plugin.fields.is_empty() {
            let stored = store.plugin_settings(&plugin.id).await?;
            out.settings.push(SettingsGroup {
                plugin: plugin.id.clone(),
                name: plugin.name.clone(),
                fields: plugin
                    .fields
                    .iter()
                    .map(|f| view_of(f, stored.get(&f.key)))
                    .collect(),
            });
        }
    }
    Ok(out)
}
