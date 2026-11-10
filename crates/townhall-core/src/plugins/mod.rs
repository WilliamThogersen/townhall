//! Plugins and marketplaces (`docs/plugins.md`). Parsing and validation only: every function
//! here works on the text it is given and touches no file, process or network.

pub(crate) mod agent_types;
mod check;
pub mod install;
pub mod manifest;
pub mod market;
pub mod marketplace;
mod official;
mod sha256;
pub mod tree;

pub use official::{Official, OFFICIAL};
