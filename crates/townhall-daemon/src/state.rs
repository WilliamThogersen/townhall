use townhall_core::store::Store;
use townhall_core::{Context, ContextOptions, Result, Workspace};

/// Shared state for every handler: the core `Context` (startup repo, store with the repo
/// registry, event bus, base URL, claude binary) built once per daemon.
#[derive(Debug, Clone)]
pub struct AppState {
    ctx: Context,
}

impl AppState {
    /// Opens the store in the workspace's home. Base URL `http://127.0.0.1:7777`, claude from
    /// `TOWNHALL_CLAUDE_BIN` or `claude`. A store that cannot be opened is reported per request.
    pub fn new(workspace: Workspace) -> Self {
        Self::with_options(workspace, ContextOptions::default())
    }

    pub fn with_options(workspace: Workspace, options: ContextOptions) -> Self {
        Self {
            ctx: Context::new(workspace, options),
        }
    }

    /// Like `with_options`, but a store that cannot be opened (another townhall holds it,
    /// or it is from a newer townhall) is an error. The daemon's `main` uses this.
    pub fn open(workspace: Workspace, options: ContextOptions) -> Result<Self> {
        Context::open(workspace, options).map(|ctx| Self { ctx })
    }

    /// Uses a store opened earlier: the desktop app keeps one across daemon restarts.
    pub fn with_store(workspace: Workspace, options: ContextOptions, store: Store) -> Self {
        Self {
            ctx: Context::with_store(workspace, options, store),
        }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    /// The startup repo's workspace.
    pub fn workspace(&self) -> &Workspace {
        self.ctx.startup()
    }
}
