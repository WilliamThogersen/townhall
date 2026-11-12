//! The skills and slash commands a plugin ships: found at install (they go on the permission
//! list) and staged per task at launch (`--plugin-dir`, `docs/plugins.md` section 6.5).

use std::path::{Path, PathBuf};

use super::check;
use crate::{Error, Result};

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
        found.push(
            entry
                .map_err(|e| check::invalid(what, e))?
                .path(),
        );
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
    let stem = |p: &Path| p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
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
