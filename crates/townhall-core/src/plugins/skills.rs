//! The skills and slash commands a plugin ships: found at install (they go on the permission
//! list) and staged per task at launch (`--plugin-dir`, `docs/plugins.md` section 6.5).

use std::path::{Path, PathBuf};

use super::check;
use super::claude_mcp::{failed, selects};
use super::install::cache_root;
use super::market::blocking;
use crate::store::{Plugin, Store};
use crate::{Error, Home, Result};

pub const DEFAULT_SKILLS: &str = "skills";
pub const DEFAULT_COMMANDS: &str = "commands";
const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declared {
    pub skills: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub skills: Vec<String>,
    pub commands: Vec<String>,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.commands.is_empty()
    }
}

pub fn inside(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

fn listed(what: &str, dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in
        std::fs::read_dir(dir).map_err(|e| Error::io(format!("read {}", dir.display()), e))?
    {
        found.push(entry.map_err(|e| check::invalid(what, e))?.path());
    }
    found.sort();
    Ok(found)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn skills_in(what: &str, path: &Path, out: &mut Vec<String>) -> Result<()> {
    if path.join(SKILL_FILE).is_file() {
        out.push(name_of(path));
        return Ok(());
    }
    for child in listed(what, path)? {
        if child.join(SKILL_FILE).is_file() {
            out.push(name_of(&child));
        }
    }
    Ok(())
}

fn commands_in(what: &str, path: &Path, out: &mut Vec<String>) -> Result<()> {
    let is_command = |p: &Path| p.is_file() && p.extension().is_some_and(|e| e == "md");
    let stem = |p: &Path| {
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    if is_command(path) {
        out.push(stem(path));
        return Ok(());
    }
    for child in listed(what, path)? {
        if is_command(&child) {
            out.push(stem(&child));
        }
    }
    Ok(())
}

fn bases(root: &Path, what: &str, default: &str, declared: &[String]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let default = root.join(default);
    if default.is_dir() {
        paths.push(default);
    }
    for rel in declared {
        let path = root.join(inside(rel));
        if !path.exists() {
            return Err(check::invalid(
                what,
                format!("`{rel}` does not exist in the plugin"),
            ));
        }
        paths.push(path);
    }
    Ok(paths)
}

const LEFT_OUT: [&str; 6] = [
    ".claude-plugin",
    ".townhall-plugin",
    ".git",
    "hooks",
    ".mcp.json",
    "agents",
];
const MAX_DEPTH: usize = 32;

fn top_of(rel: &str) -> &str {
    inside(rel).split('/').next().unwrap_or_default()
}

fn copy_tree(from: &Path, to: &Path, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(check::invalid("plugin", "folders are nested too deep"));
    }
    let io = |what: &str, path: &Path, e| Error::io(format!("{what} {}", path.display()), e);
    let meta = std::fs::metadata(from).map_err(|e| io("read", from, e))?;
    if meta.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| io("create", to, e))?;
        for entry in std::fs::read_dir(from).map_err(|e| io("read", from, e))? {
            let entry = entry.map_err(|e| io("read", from, e))?;
            copy_tree(&entry.path(), &to.join(entry.file_name()), depth + 1)?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| io("copy", from, e))?;
    Ok(())
}

fn manifest_text(name: &str, version: Option<&str>, declared: &Declared) -> String {
    let relative = |paths: &[String]| -> Vec<String> {
        paths.iter().map(|p| format!("./{}", inside(p))).collect()
    };
    let mut manifest = serde_json::Map::new();
    manifest.insert("name".into(), name.into());
    if let Some(version) = version {
        manifest.insert("version".into(), version.into());
    }
    if !declared.skills.is_empty() {
        manifest.insert("skills".into(), relative(&declared.skills).into());
    }
    if !declared.commands.is_empty() {
        manifest.insert("commands".into(), relative(&declared.commands).into());
    }
    serde_json::Value::Object(manifest).to_string()
}

/// Builds `dest`, a plugin folder claude loads with `--plugin-dir`: a generated manifest, real
/// copies of the skills and commands, and links to everything else the plugin ships except
/// what townhall injects itself (hooks, MCP servers, subagents).
pub fn stage(
    root: &Path,
    dest: &Path,
    name: &str,
    version: Option<&str>,
    declared: &Declared,
) -> Result<()> {
    let io = |what: &str, path: &Path, e| Error::io(format!("{what} {}", path.display()), e);
    match std::fs::remove_dir_all(dest) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io("remove", dest, e)),
    }
    let meta = dest.join(".claude-plugin");
    std::fs::create_dir_all(&meta).map_err(|e| io("create", &meta, e))?;
    let manifest = meta.join("plugin.json");
    std::fs::write(&manifest, manifest_text(name, version, declared))
        .map_err(|e| io("write", &manifest, e))?;
    let copied: Vec<&str> = [DEFAULT_SKILLS, DEFAULT_COMMANDS]
        .into_iter()
        .chain(declared.skills.iter().map(|p| inside(p)))
        .chain(declared.commands.iter().map(|p| inside(p)))
        .collect();
    for rel in &copied {
        let from = root.join(rel);
        if from.exists() {
            copy_tree(&from, &dest.join(rel), 0)?;
        }
    }
    let tops: Vec<&str> = copied.iter().map(|rel| top_of(rel)).collect();
    for entry in std::fs::read_dir(root).map_err(|e| io("read", root, e))? {
        let entry = entry.map_err(|e| io("read", root, e))?;
        let file = entry.file_name();
        let top = file.to_string_lossy();
        if LEFT_OUT.contains(&top.as_ref()) || tops.contains(&top.as_ref()) {
            continue;
        }
        let link = dest.join(&file);
        std::os::unix::fs::symlink(entry.path(), &link).map_err(|e| io("link", &link, e))?;
    }
    Ok(())
}

