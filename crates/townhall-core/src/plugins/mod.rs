//! Plugins and marketplaces (`docs/plugins.md`). Parsing and validation only: every function
//! here works on the text it is given and touches no file, process or network.

pub(crate) mod agent_types;
mod check;
pub mod declared;
pub mod hooks;
pub mod install;
pub mod manifest;
pub mod market;
pub mod marketplace;
mod official;
pub mod prompts;
pub mod schema;
mod sha256;
pub mod storage;
pub mod theme;
pub mod tokens;
pub mod tree;

pub use official::{Official, OFFICIAL};
