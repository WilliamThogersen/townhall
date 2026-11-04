//! The code rule's decision: score repos by their hints, tracked paths first.

use std::cmp::Reverse;

use super::{copy, Inference, Rule};

/// How a repo has a hint. A tracked path weighs more than a mention in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HitKind {
    Content,
    Path,
}

/// One hint found in one repo: how, and the text for the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    pub kind: HitKind,
    pub what: String,
}

/// What one repo has for each hint, in hint order.
pub(crate) type Hits = Vec<Option<Hit>>;

/// `None` when no repo has any hint. Else the repo with the most path hits,
/// then the most content hits, wins; repos with the same score are asked about.
pub(crate) fn decide(names: &[String], hints: &[String], hits: &[Hits]) -> Option<Inference> {
    let scores: Vec<(usize, usize)> = hits.iter().map(|h| score(h)).collect();
    let best = scores
        .iter()
        .copied()
        .max()
        .filter(|&best| best != (0, 0))?;
    let top: Vec<usize> = (0..scores.len()).filter(|&r| scores[r] == best).collect();
    match top.as_slice() {
        [winner] => Some(Inference::Found {
            repo: names.get(*winner)?.clone(),
            rule: Rule::Code,
            reason: copy::has(reason_hit(*winner, hits)?),
        }),
        _ => Some(ask(names, hints, hits, &top)),
    }
}

fn score(hits: &[Option<Hit>]) -> (usize, usize) {
    let count = |kind: HitKind| hits.iter().flatten().filter(|h| h.kind == kind).count();
    (count(HitKind::Path), count(HitKind::Content))
}

fn kind_of(hits: &[Hits], repo: usize, hint: usize) -> Option<HitKind> {
    hits.get(repo)?.get(hint)?.as_ref().map(|h| h.kind)
}

/// The hit that beats every other repo's hit for the same hint, a path hit
/// before a content hit, earlier hints first.
fn reason_hit(winner: usize, hits: &[Hits]) -> Option<&str> {
    let mine = hits.get(winner)?;
    let beats = |hint: usize| {
        let own = kind_of(hits, winner, hint);
        (0..hits.len()).all(|r| r == winner || kind_of(hits, r, hint) < own)
    };
    mine.iter()
        .enumerate()
        .filter_map(|(hint, hit)| hit.as_ref().map(|hit| (hint, hit)))
        .max_by_key(|(hint, hit)| (beats(*hint), hit.kind, Reverse(*hint)))
        .map(|(_, hit)| hit.what.as_str())
}

fn ask(names: &[String], hints: &[String], hits: &[Hits], top: &[usize]) -> Inference {
    let shared =
        (0..hints.len()).find(|&hint| top.iter().all(|&r| kind_of(hits, r, hint).is_some()));
    let reason = shared.map_or_else(
        || copy::could_fit(top.len()),
        |hint| copy::share(top.len(), &hints[hint]),
    );
    Inference::Ask {
        candidates: top.iter().filter_map(|&r| names.get(r).cloned()).collect(),
        reason,
    }
}

#[cfg(test)]
#[path = "score_tests.rs"]
mod tests;