fn declared_of(source: &serde_json::Value) -> Declared {
    let paths = |key: &str| -> Vec<String> {
        source
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    Declared {
        skills: paths("skills"),
        commands: paths("commands"),
    }
}

fn folder_name(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn stage_plugin(home: &Home, plugin: &Plugin, under: &Path) -> Result<Option<PathBuf>> {
    let root = cache_root(home, &plugin.marketplace, &plugin.name)?.join(&plugin.sha);
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", root.display()), e))?;
    let declared = declared_of(&plugin.source);
    if discover(&root, &declared)?.is_empty() {
        return Ok(None);
    }
    let dest = under.join(folder_name(&plugin.id));
    stage(
        &root,
        &dest,
        &plugin.name,
        plugin.version.as_deref(),
        &declared,
    )?;
    Ok(Some(dest))
}

/// The folders to pass to `--plugin-dir` for the enabled plugins in `names` that ship skills
/// or commands, staged under `under`. A plugin with neither adds nothing.
pub async fn selected(store: &Store, names: &[String], under: PathBuf) -> Result<Vec<PathBuf>> {
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
        let mut dirs = Vec::new();
        for plugin in &plugins {
            if let Some(dir) = stage_plugin(&home, plugin, &under).map_err(|e| failed(plugin, e))? {
                dirs.push(dir);
            }
        }
        Ok(dirs)
    })
    .await
}

pub fn discover(root: &Path, declared: &Declared) -> Result<Found> {
    let mut found = Found::default();
    for path in bases(root, "skills", DEFAULT_SKILLS, &declared.skills)? {
        skills_in("skills", &path, &mut found.skills)?;
    }
    for path in bases(root, "commands", DEFAULT_COMMANDS, &declared.commands)? {
        commands_in("commands", &path, &mut found.commands)?;
    }
    found.skills.sort();
    found.skills.dedup();
    found.commands.sort();
    found.commands.dedup();
    Ok(found)
}
