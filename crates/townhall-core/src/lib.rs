//! townhall-core: worktree and tmux primitives for running many agents.

pub mod agent;
pub mod cmd;
pub mod error;
pub mod home;
pub mod stats;
pub mod steps;
pub mod tmux;
pub mod worktree;

pub use error::{Error, Result};
pub use home::{Home, Workspace};
