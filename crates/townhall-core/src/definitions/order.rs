use super::graph::Graph;
use super::workflow::{Step, WorkflowDef};

/// The steps in waves: every step in a wave can start together, once the waves
/// before it have finished. Steps keep their written order inside a wave.
///
/// Every valid workflow has one. `None` means the workflow has a cycle, a
/// duplicate id or an unknown `after` id, which `validate_workflow` reports.
pub fn start_order(def: &WorkflowDef) -> Option<Vec<Vec<&Step>>> {
    let waves = Graph::new(&def.steps).waves()?;
    Some(
        waves
            .into_iter()
            .map(|wave| wave.into_iter().map(|i| &def.steps[i]).collect())
            .collect(),
    )
}
