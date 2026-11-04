use std::sync::Arc;

use townhall_core::Workspace;

/// Immutable shared state: the workspace (home plus repo). Everything else is rediscovered per request.
#[derive(Debug, Clone)]
pub struct AppState {
    inner: Arc<Workspace>,
}

impl AppState {
    pub fn new(workspace: Workspace) -> Self {
        Self {
            inner: Arc::new(workspace),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        &self.inner
    }
}
