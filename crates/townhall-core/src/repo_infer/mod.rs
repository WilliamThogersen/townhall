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
/// gone, is not a git repo or does not answer in time is skipped; naming one
/// asks instead of guessing. Names must be unique, ignoring case, and come
/// back trimmed.
pub async fn infer(message: &str, candidates: &[Candidate]) -> Result<Inference> {
    validate(candidates)?;
    let reachable = reachable(candidates).await?;
    let usable: Vec<Candidate> = candidates
        .iter()
        .zip(&reachable)
        .filter(|(_, ok)| **ok)
        .map(|(c, _)| c.clone())
        .collect();
    if usable.is_empty() {
        return Ok(Inference::NoRepos);
    }
    let names: Vec<String> = usable.iter().map(|c| c.name.trim().to_string()).collect();
    let named = named_in(message, candidates).await?;
    if let Some(inference) = by_name(&named, candidates, &reachable, &names) {
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

/// Whether each candidate is a git work tree that answers in time.
async fn reachable(candidates: &[Candidate]) -> Result<Vec<bool>> {
    let checks = candidates
        .iter()
        .map(|c| {
            let path = c.path.clone();
            async move { git::is_work_tree(&path, Instant::now() + CHECK_TIMEOUT).await }
        })
        .collect();
    parallel::join_all(checks).await
}

fn found(repo: &str, rule: Rule, reason: &str) -> Inference {
    Inference::Found {
        repo: repo.to_string(),
        rule,
        reason: reason.to_string(),
    }
}

/// Indices of every candidate the message names, usable or not.
async fn named_in(message: &str, candidates: &[Candidate]) -> Result<Vec<usize>> {
    let message = message.to_string();
    let candidates = candidates.to_vec();
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    parallel::blocking(move || named::named(&message, &candidates, home.as_deref())).await
}

/// Naming a skipped repo asks among the usable ones; else the named usable repo
/// wins, and naming several asks among them.
fn by_name(
    named: &[usize],
    candidates: &[Candidate],
    reachable: &[bool],
    names: &[String],
) -> Option<Inference> {
    let name_of = |i: usize| candidates.get(i).map(|c| c.name.trim().to_string());
    let (ok, lost): (Vec<usize>, Vec<usize>) = named
        .iter()
        .copied()
        .partition(|&i| reachable.get(i) == Some(&true));
    if !lost.is_empty() {
        let lost: Vec<String> = lost.into_iter().filter_map(name_of).collect();
        return Some(Inference::Ask {
            candidates: names.to_vec(),
            reason: copy::unreachable(&lost),
        });
    }
    let picked: Vec<String> = ok.into_iter().filter_map(name_of).collect();
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
