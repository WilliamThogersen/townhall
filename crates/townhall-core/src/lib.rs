//! townhall-core: worktrees, tmux sessions, the repo registry, conversation storage
//! and the API contract types shared by the daemon and the desktop app.

pub mod agent;
pub mod agent_usage;
pub mod clock;
pub mod cmd;
pub mod context;
pub mod definitions;
pub mod disk;
pub mod error;
pub mod events;
pub mod files;
pub mod fleet;
pub mod fsutil;
pub mod home;
pub mod ids;
pub mod insights;
pub mod logs;
pub mod mcp;
pub mod model;
pub mod plugins;
pub mod reaper;
pub mod redact;
pub mod repo_infer;
pub mod repos;
pub mod runner;
pub mod scheduler;
pub mod search;
pub mod stats;
pub mod steps;
pub mod store;
pub mod tmux;
pub mod usage;
pub mod version;
pub mod worktree;

pub use context::{Context, ContextOptions};
pub use error::{Error, Result};
pub use home::{Home, Workspace};
