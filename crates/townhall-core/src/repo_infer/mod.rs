//! Repo inference: which known repo a message is about. The rules, in order:
//! the message names it, its code has what the message describes, it is the
//! only repo. Otherwise townhall asks.

mod copy;
mod git;
mod hints;
mod named;
mod parallel;
mod score;
mod search;
mod types;

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use tokio::time::Instant;

pub use types::{Candidate, Inference, Rule};

use crate::{Error, Result};

const CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Infer the repo for `message` among `candidates`. A candidate whose path is
/// gone or is not a git repo is skipped. Names must be unique, ignoring case.
pub async fn infer(message: &str, candidates: &[Candidate]) -> Result<Inference> {
    validate(candidates)?;
    let usable = usable(candidates).await?;
    if usable.is_empty() {
        return Ok(Inference::NoRepos);
    }
    let names: Vec<String> = usable.iter().map(|c| c.name.clone()).collect();
    if let Some(inference) = by_name(message, &usable, &names) {
        return Ok(inference);
    }
    if let Some(inference) = by_code(message, &usable, &names).await? {
        return Ok(inference);
    }
    Ok(fallback(&names))
}

/// The `~` event line for a found repo; `None` when townhall has to ask.
pub fn event_text(inference: &Inference, base_branch: &str) -> Option<String> {
    copy::event_text(inference, base_branch)
}

fn validate(candidates: &[Candidate]) -> Result<()> {
    let mut seen = HashSet::new();
    candidates.iter().try_for_each(|c| {
        let key = c.name.trim().to_lowercase();
        if key.is_empty() {
            return Err(Error::InvalidInput("a repo has an empty name".to_string()));
        }
        if !seen.insert(key) {
            let name = c.name.trim();
            return Err(Error::InvalidInput(format!("two repos are named {name}")));
        }
        Ok(())
    })
}

async fn usable(candidates: &[Candidate]) -> Result<Vec<Candidate>> {
    let checks = candidates
        .iter()
        .map(|c| {
            let path = c.path.clone();
            async move { git::is_work_tree(&path, Instant::now() + CHECK_TIMEOUT).await }
        })
        .collect();
    let ok = parallel::join_all(checks).await?;
    Ok(candidates
        .iter()
        .zip(ok)
        .filter(|(_, ok)| *ok)
        .map(|(c, _)| c.clone())
        .collect())
}

fn found(repo: &str, rule: Rule, reason: &str) -> Inference {
    Inference::Found {
        repo: repo.to_string(),
        rule,
        reason: reason.to_string(),
    }
}

fn by_name(message: &str, usable: &[Candidate], names: &[String]) -> Option<Inference> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    let picked: Vec<String> = named::named(message, usable, home.as_deref())
        .into_iter()
        .filter_map(|i| names.get(i).cloned())
        .collect();
    match picked.as_slice() {
        [] => None,
        [repo] => Some(found(repo, Rule::Named, copy::NAMED)),
        several => Some(Inference::Ask {
            reason: copy::named_several(several.len()),
            candidates: picked,
        }),
    }
}

async fn by_code(
    message: &str,
    usable: &[Candidate],
    names: &[String],
) -> Result<Option<Inference>> {
    let hints = hints::extract(message);
    if hints.is_empty() {
        return Ok(None);
    }
    let hits = search::search_all(usable, &hints).await?;
    let texts: Vec<String> = hints.into_iter().map(|h| h.text).collect();
    Ok(score::decide(names, &texts, &hits))
}

fn fallback(names: &[String]) -> Inference {
    match names {
        [only] => found(only, Rule::Only, copy::ONLY),
        _ => Inference::Ask {
            candidates: names.to_vec(),
            reason: copy::could_fit(names.len()),
        },
    }
}
