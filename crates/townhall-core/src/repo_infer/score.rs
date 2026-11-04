//! The code rule's decision: score repos by how many distinct hints they have.

use super::{copy, Inference, Rule};

/// What one repo has for each hint, in hint order: the concrete hit or nothing.
pub(crate) type Hits = Vec<Option<String>>;

/// `None` when no repo has any hint; else the clear best, or an ask among the tied.
pub(crate) fn decide(names: &[String], hints: &[String], hits: &[Hits]) -> Option<Inference> {
    let scores: Vec<usize> = hits.iter().map(|h| h.iter().flatten().count()).collect();
    let best = scores.iter().copied().max().filter(|&best| best > 0)?;
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

fn has(hits: &[Hits], repo: usize, hint: usize) -> bool {
    hits.get(repo)
        .and_then(|h| h.get(hint))
        .is_some_and(Option::is_some)
}

/// The first hit no other repo has, else the first hit.
fn reason_hit(winner: usize, hits: &[Hits]) -> Option<&str> {
    let mine = hits.get(winner)?;
    let alone = |hint: usize| (0..hits.len()).all(|r| r == winner || !has(hits, r, hint));
    let first = |distinct: bool| {
        mine.iter()
            .enumerate()
            .find(|(hint, hit)| hit.is_some() && (!distinct || alone(*hint)))
            .and_then(|(_, hit)| hit.as_deref())
    };
    first(true).or_else(|| first(false))
}

fn ask(names: &[String], hints: &[String], hits: &[Hits], top: &[usize]) -> Inference {
    let shared = (0..hints.len()).find(|&hint| top.iter().all(|&r| has(hits, r, hint)));
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
