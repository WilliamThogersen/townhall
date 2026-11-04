//! townhall-core: worktree and tmux primitives for running many agents.

pub mod agent;
pub mod cmd;
pub mod error;
pub mod tmux;
pub mod worktree;

pub use error::{Error, Result};
