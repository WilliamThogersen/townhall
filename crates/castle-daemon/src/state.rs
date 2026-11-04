use std::path::PathBuf;
use std::sync::Arc;

/// Immutable shared state: only the repo path. Everything else is rediscovered per request.
#[derive(Debug, Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    repo: PathBuf,
}

impl AppState {
    pub fn new(repo: PathBuf) -> Self {
        Self { inner: Arc::new(Inner { repo }) }
    }

    pub fn repo(&self) -> &std::path::Path {
        &self.inner.repo
    }
}
