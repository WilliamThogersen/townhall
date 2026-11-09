use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A known repo townhall may route work to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub name: String,
    pub path: PathBuf,
}

/// Which rule picked the repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rule {
    /// The message names the repo or its path.
    Named,
    /// The repo's code has what the message describes.
    Code,
    /// There is exactly one repo.
    Only,
}

/// The outcome of repo inference for one message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Inference {
    Found {
        repo: String,
        rule: Rule,
        reason: String,
    },
    Ask {
        candidates: Vec<String>,
        reason: String,
    },
    NoRepos,
    NoSignal,
}
