//! Everything the daemon's handlers and the background units share, built once per daemon:
//! the startup repo, the store (with the repo registry over it), the event bus and options.
//! The daemon's `AppState` wraps it; core units (runner, scheduler, watcher) take it directly.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::sync::OnceCell;

use crate::agent::DEFAULT_CLAUDE_BIN;
use crate::events::EventBus;
use crate::model::{Repo, Settings};
use crate::repos::{names, RepoRegistry};
use crate::runner::{Notes, Turns};
use crate::store::Store;
use crate::{Error, Home, Result, Workspace};

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:7777";
/// The default for `ContextOptions::notify_delay`.
pub const DEFAULT_NOTIFY_DELAY: Duration = Duration::from_secs(3);
pub const CLAUDE_BIN_ENV: &str = "TOWNHALL_CLAUDE_BIN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOptions {
    /// Where this daemon listens, so townhall's session can reach its MCP endpoint.
    pub base_url: String,
    /// The `claude` program; tests point `TOWNHALL_CLAUDE_BIN` at a fake.
    pub claude_bin: String,
    /// How long task events gather before they wake the planner (`runner::notify`).
    pub notify_delay: Duration,
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
            notify_delay: DEFAULT_NOTIFY_DELAY,
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

type FreeSpace = Arc<dyn Fn() -> Result<u64> + Send + Sync>;

/// Replaces the read of the data volume's free space; tests fake a filling disk with it.
#[derive(Default)]
struct FreeSource(Mutex<Option<FreeSpace>>);

impl std::fmt::Debug for FreeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeSource")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pacing {
    pub check_backoff: Duration,
    pub check_backoff_max: Duration,
    pub ci_poll: Duration,
    pub ci_timeout: Duration,
    pub ci_waits: usize,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            check_backoff: Duration::from_secs(60),
            check_backoff_max: Duration::from_secs(600),
            ci_poll: Duration::from_secs(30),
            ci_timeout: Duration::from_secs(3600),
            ci_waits: 3,
        }
    }
}

type Sleeper = Arc<dyn Fn(Duration) + Send + Sync>;

#[derive(Default)]
struct SleepSource(Mutex<Option<Sleeper>>);

impl std::fmt::Debug for SleepSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SleepSource")
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
    notes: Notes,
    disk_told: Mutex<HashSet<String>>,
    free_source: FreeSource,
    pacing: Mutex<Pacing>,
    sleeper: SleepSource,
}

#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

/// A `Context` that does not keep the daemon's store open: background work that sleeps
/// holds one, so a restart can reopen the store while it waits.
#[derive(Debug, Clone)]
pub struct WeakContext {
    inner: Weak<Inner>,
}

impl WeakContext {
    pub fn upgrade(&self) -> Option<Context> {
        self.inner.upgrade().map(|inner| Context { inner })
    }
}

impl Context {
    pub fn downgrade(&self) -> WeakContext {
        WeakContext {
            inner: Arc::downgrade(&self.inner),
        }
    }

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
            notes: Notes::default(),
            disk_told: Mutex::default(),
            free_source: FreeSource::default(),
            pacing: Mutex::new(Pacing::default()),
            sleeper: SleepSource::default(),
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

    /// The task events waiting to wake each conversation's planner.
    pub fn notes(&self) -> &Notes {
        &self.inner.notes
    }

    /// Marks a conversation as told the disk is too full; false when it already was.
    pub(crate) fn tell_disk_low(&self, conversation: &str) -> bool {
        let mut told = self
            .inner
            .disk_told
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        told.insert(conversation.to_string())
    }

    /// Forgets every told conversation: the disk has room again.
    pub(crate) fn clear_disk_told(&self) {
        let mut told = self
            .inner
            .disk_told
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        told.clear();
    }

    /// Bytes free on the volume townhall writes to: the real read, unless `set_free_space`
    /// replaced it.
    pub fn free_bytes(&self) -> Result<u64> {
        let fake = self
            .inner
            .free_source
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        match fake {
            Some(read) => read(),
            None => crate::disk::free_bytes(self.home().root()),
        }
    }

    /// Reads the free space from `read` from now on, instead of the volume (for tests).
    pub fn set_free_space(&self, read: impl Fn() -> Result<u64> + Send + Sync + 'static) {
        *self
            .inner
            .free_source
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(read));
    }

    pub fn pacing(&self) -> Pacing {
        *self.inner.pacing.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn set_pacing(&self, pacing: Pacing) {
        *self.inner.pacing.lock().unwrap_or_else(|p| p.into_inner()) = pacing;
    }

    pub fn sleep(&self, delay: Duration) -> impl std::future::Future<Output = ()> + Send + 'static {
        let fake = self
            .inner
            .sleeper
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        async move {
            match fake {
                Some(record) => record(delay),
                None => tokio::time::sleep(delay).await,
            }
        }
    }

    pub fn set_sleeper(&self, record: impl Fn(Duration) + Send + Sync + 'static) {
        *self
            .inner
            .sleeper
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(record));
    }

    pub fn notify_delay(&self) -> Duration {
        self.inner.options.notify_delay
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
                    tracing::warn!(
                        target: "townhall::store", event = "repo.startup_not_registered",
                        path = %self.inner.startup.repo().display(), error_kind = e.kind(),
                        error = %crate::redact::redact(&e.to_string()), "startup repo not registered"
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
