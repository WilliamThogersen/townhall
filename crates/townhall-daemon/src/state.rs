use std::sync::Arc;

use townhall_core::store::Store;
use townhall_core::usage::UsageService;
use townhall_core::{Context, ContextOptions, Result, Workspace};

use crate::system::SystemSampler;

/// Shared state for every handler: the core `Context` (startup repo, store with the repo
/// registry, event bus, base URL, claude binary) built once per daemon, and the usage service.
#[derive(Debug, Clone)]
pub struct AppState {
    ctx: Context,
    usage: Arc<UsageService>,
    system: Arc<SystemSampler>,
}

impl AppState {
    /// Opens the store in the workspace's home. Base URL `http://127.0.0.1:7777`, claude from
    /// `TOWNHALL_CLAUDE_BIN` or `claude`. A store that cannot be opened is reported per request.
    pub fn new(workspace: Workspace) -> Self {
        Self::with_options(workspace, ContextOptions::default())
    }

    pub fn with_options(workspace: Workspace, options: ContextOptions) -> Self {
        Self::from_context(Context::new(workspace, options))
    }

    /// Like `with_options`, but a store that cannot be opened (another townhall holds it,
    /// or it is from a newer townhall) is an error. The daemon's `main` uses this.
    pub fn open(workspace: Workspace, options: ContextOptions) -> Result<Self> {
        Context::open(workspace, options).map(Self::from_context)
    }

    /// Uses a store opened earlier: the desktop app keeps one across daemon restarts.
    pub fn with_store(workspace: Workspace, options: ContextOptions, store: Store) -> Self {
        Self::from_context(Context::with_store(workspace, options, store))
    }

    fn from_context(ctx: Context) -> Self {
        let usage = Arc::new(UsageService::system(ctx.home()));
        let system = Arc::new(SystemSampler::new(ctx.home().root()));
        Self { ctx, usage, system }
    }

    /// Swaps the usage service, so tests need no Keychain and no network.
    pub fn with_usage(self, usage: Arc<UsageService>) -> Self {
        Self { usage, ..self }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn usage(&self) -> &Arc<UsageService> {
        &self.usage
    }

    pub fn system(&self) -> &Arc<SystemSampler> {
        &self.system
    }

    /// The startup repo's workspace.
    pub fn workspace(&self) -> &Workspace {
        self.ctx.startup()
    }
}
