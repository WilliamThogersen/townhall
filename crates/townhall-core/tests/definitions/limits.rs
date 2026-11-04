use std::thread;

use townhall_core::definitions::{parse_workflow, start_order, validate_workflow, MAX_STEPS};

use crate::support::KNOWN;

/// The verifier's repro: `s{i}` waits for `s{i+1}`, so every step is written before
/// the step it waits for.
fn rchain(size: usize) -> String {
    let steps: String = (0..size)
        .map(|i| {
            let after = match i + 1 < size {
                true => format!("after = [\"s{}\"]\n", i + 1),
                false => String::new(),
            };
            format!("[[step]]\nid = \"s{i}\"\nagent = \"coder\"\n{after}\n")
        })
        .collect();
    format!("name = \"w\"\n\n{steps}")
}

/// Errors as text and whether a start order exists, on a 2 MiB stack like a test
/// or tokio worker thread. A stack overflow would abort the whole test binary.
fn check_on_small_stack(size: usize) -> (Vec<String>, Option<usize>) {
    thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let def = parse_workflow(&rchain(size)).unwrap().def;
            let errors = match validate_workflow(&def, &KNOWN) {
                Ok(()) => Vec::new(),
                Err(errors) => errors.iter().map(ToString::to_string).collect(),
            };
            (errors, start_order(&def).map(|waves| waves.len()))
        })
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn a_workflow_has_at_most_200_steps() {
    assert_eq!(MAX_STEPS, 200);
    assert_eq!(check_on_small_stack(200), (Vec::new(), Some(200)));
    assert_eq!(
        check_on_small_stack(201),
        (
            vec!["error: workflow \"w\": step count 201 is over the limit of 200".to_string()],
            None
        )
    );
}

#[test]
fn a_huge_child_first_chain_gets_the_cap_error_not_an_abort() {
    assert_eq!(
        check_on_small_stack(15_000),
        (
            vec!["error: workflow \"w\": step count 15000 is over the limit of 200".to_string()],
            None
        )
    );
}
