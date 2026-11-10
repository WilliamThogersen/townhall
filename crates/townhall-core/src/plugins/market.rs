use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::check;
use super::marketplace::{self, MAX_MARKETPLACE_BYTES};
use super::OFFICIAL;
use crate::clock;
use crate::redact::redact;
use crate::repos::publish::Programs;
use crate::repos::remote_git::{self, AuthEnv};
use crate::repos::resolve_path;
use crate::repos::source::{classify, Remote, Source};
use crate::store::{Marketplace, Store};
use crate::{Error, Home, Result};

pub const MARKETPLACES_DIR: &str = "marketplaces";

const INDEX_PATHS: [&str; 2] = [
    ".townhall-plugin/marketplace.json",
    ".claude-plugin/marketplace.json",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddMarketplace {
    pub source: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceView {
    pub name: String,
    pub source: Value,
    pub account: Option<String>,
    pub enabled: bool,
    pub official: bool,
    pub head_sha: Option<String>,
    pub fetched_at: Option<String>,
    pub error: Option<String>,
    pub plugins: Vec<PluginSummary>,
}

impl MarketplaceView {
    fn of(row: Marketplace, plugins: Vec<PluginSummary>, problem: Option<String>) -> Self {
        Self {
            name: row.name,
            source: row.source,
            account: row.account,
            enabled: row.enabled,
            official: row.official,
            head_sha: row.head_sha,
            fetched_at: row.fetched_at,
            error: row.error.or(problem),
            plugins,
        }
    }
}

enum Target {
    Folder(PathBuf),
    Remote(Remote),
}

enum Place {
    Directory(PathBuf),
    Remote(String),
}

fn place_of(source: &Value) -> Result<Place> {
    let text = |key: &str| source.get(key).and_then(Value::as_str);
    let found = match source.get("source").and_then(Value::as_str) {
        Some("directory") => text("path").map(|p| Place::Directory(PathBuf::from(p))),
        Some("url") => text("url").map(|u| Place::Remote(u.to_string())),
        Some("github") => text("repo").map(|r| Place::Remote(format!("https://github.com/{r}"))),
        _ => None,
    };
    found.ok_or_else(|| Error::Corrupt {
        path: "marketplaces.source".into(),
        message: format!("not a marketplace source: {source}"),
    })
}

fn is_shorthand(input: &str) -> bool {
    let part = |s: &str| {
        s.starts_with(|c: char| c.is_ascii_alphanumeric())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    match input.split_once('/') {
        Some((owner, repo)) => part(owner) && part(repo),
        None => false,
    }
}

fn refuse_credentials(url: &str) -> Result<()> {
    let Some((scheme, rest)) = url.split_once("://") else {
        return Ok(());
    };
    let web = matches!(scheme.to_ascii_lowercase().as_str(), "https" | "http");
    let authority = rest.split('/').next().unwrap_or_default();
    if web && authority.contains('@') {
        return Err(Error::InvalidInput(
            "the url must not hold a user name or a token: register the account in settings and pass `account`".into(),
        ));
    }
    Ok(())
}

fn target_of(input: &str) -> Result<Target> {
    let remote = |url: &str| match classify(url)? {
        Source::Remote(remote) => refuse_credentials(&remote.url).map(|()| Target::Remote(remote)),
        Source::Path(path) => Err(Error::InvalidInput(format!("`{path}` is not a git url"))),
    };
    match classify(input)? {
        Source::Remote(_) => remote(input.trim()),
        Source::Path(path) if is_shorthand(&path) => remote(&format!("https://github.com/{path}")),
        Source::Path(path) if path.starts_with('~') || Path::new(&path).is_absolute() => {
            resolve_path(&path).map(Target::Folder)
        }
        Source::Path(path) => Err(Error::InvalidInput(format!(
            "`{path}` is not a git url, an `owner/repo` or an absolute folder"
        ))),
    }
}

fn reserved(name: &str) -> Result<()> {
    if OFFICIAL.is(name) {
        return Err(Error::Conflict(format!(
            "the marketplace name `{}` is reserved for the built in marketplace",
            OFFICIAL.name
        )));
    }
    Ok(())
}

fn marketplaces_dir(home: &Home) -> Result<PathBuf> {
    let dir = home.plugins_dir()?.join(MARKETPLACES_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(format!("create {}", dir.display()), e))?;
    Ok(dir)
}

fn read_index_dir(dir: &Path) -> Result<marketplace::Marketplace> {
    for rel in INDEX_PATHS {
        let path = dir.join(rel);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(Error::io(format!("read {}", path.display()), e)),
        };
        if !meta.is_file() {
            return Err(Error::InvalidInput(format!("{rel} is not a regular file")));
        }
        if meta.len() > MAX_MARKETPLACE_BYTES as u64 {
            return Err(Error::TooLarge(format!(
                "{rel} is {} bytes, the limit is {MAX_MARKETPLACE_BYTES}",
                meta.len()
            )));
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::io(format!("read {}", path.display()), e))?;
        return marketplace::parse(&text);
    }
    Err(no_index(&dir.display().to_string()))
}

fn no_index(whence: &str) -> Error {
    Error::InvalidInput(format!(
        "no marketplace.json in {whence}: expected {} or {}",
        INDEX_PATHS[0], INDEX_PATHS[1]
    ))
}

fn index_at(dir: &Path, sha: &str) -> Result<marketplace::Marketplace> {
    let none = AuthEnv::new();
    for rel in INDEX_PATHS {
        let spec = format!("{sha}:{rel}");
        let size = match remote_git::run(&["cat-file", "-s", &spec], Some(dir), &none) {
            Ok(size) => size,
            Err(Error::Command { .. }) => continue,
            Err(e) => return Err(e),
        };
        if size
            .parse::<usize>()
            .map_or(true, |n| n > MAX_MARKETPLACE_BYTES)
        {
            return Err(Error::TooLarge(format!(
                "{rel} is {size} bytes, the limit is {MAX_MARKETPLACE_BYTES}"
            )));
        }
        let text = remote_git::run(&["cat-file", "blob", &spec], Some(dir), &none)?;
        return marketplace::parse(&text);
    }
    Err(no_index(&format!("commit {sha}")))
}

fn head_of(dir: &Path) -> Result<Option<String>> {
    if !dir.join(".git").exists() {
        return Ok(None);
    }
    match remote_git::run(&["rev-parse", "HEAD"], Some(dir), &AuthEnv::new()) {
        Ok(sha) => Ok(Some(sha)),
        Err(Error::Command { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

fn summaries(market: &marketplace::Marketplace) -> Vec<PluginSummary> {
    market
        .plugins
        .iter()
        .map(|p| PluginSummary {
            name: p.name.clone(),
            description: p.description.clone(),
            version: p.version.clone(),
            category: p.category.clone(),
        })
        .collect()
}

fn listed(home: &Home, row: &Marketplace) -> Result<marketplace::Marketplace> {
    match place_of(&row.source)? {
        Place::Directory(path) => read_index_dir(&path),
        Place::Remote(_) => {
            let dir = marketplaces_dir(home)?.join(&row.name);
            if dir.exists() {
                read_index_dir(&dir)
            } else if row.official {
                marketplace::parse(OFFICIAL.snapshot)
            } else {
                Err(Error::InvalidInput(
                    "the clone of this marketplace is missing: refresh it".into(),
                ))
            }
        }
    }
}

fn view(home: &Home, row: Marketplace) -> MarketplaceView {
    match listed(home, &row) {
        Ok(market) => MarketplaceView::of(row, summaries(&market), None),
        Err(e) => MarketplaceView::of(row, Vec::new(), Some(redact(&e.to_string()).into_owned())),
    }
}

fn sync(
    programs: &Programs,
    home: &Home,
    row: &Marketplace,
    registered: &[crate::model::ForgeAccount],
) -> Result<Option<String>> {
    let url = match place_of(&row.source)? {
        Place::Directory(path) => {
            read_index_dir(&path)?;
            return head_of(&path);
        }
        Place::Remote(url) => url,
    };
    let parent = marketplaces_dir(home)?;
    let dest = parent.join(&row.name);
    let auth = remote_git::auth_env(programs, &url, row.account.as_deref(), registered, &parent)?;
    if !dest.exists() {
        let Source::Remote(remote) = classify(&url)? else {
            return Err(Error::InvalidInput(format!("`{url}` is not a git url")));
        };
        remote_git::clone(&remote, &dest, &auth)?;
        let checked = read_index_dir(&dest).and_then(|_| head_of(&dest));
        if checked.is_err() {
            let _ = std::fs::remove_dir_all(&dest);
        }
        return checked;
    }
    let fetched = remote_git::run(&["fetch", "-q", "origin", "HEAD"], Some(&dest), &auth);
    if let Err(Error::Command { stderr, .. }) = &fetched {
        return Err(remote_git::failed("fetch", &url, stderr));
    }
    fetched?;
    let none = AuthEnv::new();
    let sha = remote_git::run(
        &["rev-parse", "--verify", "FETCH_HEAD^{commit}"],
        Some(&dest),
        &none,
    )?;
    index_at(&dest, &sha)?;
    remote_git::run(
        &["checkout", "-q", "--detach", "--force", &sha],
        Some(&dest),
        &none,
    )?;
    Ok(Some(sha))
}

async fn blocking<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::InvalidInput(format!("git check failed: {e}")))?
}

#[derive(Debug, Clone)]
pub struct MarketplaceRegistry {
    store: Store,
    programs: Programs,
}

impl MarketplaceRegistry {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            programs: Programs::from_env(),
        }
    }

    async fn ensure_official(&self) -> Result<()> {
        let row = Marketplace {
            name: OFFICIAL.name.to_string(),
            source: json!({"source": "github", "repo": OFFICIAL.repo}),
            account: None,
            enabled: true,
            official: true,
            head_sha: Some(OFFICIAL.sha.to_string()),
            fetched_at: None,
            error: None,
        };
        match self.store.add_marketplace(row).await {
            Ok(_) | Err(Error::Conflict(_)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn list(&self) -> Result<Vec<MarketplaceView>> {
        self.ensure_official().await?;
        let rows = self.store.marketplaces().await?;
        let home = self.store.home().clone();
        blocking(move || Ok(rows.into_iter().map(|row| view(&home, row)).collect())).await
    }

    pub async fn add(&self, req: &AddMarketplace) -> Result<MarketplaceView> {
        self.ensure_official().await?;
        let wanted = req.name.as_deref().map(str::trim);
        if let Some(name) = wanted {
            check::name("marketplace name", name)?;
            reserved(name)?;
        }
        let account = req
            .account
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_string);
        let home = self.store.home().clone();
        let row = match target_of(&req.source)? {
            Target::Folder(path) => {
                if account.is_some() {
                    return Err(Error::InvalidInput(
                        "an account only applies to a git url, not to a folder".into(),
                    ));
                }
                let found = path.clone();
                let (market, sha) = blocking(move || {
                    let market = read_index_dir(&found)?;
                    head_of(&found).map(|sha| (market, sha))
                })
                .await?;
                let source = json!({"source": "directory", "path": path});
                self.register(wanted, &market, source, None, sha).await?
            }
            Target::Remote(remote) => self.add_remote(&home, wanted, remote, account).await?,
        };
        let home = self.store.home().clone();
        blocking(move || Ok(view(&home, row))).await
    }

    async fn add_remote(
        &self,
        home: &Home,
        wanted: Option<&str>,
        remote: Remote,
        account: Option<String>,
    ) -> Result<Marketplace> {
        let registered = self.store.settings().await?.forge_accounts;
        let parent = marketplaces_dir(home)?;
        let tmp = parent.join(format!(".adding-{}", uuid::Uuid::new_v4().simple()));
        let (programs, url, found, at) = (
            self.programs.clone(),
            remote.url.clone(),
            remote.clone(),
            tmp.clone(),
        );
        let who = account.clone();
        let cloned = blocking(move || {
            let auth = remote_git::auth_env(&programs, &url, who.as_deref(), &registered, &parent)?;
            remote_git::clone(&found, &at, &auth)?;
            let read = read_index_dir(&at).and_then(|m| head_of(&at).map(|sha| (m, sha)));
            if read.is_err() {
                let _ = std::fs::remove_dir_all(&at);
            }
            read
        })
        .await?;
        let (market, sha) = cloned;
        let source = json!({"source": "url", "url": remote.url});
        let row = match self.register(wanted, &market, source, account, sha).await {
            Ok(row) => row,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(e);
            }
        };
        let dest = marketplaces_dir(home)?.join(&row.name);
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_dir_all(&tmp);
            self.store.remove_marketplace(&row.name).await?;
            return Err(Error::io(
                format!("move the clone to {}", dest.display()),
                e,
            ));
        }
        Ok(row)
    }

    async fn register(
        &self,
        wanted: Option<&str>,
        market: &marketplace::Marketplace,
        source: Value,
        account: Option<String>,
        head_sha: Option<String>,
    ) -> Result<Marketplace> {
        let name = wanted.unwrap_or(&market.name).to_string();
        reserved(&name)?;
        self.store
            .add_marketplace(Marketplace {
                name,
                source,
                account,
                enabled: true,
                official: false,
                head_sha,
                fetched_at: Some(clock::now_rfc3339()),
                error: None,
            })
            .await
    }

    pub async fn remove(&self, name: &str) -> Result<MarketplaceView> {
        if OFFICIAL.is(name) {
            return Err(Error::Conflict(format!(
                "`{}` is built in: it cannot be removed",
                OFFICIAL.name
            )));
        }
        let row = self.store.remove_marketplace(name).await?;
        if let Place::Remote(_) = place_of(&row.source)? {
            let dir = marketplaces_dir(self.store.home())?.join(&row.name);
            if dir.exists() {
                std::fs::remove_dir_all(&dir)
                    .map_err(|e| Error::io(format!("remove {}", dir.display()), e))?;
            }
        }
        Ok(MarketplaceView::of(row, Vec::new(), None))
    }

    pub async fn refresh(&self, name: &str) -> Result<MarketplaceView> {
        self.ensure_official().await?;
        let row = self.store.marketplace(name).await?;
        let registered = self.store.settings().await?.forge_accounts;
        let (programs, home, found) = (
            self.programs.clone(),
            self.store.home().clone(),
            row.clone(),
        );
        let synced = blocking(move || sync(&programs, &home, &found, &registered)).await;
        match synced {
            Ok(head_sha) => {
                let saved = self
                    .store
                    .update_marketplace(Marketplace {
                        head_sha,
                        fetched_at: Some(clock::now_rfc3339()),
                        error: None,
                        ..row
                    })
                    .await?;
                let home = self.store.home().clone();
                blocking(move || Ok(view(&home, saved))).await
            }
            Err(e) => {
                let message = redact(&e.to_string()).into_owned();
                self.store
                    .update_marketplace(Marketplace {
                        error: Some(message),
                        ..row
                    })
                    .await?;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const INDEX: &str = r#"{"name":"acme","owner":{"name":"A"},"plugins":[]}"#;

    #[test]
    fn owner_slash_repo_is_github_and_other_shapes_are_not() {
        assert!(is_shorthand("acme/plugins"));
        assert!(is_shorthand("a.b/c_d-e"));
        for no in [
            "acme",
            "/acme/plugins",
            "./a/b",
            "a/b/c",
            "~/a",
            "a/",
            "/b",
            "-a/b",
        ] {
            assert!(!is_shorthand(no), "{no}");
        }
    }

    #[test]
    fn shorthand_urls_and_folders_each_find_their_target() {
        match target_of("acme/plugins").unwrap() {
            Target::Remote(r) => assert_eq!(r.url, "https://github.com/acme/plugins"),
            Target::Folder(_) => panic!("folder"),
        }
        match target_of("git@gitlab.com:acme/plugins.git").unwrap() {
            Target::Remote(r) => assert_eq!(r.name, "plugins"),
            Target::Folder(_) => panic!("folder"),
        }
        let dir = tempfile::tempdir().unwrap();
        match target_of(&dir.path().to_string_lossy()).unwrap() {
            Target::Folder(p) => assert_eq!(p, dir.path().canonicalize().unwrap()),
            Target::Remote(_) => panic!("remote"),
        }
        assert!(target_of("relative").is_err());
        assert!(target_of("./relative/x").is_err());
    }

    #[test]
    fn a_url_with_a_user_or_token_is_refused_without_echoing_it() {
        for url in [
            "https://me:s3cret@github.com/acme/plugins.git",
            "https://s3cret@github.com/acme/plugins.git",
        ] {
            let err = target_of(url).err().unwrap().to_string();
            assert!(err.contains("must not hold"), "{err}");
            assert!(!err.contains("s3cret"), "{err}");
        }
        assert!(target_of("ssh://git@github.com/acme/plugins.git").is_ok());
        assert!(target_of("git@github.com:acme/plugins.git").is_ok());
    }

    #[test]
    fn the_townhall_index_wins_over_the_claude_one() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".claude-plugin/marketplace.json", INDEX);
        assert_eq!(read_index_dir(dir.path()).unwrap().name, "acme");
        let other = INDEX.replace("acme", "first");
        write(dir.path(), ".townhall-plugin/marketplace.json", &other);
        assert_eq!(read_index_dir(dir.path()).unwrap().name, "first");
    }

    #[test]
    fn a_folder_without_an_index_or_with_a_bad_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_index_dir(dir.path()).unwrap_err().to_string();
        assert!(err.contains("no marketplace.json"), "{err}");
        write(dir.path(), ".townhall-plugin/marketplace.json", "{ nope");
        let err = read_index_dir(dir.path()).unwrap_err().to_string();
        assert!(err.contains("marketplace.json"), "{err}");
    }

    #[test]
    fn an_index_that_is_a_symlink_is_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, INDEX).unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".townhall-plugin")).unwrap();
        std::os::unix::fs::symlink(&outside, repo.join(".townhall-plugin/marketplace.json"))
            .unwrap();
        let err = read_index_dir(&repo).unwrap_err().to_string();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn an_oversized_index_is_refused_before_it_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let big = " ".repeat(MAX_MARKETPLACE_BYTES + 1);
        write(dir.path(), ".townhall-plugin/marketplace.json", &big);
        let err = read_index_dir(dir.path()).unwrap_err();
        assert!(matches!(err, Error::TooLarge(_)), "{err}");
    }

    #[test]
    fn the_reserved_name_is_a_conflict_ignoring_case() {
        assert!(matches!(
            reserved("Townhall-Official"),
            Err(Error::Conflict(_))
        ));
        assert!(reserved("acme").is_ok());
    }

    #[test]
    fn stored_sources_decode_and_unknown_ones_are_corrupt() {
        let url = place_of(&json!({"source": "github", "repo": "a/b"})).unwrap();
        assert!(matches!(url, Place::Remote(u) if u == "https://github.com/a/b"));
        let dir = place_of(&json!({"source": "directory", "path": "/x"})).unwrap();
        assert!(matches!(dir, Place::Directory(p) if p == Path::new("/x")));
        let err = place_of(&json!({"source": "ftp"})).err().unwrap();
        assert!(matches!(err, Error::Corrupt { .. }), "{err}");
    }
}
