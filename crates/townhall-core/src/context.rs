//! Everything the daemon's handlers and the background units share, built once per daemon:
//! the startup repo, the store (with the repo registry over it), the event bus and options.
//! The daemon's `AppState` wraps it; core units (runner, scheduler, watcher) take it directly.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::agent::DEFAULT_CLAUDE_BIN;
use crate::events::EventBus;
use crate::model::{Repo, Settings};
use crate::repos::{names, RepoRegistry};
use crate::runner::Turns;
use crate::store::Store;
use crate::{Error, Home, Result, Workspace};

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:7777";
pub const CLAUDE_BIN_ENV: &str = "TOWNHALL_CLAUDE_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOptions {
    /// Where this daemon listens, so townhall's session can reach its MCP endpoint.
    pub base_url: String,
    /// The `claude` program; tests point `TOWNHALL_CLAUDE_BIN` at a fake.
    pub claude_bin: String,
}

fn claude_bin_from_env() -> String {
    std::env::var(CLAUDE_BIN_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLAUDE_BIN.to_string())
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            claude_bin: claude_bin_from_env(),
        }
    }
}

impl ContextOptions {
    pub fn for_addr(addr: SocketAddr) -> Self {
        Self {
            base_url: format!("http://{addr}"),
            ..Self::default()
        }
    }
}

#[derive(Debug)]
struct Inner {
    startup: Workspace,
    /// The open store, or why it could not be opened (every use then reports that).
    store: std::result::Result<Store, String>,
    bus: EventBus,
    options: ContextOptions,
    startup_name: OnceCell<Option<String>>,
    turns: Turns,
}

#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

impl Context {
    /// Opens the store in the startup repo's home. If that fails (another townhall holds
    /// it), the context still serves what needs no store and reports the error on each use.
    pub fn new(startup: Workspace, options: ContextOptions) -> Self {
        let bus = EventBus::new();
        let store = Store::open(startup.home(), bus.clone()).map_err(|e| e.to_string());
        Self::build(startup, options, store, bus)
    }

    /// Like `new`, but a store that cannot be opened is an error.
    pub fn open(startup: Workspace, options: ContextOptions) -> Result<Self> {
        let bus = EventBus::new();
        let store = Store::open(startup.home(), bus.clone())?;
        Ok(Self::with_store(startup, options, store))
    }

    /// Uses a store opened earlier: the desktop app keeps one across daemon restarts.
    pub fn with_store(startup: Workspace, options: ContextOptions, store: Store) -> Self {
        let bus = store.bus().clone();
        Self::build(startup, options, Ok(store), bus)
    }

    fn build(
        startup: Workspace,
        options: ContextOptions,
        store: std::result::Result<Store, String>,
        bus: EventBus,
    ) -> Self {
        let inner = Inner {
            startup,
            store,
            bus,
            options,
            startup_name: OnceCell::new(),
            turns: Turns::default(),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn home(&self) -> &Home {
        self.inner.startup.home()
    }

    /// The repo the daemon was started with (`--repo`): the default for `POST /api/agents`.
    pub fn startup(&self) -> &Workspace {
        &self.inner.startup
    }

    /// Every stored record goes through this (typed functions, no SQL outside `store`).
    pub fn store(&self) -> Result<&Store> {
        self.inner
            .store
            .as_ref()
            .map_err(|e| Error::Unavailable(e.clone()))
    }

    pub fn registry(&self) -> Result<RepoRegistry> {
        self.store().map(|s| RepoRegistry::new(s.clone()))
    }

    pub fn bus(&self) -> &EventBus {
        &self.inner.bus
    }

    pub fn base_url(&self) -> &str {
        &self.inner.options.base_url
    }

    pub fn claude_bin(&self) -> &str {
        &self.inner.options.claude_bin
    }

    /// The turns of townhall's own session running now (unit 2.2's runner).
    pub fn turns(&self) -> &Turns {
        &self.inner.turns
    }

    pub async fn settings(&self) -> Result<Settings> {
        self.store()?.settings().await
    }

    /// Registers the startup repo once per process and returns its name.
    /// `None` when it cannot be registered (for example it is not a git repo).
    pub async fn ensure_startup_repo(&self) -> Option<String> {
        let init = || async {
            let registered = match self.registry() {
                Ok(registry) => registry.ensure(self.inner.startup.repo()).await,
                Err(e) => Err(e),
            };
            match registered {
                Ok(repo) => Some(repo.name),
                Err(e) => {
                    eprintln!(
                        "warn: startup repo {} not registered: {e}",
                        self.inner.startup.repo().display()
                    );
                    None
                }
            }
        };
        self.inner.startup_name.get_or_init(init).await.clone()
    }

    /// The startup repo's registered name, else its folder name.
    pub async fn startup_repo_name(&self) -> String {
        match self.ensure_startup_repo().await {
            Some(name) => name,
            None => names::from_folder(self.inner.startup.repo()),
        }
    }

    pub async fn repos(&self) -> Result<Vec<Repo>> {
        self.ensure_startup_repo().await;
        self.registry()?.list().await
    }

    /// The registered repo called `name` (ignoring case).
    pub async fn repo(&self, name: &str) -> Result<Repo> {
        self.ensure_startup_repo().await;
        self.registry()?.get(name).await
    }

    pub fn workspace(&self, repo: &Repo) -> Result<Workspace> {
        Workspace::new(self.home().clone(), &repo.path)
    }
}
