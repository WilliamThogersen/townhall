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
        let mut found = Vec::new();
        for start in 0..self.deps.len() {
            if marks[start] == Mark::New {
                self.walk(start, &mut marks, &mut found);
            }
        }
        let canonical: Vec<Vec<usize>> = found.into_iter().map(rotate_to_lowest).collect();
        unique(canonical)
    }

    /// Depth first from `start` on an explicit stack of `(step, next dep to try)`,
    /// so a long chain of steps cannot overflow the thread's stack.
    fn walk(&self, start: usize, marks: &mut [Mark], found: &mut Vec<Vec<usize>>) {
        marks[start] = Mark::Open;
        let mut path: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(&(node, edge)) = path.last() {
            let Some(&next) = self.deps[node].get(edge) else {
                marks[node] = Mark::Done;
                path.pop();
                continue;
            };
            if let Some(top) = path.last_mut() {
                top.1 += 1;
            }
            match marks[next] {
                Mark::New => {
                    marks[next] = Mark::Open;
                    path.push((next, 0));
                }
                Mark::Open => {
                    let at = path.iter().position(|&(n, _)| n == next).unwrap_or(0);
                    found.push(path[at..].iter().map(|&(n, _)| n).collect());
                }
                Mark::Done => {}
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `s{i}` waits for `s{i+1}`; with `close`, the last step waits for `s0`.
    fn chain(size: usize, close: bool) -> Vec<Step> {
        (0..size)
            .map(|i| Step {
                id: format!("s{i}"),
                agent: "coder".into(),
                after: match (i + 1 < size, close) {
                    (true, _) => vec![format!("s{}", i + 1)],
                    (false, true) => vec!["s0".into()],
                    (false, false) => Vec::new(),
                },
                from: None,
                prompt: None,
            })
            .collect()
    }

    fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn walks_a_deep_chain_without_recursion() {
        let found = on_small_stack(|| Graph::new(&chain(100_000, false)).cycles().len());
        assert_eq!(found, 0);
    }

    #[test]
    fn finds_a_deep_cycle_without_recursion() {
        let cycles = on_small_stack(|| Graph::new(&chain(100_000, true)).cycles());
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 100_000);
        assert_eq!(cycles[0][..3], [0, 1, 2]);
    }
}
