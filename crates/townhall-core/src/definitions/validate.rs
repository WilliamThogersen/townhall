use std::collections::HashSet;

use super::error::{into_result, invalid_name, DefKind, DefinitionError};
use super::graph::Graph;
use super::names::MAX_STEP_ID_LEN;
use super::vars::{tokens, Token, Var};
use super::workflow::{Step, WorkflowDef, MAX_STEPS};

/// Check a workflow against every rule and return all errors found, workflow
/// level first, then each step in written order. Over `MAX_STEPS` steps, only
/// the workflow level is checked.
pub fn validate_workflow(
    def: &WorkflowDef,
    known_agents: &[&str],
) -> Result<(), Vec<DefinitionError>> {
    if def.steps.len() > MAX_STEPS {
        return into_result(workflow_errors(def));
    }
    let graph = Graph::new(&def.steps);
    let cycles = graph.cycles();
    let steps = (0..def.steps.len()).flat_map(|index| {
        StepCheck {
            def,
            graph: &graph,
            known_agents,
            index,
            ancestors: graph.ancestors(index),
        }
        .errors(&cycles)
    });
    into_result(workflow_errors(def).into_iter().chain(steps).collect())
}

fn workflow_errors(def: &WorkflowDef) -> Vec<DefinitionError> {
    let error = |field: &str, message: String| {
        DefinitionError::field(DefKind::Workflow, &def.name, field, message)
    };
    let name = invalid_name(&def.name).map(|m| error("name", m));
    let count = def.steps.len();
    let steps = match count {
        0 => Some("is missing: add at least one [[step]]".to_string()),
        n if n > MAX_STEPS => Some(format!("count {n} is over the limit of {MAX_STEPS}")),
        _ => None,
    };
    name.into_iter()
        .chain(steps.map(|m| error("step", m)))
        .collect()
}

struct StepCheck<'a> {
    def: &'a WorkflowDef,
    graph: &'a Graph<'a>,
    known_agents: &'a [&'a str],
    index: usize,
    ancestors: HashSet<usize>,
}

impl StepCheck<'_> {
    fn step(&self) -> &Step {
        &self.def.steps[self.index]
    }

    fn error(&self, field: &str, message: String) -> DefinitionError {
        DefinitionError::field(DefKind::Workflow, &self.def.name, field, message)
            .at_step(&self.step().id)
    }

    fn errors(&self, cycles: &[Vec<usize>]) -> Vec<DefinitionError> {
        [
            self.check_id(),
            self.check_agent(),
            self.check_after(),
            self.check_cycles(cycles),
            self.check_from(),
            self.check_prompt(),
        ]
        .concat()
    }

    fn check_id(&self) -> Vec<DefinitionError> {
        let id = &self.step().id;
        let invalid = invalid_name(id);
        let long = (id.len() > MAX_STEP_ID_LEN)
            .then(|| format!("is longer than {MAX_STEP_ID_LEN} characters"));
        let duplicate = self.def.steps[..self.index]
            .iter()
            .find(|s| s.id.eq_ignore_ascii_case(id))
            .map(|s| match &s.id == id {
                true => format!("\"{id}\" is used by more than one step"),
                false => format!(
                    "\"{id}\" clashes with \"{}\": ids must differ by more than case",
                    s.id
                ),
            });
        invalid
            .into_iter()
            .chain(long)
            .chain(duplicate)
            .map(|m| self.error("id", m))
            .collect()
    }

    fn check_agent(&self) -> Vec<DefinitionError> {
        let agent = &self.step().agent;
        let saved = self.known_agents.contains(&agent.as_str());
        (!saved)
            .then(|| self.error("agent", format!("\"{agent}\" is not a saved agent")))
            .into_iter()
            .collect()
    }

    fn check_after(&self) -> Vec<DefinitionError> {
        self.step()
            .after
            .iter()
            .filter(|id| self.graph.find(id).is_none())
            .map(|id| self.error("after", format!("\"{id}\" is not a step")))
            .collect()
    }

    fn check_cycles(&self, cycles: &[Vec<usize>]) -> Vec<DefinitionError> {
        cycles
            .iter()
            .filter(|cycle| cycle.first() == Some(&self.index))
            .map(|cycle| self.error("after", format!("makes a cycle: {}", self.describe(cycle))))
            .collect()
    }

    fn describe(&self, cycle: &[usize]) -> String {
        let id = |i: usize| self.def.steps[cycle[i % cycle.len()]].id.as_str();
        (0..cycle.len())
            .map(|i| format!("{} waits for {}", id(i), id(i + 1)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn check_from(&self) -> Vec<DefinitionError> {
        let Some(from) = &self.step().from else {
            return Vec::new();
        };
        let message = match self.graph.find(from) {
            None => format!("\"{from}\" is not a step"),
            Some(j) if !self.ancestors.contains(&j) => {
                format!("\"{from}\" is not a step this step waits for")
            }
            Some(_) => return Vec::new(),
        };
        vec![self.error("from", message)]
    }

    fn check_prompt(&self) -> Vec<DefinitionError> {
        let prompt = self.step().prompt.as_deref().unwrap_or_default();
        tokens(prompt)
            .into_iter()
            .filter_map(|token| match token {
                Token::Text(_) => None,
                Token::Bad(problem) => Some(problem.detail()),
                Token::Var(var) => self.var_problem(&var),
            })
            .map(|m| self.error("prompt", m))
            .collect()
    }

    /// A `steps.<id>` variable must name a step this one waits for.
    fn var_problem(&self, var: &Var) -> Option<String> {
        let id = var.step()?;
        match self.graph.find(id) {
            None => Some(format!(
                "variable \"{var}\" names \"{id}\", which is not a step"
            )),
            Some(j) if !self.ancestors.contains(&j) => Some(format!(
                "variable \"{var}\" names \"{id}\", which does not run before this step"
            )),
            Some(_) => None,
        }
    }
}
