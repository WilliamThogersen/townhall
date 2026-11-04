use std::collections::{HashMap, HashSet};

use super::workflow::Step;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Open,
    Done,
}

/// The `after` edges of a workflow, by step index. `deps[i]` are the steps `i` waits for.
pub(crate) struct Graph<'a> {
    index: HashMap<&'a str, usize>,
    deps: Vec<Vec<usize>>,
    /// Every id is unique and every `after` id is a step.
    complete: bool,
}

impl<'a> Graph<'a> {
    pub(crate) fn new(steps: &'a [Step]) -> Self {
        let index = steps
            .iter()
            .enumerate()
            .rev()
            .map(|(i, step)| (step.id.as_str(), i))
            .collect::<HashMap<_, _>>();
        let find = |id: &String| index.get(id.as_str()).copied();
        let deps: Vec<Vec<usize>> = steps
            .iter()
            .map(|step| unique(step.after.iter().filter_map(find)))
            .collect();
        let known = steps
            .iter()
            .all(|step| step.after.iter().all(|a| find(a).is_some()));
        let complete = known && index.len() == steps.len();
        Self {
            index,
            deps,
            complete,
        }
    }

    /// The index of the first step with this id.
    pub(crate) fn find(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// Every step `step` waits for, directly or through other steps. Never `step` itself.
    pub(crate) fn ancestors(&self, step: usize) -> HashSet<usize> {
        let mut seen = HashSet::new();
        let mut todo = self.deps[step].clone();
        while let Some(next) = todo.pop() {
            if seen.insert(next) {
                todo.extend(&self.deps[next]);
            }
        }
        seen.remove(&step);
        seen
    }

    /// One cycle per back edge, each as `[a, b, ..]` where a waits for b and the last
    /// waits for a. Each starts at its lowest index, so the first step written leads.
    pub(crate) fn cycles(&self) -> Vec<Vec<usize>> {
        let mut marks = vec![Mark::New; self.deps.len()];
        let mut path = Vec::new();
        let mut found = Vec::new();
        for start in 0..self.deps.len() {
            if marks[start] == Mark::New {
                self.visit(start, &mut marks, &mut path, &mut found);
            }
        }
        let canonical: Vec<Vec<usize>> = found.into_iter().map(rotate_to_lowest).collect();
        unique(canonical)
    }

    fn visit(
        &self,
        node: usize,
        marks: &mut [Mark],
        path: &mut Vec<usize>,
        found: &mut Vec<Vec<usize>>,
    ) {
        marks[node] = Mark::Open;
        path.push(node);
        for &next in &self.deps[node] {
            match marks[next] {
                Mark::New => self.visit(next, marks, path, found),
                Mark::Open => {
                    let at = path.iter().position(|&n| n == next).unwrap_or(0);
                    found.push(path[at..].to_vec());
                }
                Mark::Done => {}
            }
        }
        path.pop();
        marks[node] = Mark::Done;
    }

    /// Steps in waves: each wave starts once every earlier wave it waits on has finished.
    /// `None` when the graph has a cycle, a duplicate id or an unknown `after` id.
    pub(crate) fn waves(&self) -> Option<Vec<Vec<usize>>> {
        if !self.complete {
            return None;
        }
        let count = self.deps.len();
        let mut done = vec![false; count];
        let mut waves: Vec<Vec<usize>> = Vec::new();
        while waves.iter().map(Vec::len).sum::<usize>() < count {
            let wave: Vec<usize> = (0..count)
                .filter(|&i| !done[i] && self.deps[i].iter().all(|&d| done[d]))
                .collect();
            if wave.is_empty() {
                return None;
            }
            wave.iter().for_each(|&i| done[i] = true);
            waves.push(wave);
        }
        Some(waves)
    }
}

fn unique<T: PartialEq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    items.into_iter().fold(Vec::new(), |mut kept, item| {
        if !kept.contains(&item) {
            kept.push(item);
        }
        kept
    })
}

fn rotate_to_lowest(cycle: Vec<usize>) -> Vec<usize> {
    let lowest = (0..cycle.len()).min_by_key(|&i| cycle[i]).unwrap_or(0);
    cycle[lowest..]
        .iter()
        .chain(&cycle[..lowest])
        .copied()
        .collect()
}
