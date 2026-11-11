use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::check;
use super::market::{
    blocking, head_of, listed, marketplaces_dir, place_of, MarketplaceRegistry, Place,
};
use super::marketplace::{Entry, Source};
use super::tree;
use crate::model::ForgeAccount;
use crate::redact::redact;
use crate::repos::publish::Programs;
use crate::repos::remote_git::{self, AuthEnv};
use crate::store::{Marketplace, Plugin, Store};
use crate::{Error, Home, Result};

const CACHE_DIR: &str = "cache";
const DATA_DIR: &str = "data";
const NO_HOOKS: [&str; 2] = ["-c", "core.hooksPath=/dev/null"];
const NOT_FOUND: [&str; 3] = [
    "not found",
    "does not exist",
    "does not appear to be a git repository",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallPlugin {
    pub plugin: String,
    pub approve: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchPlugin {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub id: String,
    pub marketplace: String,
    pub name: String,
    pub version: Option<String>,
    pub sha: String,
    pub enabled: bool,
    pub pinned: bool,
    pub permissions: Vec<String>,
    pub permissions_hash: String,
    pub installed_at: String,
    pub updated_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailablePlugin {
    pub id: String,
    pub marketplace: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub available: Vec<AvailablePlugin>,
    pub installed: Vec<PluginView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub plugin: String,
    pub marketplace: String,
    pub name: String,
    pub version: Option<String>,
    pub sha: String,
    pub permissions: Vec<String>,
    pub permissions_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Unchanged,
    Updated,
    Pinned,
    NeedsApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub outcome: Outcome,
    pub sha: Option<String>,
    pub added: Vec<String>,
    pub permissions_hash: Option<String>,
    pub plugin: PluginView,
}

fn split_id(id: &str) -> Result<(&str, &str)> {
    let invalid = || {
        check::invalid(
            "plugin",
            format!("`{id}` must look like `name@marketplace`"),
        )
    };
    let (name, market) = id.split_once('@').ok_or_else(invalid)?;
    check::name("plugin name", name)?;
    check::name("marketplace name", market)?;
    Ok((name, market))
}

fn permissions_of(plugin: &Plugin) -> Result<Vec<String>> {
    serde_json::from_value(plugin.approved.clone()).map_err(|e| Error::Corrupt {
        path: "plugins.approved".into(),
        message: e.to_string(),
    })
}

fn view_of(plugin: Plugin) -> Result<PluginView> {
    Ok(PluginView {
        permissions: permissions_of(&plugin)?,
        id: plugin.id,
        marketplace: plugin.marketplace,
        name: plugin.name,
        version: plugin.version,
        sha: plugin.sha,
        enabled: plugin.enabled,
        pinned: plugin.pinned,
        permissions_hash: plugin.permissions_hash,
        installed_at: plugin.installed_at,
        updated_at: plugin.updated_at,
        error: plugin.error,
    })
}

pub(super) fn cache_root(home: &Home, market: &str, plugin: &str) -> Result<PathBuf> {
    Ok(home
        .plugins_dir()?
        .join(CACHE_DIR)
        .join(market)
        .join(plugin))
}

fn data_root(home: &Home, market: &str, plugin: &str) -> Result<PathBuf> {
    Ok(home.plugins_dir()?.join(DATA_DIR).join(market).join(plugin))
}

fn remove_tree(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(format!("remove {}", path.display()), e)),
    }
}

fn safe_env() -> AuthEnv {
    vec![
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

fn git(args: &[&str], cwd: &Path) -> Result<String> {
    let mut full: Vec<&str> = NO_HOOKS.to_vec();
    full.extend_from_slice(args);
    remote_git::run(&full, Some(cwd), &safe_env())
}

fn clone_failed(url: &str, stderr: &str, account: Option<&str>) -> Error {
    let lower = stderr.to_ascii_lowercase();
    if NOT_FOUND.iter().any(|needle| lower.contains(needle)) {
        let who = account.unwrap_or("the git credentials in use");
        return Error::InvalidInput(
            redact(&format!(
                "repository not found or not readable as {who}: {url}"
            ))
            .into_owned(),
        );
    }
    remote_git::failed("clone", url, stderr)
}

fn clone(url: &str, dest: &Path, auth: &AuthEnv, account: Option<&str>) -> Result<()> {
    let target = dest.to_string_lossy();
    let mut args: Vec<&str> = NO_HOOKS.to_vec();
    args.extend([
        "clone",
        "-q",
        "--no-recurse-submodules",
        "--no-checkout",
        "--",
    ]);
    args.extend([url, &target]);
    match remote_git::run(&args, None, auth) {
        Ok(_) => Ok(()),
        Err(Error::Command { stderr, .. }) => Err(clone_failed(url, &stderr, account)),
        Err(e) => Err(e),
    }
}

struct Spec {
    url: String,
    git_ref: Option<String>,
    sha: Option<String>,
    subdir: Option<String>,
    source: Value,
}

fn source_json(kind: &str, fields: &[(&str, Option<&str>)]) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert("source".into(), json!(kind));
    for (key, value) in fields {
        if let Some(value) = value {
            map.insert((*key).into(), json!(value));
        }
    }
    map
}

fn spec_of(home: &Home, market: &Marketplace, entry: &Entry) -> Result<Spec> {
    let (url, git_ref, sha, subdir, map) = match &entry.source {
        Source::Path(path) => {
            let dir = match place_of(&market.source)? {
                Place::Directory(dir) => dir,
                Place::Remote(_) => marketplaces_dir(home)?.join(&market.name),
            };
            let head = head_of(&dir)?.ok_or_else(|| {
                Error::InvalidInput(format!(
                    "`{}` has no git history to pin `{}` to: refresh the marketplace or keep it in a git repo",
                    market.name, entry.name
                ))
            })?;
            let inside = path.strip_prefix("./").unwrap_or_default();
            let subdir = (!inside.is_empty()).then(|| inside.to_string());
            let map = source_json("path", &[("path", Some(path))]);
            (
                dir.to_string_lossy().into_owned(),
                None,
                Some(head),
                subdir,
                map,
            )
        }
        Source::Github { repo, git_ref, sha } => (
            format!("https://github.com/{repo}"),
            git_ref.clone(),
            sha.clone(),
            None,
            source_json(
                "github",
                &[("repo", Some(repo)), ("ref", git_ref.as_deref())],
            ),
        ),
        Source::Url { url, git_ref, sha } => (
            url.clone(),
            git_ref.clone(),
            sha.clone(),
            None,
            source_json("url", &[("url", Some(url)), ("ref", git_ref.as_deref())]),
        ),
        Source::GitSubdir {
            url,
            path,
            git_ref,
            sha,
        } => (
            url.clone(),
            git_ref.clone(),
            sha.clone(),
            Some(path.clone()),
            source_json(
                "git-subdir",
                &[
                    ("url", Some(url)),
                    ("path", Some(path)),
                    ("ref", git_ref.as_deref()),
                ],
            ),
        ),
    };
    Ok(Spec {
        url,
        git_ref,
        sha,
        subdir,
        source: Value::Object(map),
    })
}

fn resolve(clone_dir: &Path, spec: &Spec) -> Result<String> {
    let (what, candidates) = match (&spec.sha, &spec.git_ref) {
        (Some(sha), _) => (format!("commit {sha}"), vec![format!("{sha}^{{commit}}")]),
        (None, Some(name)) => (
            format!("ref `{name}`"),
            vec![
                format!("refs/tags/{name}^{{commit}}"),
                format!("refs/remotes/origin/{name}^{{commit}}"),
            ],
        ),
        (None, None) => (
            "the default branch".to_string(),
            vec!["HEAD^{commit}".to_string()],
        ),
    };
    for candidate in &candidates {
        match git(&["rev-parse", "--verify", "-q", candidate], clone_dir) {
            Ok(sha) => return Ok(sha),
            Err(Error::Command { .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::InvalidInput(
        redact(&format!("{what} was not found in {}", spec.url)).into_owned(),
    ))
}

fn refuse_submodules(clone_dir: &Path, sha: &str, subdir: Option<&str>) -> Result<()> {
    let mut args = vec!["ls-tree", "-r", sha];
    if let Some(subdir) = subdir {
        args.extend(["--", subdir]);
    }
    let listing = git(&args, clone_dir)?;
    if listing.lines().any(|line| line.starts_with("160000 ")) {
        return Err(Error::InvalidInput(
            "the plugin holds a git submodule: submodules are not supported".into(),
        ));
    }
    Ok(())
}

fn folder_in(clone_dir: &Path, subdir: &str) -> Result<PathBuf> {
    let base = clone_dir
        .canonicalize()
        .map_err(|e| Error::io(format!("read {}", clone_dir.display()), e))?;
    let found = base
        .join(subdir)
        .canonicalize()
        .map_err(|_| check::invalid("source", format!("folder `{subdir}` is not in the repo")))?;
    if !found.starts_with(&base) || !found.is_dir() {
        return Err(check::invalid(
            "source",
            format!("`{subdir}` is not a folder inside the repo"),
        ));
    }
    Ok(found)
}

struct Job {
    programs: Programs,
    home: Home,
    market: Marketplace,
    entry: Entry,
    registered: Vec<ForgeAccount>,
}

struct Staged {
    tmp: PathBuf,
    root: PathBuf,
    sha: String,
    source: Value,
    version: Option<String>,
    permissions: Vec<String>,
    hash: String,
}

impl Staged {
    fn discard(self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }

    fn commit(self, dest: &Path) -> Result<()> {
        remove_tree(dest)?;
        std::fs::rename(&self.root, dest)
            .map_err(|e| Error::io(format!("move the plugin to {}", dest.display()), e))?;
        remove_tree(&self.tmp)
    }
}

fn stage(job: &Job) -> Result<Staged> {
    let parent = cache_root(&job.home, &job.market.name, &job.entry.name)?;
    let tmp = parent.join(format!(".staging-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp).map_err(|e| Error::io(format!("create {}", tmp.display()), e))?;
    match fetch(job, &tmp) {
        Ok(staged) => Ok(staged),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

fn fetch(job: &Job, tmp: &Path) -> Result<Staged> {
    let spec = spec_of(&job.home, &job.market, &job.entry)?;
    let account = job.market.account.as_deref();
    let auth = remote_git::auth_env(&job.programs, &spec.url, account, &job.registered, tmp)?;
    let clone_dir = tmp.join("clone");
    clone(&spec.url, &clone_dir, &auth, account)?;
    let sha = resolve(&clone_dir, &spec)?;
    refuse_submodules(&clone_dir, &sha, spec.subdir.as_deref())?;
    git(&["checkout", "-q", "--detach", "--force", &sha], &clone_dir)?;
    let root = match &spec.subdir {
        Some(subdir) => folder_in(&clone_dir, subdir)?,
        None => {
            remove_tree(&clone_dir.join(".git"))?;
            clone_dir
        }
    };
    let checked = tree::validate(&root, &job.entry)?;
    let mut source = spec.source;
    if let Some(map) = source.as_object_mut() {
        map.insert("sha".into(), json!(sha));
    }
    Ok(Staged {
        tmp: tmp.to_path_buf(),
        root,
        sha,
        source,
        version: checked.version,
        permissions: checked.permissions,
        hash: checked.hash,
    })
}

#[derive(Debug, Clone)]
pub struct PluginRegistry {
    store: Store,
    programs: Programs,
}

impl PluginRegistry {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            programs: Programs::from_env(),
        }
    }

    async fn job(&self, id: &str) -> Result<Job> {
        let (name, market) = split_id(id)?;
        MarketplaceRegistry::new(self.store.clone())
            .ensure_official()
            .await?;
        let row = self.store.marketplace(market).await?;
        let home = self.store.home().clone();
        let (found, wanted) = (row.clone(), name.to_string());
        let entry = blocking({
            let home = home.clone();
            move || {
                listed(&home, &found)?
                    .plugins
                    .into_iter()
                    .find(|e| e.name.eq_ignore_ascii_case(&wanted))
                    .ok_or_else(|| Error::missing("plugin", format!("{wanted}@{}", found.name)))
            }
        })
        .await?;
        Ok(Job {
            programs: self.programs.clone(),
            home,
            market: row,
            entry,
            registered: self.store.settings().await?.forge_accounts,
        })
    }

    pub async fn list(&self) -> Result<Listing> {
        MarketplaceRegistry::new(self.store.clone())
            .ensure_official()
            .await?;
        let rows = self.store.plugins().await?;
        let markets = self.store.marketplaces().await?;
        let home = self.store.home().clone();
        let installed_ids: Vec<String> = rows.iter().map(|p| p.id.clone()).collect();
        let available = blocking(move || {
            let mut found = Vec::new();
            for market in markets.iter().filter(|m| m.enabled) {
                let Ok(index) = listed(&home, market) else {
                    continue;
                };
                for entry in index.plugins {
                    let id = format!("{}@{}", entry.name, market.name);
                    found.push(AvailablePlugin {
                        installed: installed_ids.iter().any(|i| i.eq_ignore_ascii_case(&id)),
                        id,
                        marketplace: market.name.clone(),
                        name: entry.name,
                        description: entry.description,
                        version: entry.version,
                        category: entry.category,
                    });
                }
            }
            Ok(found)
        })
        .await?;
        let installed = rows.into_iter().map(view_of).collect::<Result<_>>()?;
        Ok(Listing {
            available,
            installed,
        })
    }

    pub async fn preview(&self, id: &str) -> Result<Preview> {
        let job = self.job(id).await?;
        let staged = blocking(move || {
            stage(&job).map(|s| {
                let shown = (
                    s.sha.clone(),
                    s.version.clone(),
                    s.permissions.clone(),
                    s.hash.clone(),
                );
                s.discard();
                shown
            })
        })
        .await?;
        let (sha, version, permissions, permissions_hash) = staged;
        let (name, market) = split_id(id)?;
        Ok(Preview {
            plugin: format!("{name}@{market}"),
            marketplace: market.to_string(),
            name: name.to_string(),
            version,
            sha,
            permissions,
            permissions_hash,
        })
    }

    pub async fn install(&self, req: &InstallPlugin) -> Result<PluginView> {
        let job = self.job(&req.plugin).await?;
        let id = format!("{}@{}", job.entry.name, job.market.name);
        let taken = self.store.plugins().await?;
        if taken.iter().any(|p| p.id.eq_ignore_ascii_case(&id)) {
            return Err(Error::Conflict(format!(
                "plugin `{id}` is already installed"
            )));
        }
        let approve = req.approve.clone();
        let (market, name) = (job.market.name.clone(), job.entry.name.clone());
        let (staged_id, home) = (id.clone(), job.home.clone());
        let installed = blocking(move || {
            let staged = stage(&job)?;
            if staged.hash != approve {
                staged.discard();
                return Err(Error::InvalidInput(format!(
                    "approve does not match the permissions of {staged_id}: ask for the preview again and approve what it lists"
                )));
            }
            let dest = cache_root(&home, &job.market.name, &job.entry.name)?.join(&staged.sha);
            let held = (
                staged.sha.clone(),
                staged.source.clone(),
                staged.version.clone(),
                staged.permissions.clone(),
                staged.hash.clone(),
            );
            staged.commit(&dest)?;
            let data = data_root(&home, &job.market.name, &job.entry.name)?;
            std::fs::create_dir_all(&data)
                .map_err(|e| Error::io(format!("create {}", data.display()), e))?;
            Ok((dest, held))
        })
        .await?;
        let (dest, (sha, source, version, permissions, hash)) = installed;
        let row = Plugin {
            id: String::new(),
            marketplace: market,
            name,
            version,
            sha,
            source,
            enabled: false,
            pinned: false,
            permissions_hash: hash,
            approved: json!(permissions),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        };
        match self.store.add_plugin(row).await {
            Ok(saved) => view_of(saved),
            Err(e) => {
                remove_tree(&dest)?;
                Err(e)
            }
        }
    }

    pub async fn patch(&self, id: &str, req: &PatchPlugin) -> Result<PluginView> {
        if req.enabled.is_none() && req.pinned.is_none() {
            return Err(Error::InvalidInput(
                "nothing to change: send `enabled` or `pinned`".into(),
            ));
        }
        let mut plugin = self.store.plugin(id).await?;
        if let Some(enabled) = req.enabled {
            plugin.enabled = enabled;
        }
        if let Some(pinned) = req.pinned {
            plugin.pinned = pinned;
        }
        view_of(self.store.update_plugin(plugin).await?)
    }

    pub async fn update(&self, id: &str, approve: Option<&str>) -> Result<UpdateResult> {
        let plugin = self.store.plugin(id).await?;
        if plugin.pinned {
            return Ok(UpdateResult {
                outcome: Outcome::Pinned,
                sha: None,
                added: Vec::new(),
                permissions_hash: None,
                plugin: view_of(plugin)?,
            });
        }
        let approved = permissions_of(&plugin)?;
        let job = self.job(&plugin.id).await?;
        let home = job.home.clone();
        let staged = blocking(move || stage(&job)).await?;
        if let Some(given) = approve {
            if given != staged.hash {
                staged.discard();
                return Err(Error::InvalidInput(format!(
                    "approve does not match the permissions of {}: ask for the update again and approve what it lists",
                    plugin.id
                )));
            }
        }
        if staged.sha == plugin.sha && staged.hash == plugin.permissions_hash {
            staged.discard();
            let plugin = if plugin.error.is_some() {
                self.store
                    .update_plugin(Plugin {
                        error: None,
                        ..plugin
                    })
                    .await?
            } else {
                plugin
            };
            return Ok(UpdateResult {
                outcome: Outcome::Unchanged,
                sha: None,
                added: Vec::new(),
                permissions_hash: None,
                plugin: view_of(plugin)?,
            });
        }
        let added = tree::added(&approved, &staged.permissions);
        if !added.is_empty() && approve.is_none() {
            let (sha, hash) = (staged.sha.clone(), staged.hash.clone());
            staged.discard();
            let saved = self
                .store
                .update_plugin(Plugin {
                    error: Some(format!("update to {sha} needs approval")),
                    ..plugin
                })
                .await?;
            return Ok(UpdateResult {
                outcome: Outcome::NeedsApproval,
                sha: Some(sha),
                added,
                permissions_hash: Some(hash),
                plugin: view_of(saved)?,
            });
        }
        let old_sha = plugin.sha.clone();
        let (market, name) = (plugin.marketplace.clone(), plugin.name.clone());
        let (new_sha, hash) = (staged.sha.clone(), staged.hash.clone());
        let fresh = new_sha != old_sha;
        let dest = cache_root(&home, &market, &name)?.join(&new_sha);
        let held = (
            staged.source.clone(),
            staged.version.clone(),
            staged.permissions.clone(),
        );
        blocking(move || {
            if fresh {
                staged.commit(&dest)
            } else {
                staged.discard();
                Ok(())
            }
        })
        .await?;
        let (source, version, permissions) = held;
        let saved = self
            .store
            .update_plugin(Plugin {
                version,
                sha: new_sha.clone(),
                source,
                permissions_hash: hash.clone(),
                approved: json!(permissions),
                error: None,
                ..plugin
            })
            .await;
        let saved = match saved {
            Ok(saved) => saved,
            Err(e) => {
                if fresh {
                    remove_tree(&cache_root(&home, &market, &name)?.join(&new_sha))?;
                }
                return Err(e);
            }
        };
        if fresh {
            remove_tree(&cache_root(&home, &market, &name)?.join(&old_sha))?;
        }
        Ok(UpdateResult {
            outcome: Outcome::Updated,
            sha: Some(new_sha),
            added,
            permissions_hash: Some(hash),
            plugin: view_of(saved)?,
        })
    }

    pub async fn uninstall(&self, id: &str, purge: bool) -> Result<PluginView> {
        let plugin = self.store.plugin(id).await?;
        let plugin = self
            .store
            .update_plugin(Plugin {
                enabled: false,
                ..plugin
            })
            .await?;
        let home = self.store.home();
        remove_tree(&cache_root(home, &plugin.marketplace, &plugin.name)?)?;
        if purge {
            remove_tree(&data_root(home, &plugin.marketplace, &plugin.name)?)?;
            self.store.clear_plugin_settings(&plugin.id).await?;
        }
        let theme = self.store.plugin_theme().await?;
        if theme.is_some_and(|id| id.eq_ignore_ascii_case(&plugin.id)) {
            self.store.set_plugin_theme(None).await?;
        }
        view_of(self.store.remove_plugin(&plugin.id).await?)
    }
}
