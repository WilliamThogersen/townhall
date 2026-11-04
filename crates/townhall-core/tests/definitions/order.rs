use townhall_core::definitions::{parse_workflow, start_order, validate_workflow, WorkflowDef};

use crate::support::{workflow, KNOWN, SHIP};

fn def(steps: &[(&str, &[&str], &str)]) -> WorkflowDef {
    parse_workflow(&workflow(steps)).unwrap().def
}

fn waves(def: &WorkflowDef) -> Option<Vec<Vec<&str>>> {
    start_order(def).map(|waves| {
        waves
            .iter()
            .map(|wave| wave.iter().map(|s| s.id.as_str()).collect())
            .collect()
    })
}

#[test]
fn ship_plans_then_builds_then_tests_and_reviews_together() {
    let ship = parse_workflow(SHIP).unwrap().def;
    assert_eq!(
        waves(&ship).unwrap(),
        [vec!["plan"], vec!["build"], vec!["test", "review"]]
    );
}

#[test]
fn a_single_step_is_one_wave() {
    assert_eq!(waves(&def(&[("a", &[], "")])).unwrap(), [vec!["a"]]);
}

#[test]
fn fan_out_starts_the_children_together() {
    let d = def(&[
        ("a", &[], ""),
        ("b", &["a"], ""),
        ("c", &["a"], ""),
        ("d", &["a"], ""),
    ]);
    assert_eq!(waves(&d).unwrap(), [vec!["a"], vec!["b", "c", "d"]]);
}

#[test]
fn fan_in_waits_for_every_parent() {
    let d = def(&[
        ("a", &[], ""),
        ("b", &[], ""),
        ("c", &[], ""),
        ("d", &["a", "b", "c"], ""),
    ]);
    assert_eq!(waves(&d).unwrap(), [vec!["a", "b", "c"], vec!["d"]]);
}

#[test]
fn diamond_meets_at_the_end() {
    let d = def(&[
        ("a", &[], ""),
        ("b", &["a"], ""),
        ("c", &["a"], ""),
        ("d", &["b", "c"], ""),
    ]);
    assert_eq!(waves(&d).unwrap(), [vec!["a"], vec!["b", "c"], vec!["d"]]);
}

#[test]
fn uneven_fan_in_waits_for_the_longest_branch() {
    let d = def(&[
        ("a", &[], ""),
        ("b", &["a"], ""),
        ("c", &[], ""),
        ("d", &["b", "c"], ""),
    ]);
    assert_eq!(waves(&d).unwrap(), [vec!["a", "c"], vec!["b"], vec!["d"]]);
}

#[test]
fn steps_listed_before_their_parents_still_order() {
    let d = def(&[("d", &["b"], ""), ("b", &["a"], ""), ("a", &[], "")]);
    assert_eq!(waves(&d).unwrap(), [vec!["a"], vec!["b"], vec!["d"]]);
}

#[test]
fn invalid_graphs_have_no_order() {
    assert_eq!(waves(&def(&[("a", &["b"], ""), ("b", &["a"], "")])), None);
    assert_eq!(waves(&def(&[("a", &["z"], "")])), None);
    assert_eq!(waves(&def(&[("a", &[], ""), ("a", &[], "")])), None);
}

/// Deterministic pseudo random numbers, so the test needs no extra crate.
fn next(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

fn random_dag(seed: &mut u64, size: usize) -> String {
    let ids: Vec<String> = (0..size).map(|i| format!("s{i}")).collect();
    let steps: Vec<(String, Vec<String>)> = (0..size)
        .map(|i| {
            let after = (0..i)
                .filter(|_| next(seed).is_multiple_of(3))
                .map(|j| ids[j].clone())
                .collect();
            (ids[i].clone(), after)
        })
        .collect();
    let refs: Vec<(&str, Vec<&str>)> = steps
        .iter()
        .map(|(id, after)| (id.as_str(), after.iter().map(String::as_str).collect()))
        .collect();
    let rows: Vec<(&str, &[&str], &str)> = refs
        .iter()
        .rev()
        .map(|(id, a)| (*id, a.as_slice(), ""))
        .collect();
    workflow(&rows)
}

#[test]
fn every_valid_workflow_has_a_start_order() {
    let mut seed = 7;
    for round in 0..200 {
        let def = parse_workflow(&random_dag(&mut seed, 1 + round % 12))
            .unwrap()
            .def;
        assert_eq!(validate_workflow(&def, &KNOWN), Ok(()));
        let order = start_order(&def).expect("a valid workflow has an order");
        let position = |id: &str| {
            order
                .iter()
                .position(|w| w.iter().any(|s| s.id == id))
                .unwrap()
        };
        assert_eq!(order.iter().map(Vec::len).sum::<usize>(), def.steps.len());
        for step in &def.steps {
            let wave = position(&step.id);
            assert!(step.after.iter().all(|a| position(a) < wave));
            let latest = step
                .after
                .iter()
                .map(|a| position(a) + 1)
                .max()
                .unwrap_or(0);
            assert_eq!(wave, latest, "{} starts as soon as it can", step.id);
        }
    }
}
