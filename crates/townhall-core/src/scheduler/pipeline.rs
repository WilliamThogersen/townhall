use std::collections::HashSet;

use crate::context::Context;
use crate::definitions::{parse_workflow, render, variables, Step, Values, Var, WorkflowDef};
use crate::model::{CreateTaskInput, EntryBody, Task, TaskState, TASK_BRANCH_PREFIX};
use crate::store::{
    Run, RunStep, StepUpdate, RUN_FAILED, RUN_FINISHED, RUN_NEEDS_YOU, RUN_RUNNING,
};
use crate::{cmd, runner, Error, Result};

use super::gate::report_of;
use super::launch::commit_of;
use super::{paths, plan, report};

const STARTING: &str = "starting";
const STARTED: &str = "started";
const DONE: &str = "done";
const FAILED: &str = "failed";
const SKIPPED: &str = "skipped";
const FIXING: &str = "fixing";
const REDONE: &str = "redone";

pub fn task_name(run: &str, step: &str, round: u32) -> String {
    match round {
        0 | 1 => format!("{run}-{step}"),
        n => format!("{run}-{step}-r{n}"),
    }
}

pub(super) async fn drive(ctx: &Context) -> Result<(Vec<String>, Vec<String>)> {
    let mut created = vec![];
    let mut warnings = vec![];
    for run in ctx.store()?.runs_in(RUN_RUNNING).await? {
        match advance(ctx, &run).await {
            Ok(tasks) => created.extend(tasks.into_iter().map(|t| t.name)),
            Err(e) => warnings.push(format!("run `{}`: {e}", run.id)),
        }
    }
    Ok((created, warnings))
}

pub(crate) async fn advance(ctx: &Context, run: &Run) -> Result<Vec<Task>> {
    let store = ctx.store()?;
    let def = match parse_workflow(&run.snapshot) {
        Ok(parsed) => parsed.def,
        Err(e) => {
            let why = format!("its saved definition cannot be read: {e}");
            end_run(ctx, run, RUN_FAILED, &why).await?;
            return Ok(vec![]);
        }
    };
    let mut made = vec![];
    for row in store.run_steps(&run.id).await? {
        if row.state != STARTING {
            continue;
        }
        let Some(step) = def.steps.iter().find(|s| s.id == row.step_id) else {
            let why = format!("step `{}` is not in the run's definition", row.step_id);
            end_run(ctx, run, RUN_FAILED, &why).await?;
            return Ok(made);
        };
        match begin(ctx, run, &def, step, row.round).await? {
            Began::Task(task) => made.push(*task),
            Began::Skipped => {}
            Began::Ended => return Ok(made),
        }
    }
    if !settle(ctx, run, &def).await? {
        return Ok(made);
    }
    if !next_round(ctx, run, &def, &mut made).await? {
        return Ok(made);
    }
    let mut skipped = true;
    while std::mem::take(&mut skipped) {
        let rows = store.run_steps(&run.id).await?;
        let done = finished(&rows);
        for step in &def.steps {
            let started = rows.iter().any(|r| r.step_id == step.id);
            if started || !step.after.iter().all(|a| done.contains(a.as_str())) {
                continue;
            }
            if !store.claim_run_step(&run.id, &step.id, 1).await? {
                continue;
            }
            match begin(ctx, run, &def, step, 1).await? {
                Began::Task(task) => made.push(*task),
                Began::Skipped => skipped = true,
                Began::Ended => return Ok(made),
            }
        }
    }
    let rows = store.run_steps(&run.id).await?;
    let done = finished(&rows);
    if def.steps.iter().all(|s| done.contains(s.id.as_str())) {
        end_run(ctx, run, RUN_FINISHED, "every step is done").await?;
    }
    Ok(made)
}

enum Began {
    Task(Box<Task>),
    Skipped,
    Ended,
}

fn newest_row<'a>(rows: &'a [RunStep], step: &str) -> Option<&'a RunStep> {
    rows.iter()
        .filter(|r| r.step_id == step)
        .max_by_key(|r| r.round)
}

fn finished(rows: &[RunStep]) -> HashSet<&str> {
    let steps: HashSet<&str> = rows.iter().map(|r| r.step_id.as_str()).collect();
    steps
        .into_iter()
        .filter(|id| newest_row(rows, id).is_some_and(|r| r.state == DONE || r.state == SKIPPED))
        .collect()
}

async fn begin(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    step: &Step,
    round: u32,
) -> Result<Began> {
    let store = ctx.store()?;
    let name = task_name(&run.id, &step.id, round);
    let patterns = step.when.as_ref().map(|w| &w.paths[..]).unwrap_or(&[]);
    if patterns.is_empty() || store.find_task(&name).await?.is_some() {
        return Ok(match ensure_task(ctx, run, def, step, round).await? {
            Some(task) => Began::Task(Box::new(task)),
            None => Began::Ended,
        });
    }
    let rows = store.run_steps(&run.id).await?;
    let files = match changed_files(ctx, run, def, &rows).await? {
        Ok(files) => files,
        Err(why) => {
            let why = format!("step `{}` cannot be checked: {why}", step.id);
            let update = StepUpdate {
                note: Some(&why),
                ..StepUpdate::state(FAILED)
            };
            store.set_run_step(&run.id, &step.id, round, update).await?;
            end_run(ctx, run, RUN_FAILED, &why).await?;
            return Ok(Began::Ended);
        }
    };
    if paths::any_match(patterns, &files) {
        return Ok(match ensure_task(ctx, run, def, step, round).await? {
            Some(task) => Began::Task(Box::new(task)),
            None => Began::Ended,
        });
    }
    let why = format!(
        "no changed file matches {} ({} changed)",
        patterns.join(", "),
        files.len()
    );
    let update = StepUpdate {
        note: Some(&why),
        ..StepUpdate::state(SKIPPED)
    };
    store.set_run_step(&run.id, &step.id, round, update).await?;
    let text = format!("run `{}`: step `{}` skipped: {why}", run.id, step.id);
    store
        .append_entry(&run.conversation_id, EntryBody::Event { text })
        .await?;
    Ok(Began::Skipped)
}

async fn changed_files(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    rows: &[RunStep],
) -> Result<std::result::Result<Vec<String>, String>> {
    let Some(work) = def.work.as_deref() else {
        return Ok(Err(
            "it has a `when` but the pipeline names no build step".into()
        ));
    };
    let Some(name) = newest_task(rows, work) else {
        return Ok(Err(format!("build step `{work}` has no task")));
    };
    let Some(launch) = ctx.store()?.task_launch(name).await? else {
        return Ok(Err(format!(
            "build task `{name}` has no recorded start commit"
        )));
    };
    let repo = ctx.repo(&run.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let range = format!("{}..{TASK_BRANCH_PREFIX}{name}", launch.start_commit);
    let args = ["diff", "--name-only", range.as_str(), "--"];
    let out = cmd::run("git", &args, Some(ws.repo())).await?;
    Ok(Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()))
}

async fn next_round(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    made: &mut Vec<Task>,
) -> Result<bool> {
    let store = ctx.store()?;
    let rows = store.run_steps(&run.id).await?;
    let Some(work) = def.work.as_deref() else {
        return Ok(true);
    };
    if newest_row(&rows, work).is_none_or(|r| r.state != DONE) {
        return Ok(true);
    }
    let Some(round) = rows
        .iter()
        .filter(|r| r.state == FIXING)
        .map(|r| r.round)
        .max()
    else {
        return Ok(true);
    };
    for step in &def.steps {
        let ran = rows
            .iter()
            .any(|r| r.step_id == step.id && r.round == round && r.state != SKIPPED);
        if !ran || report::vocabulary(&step.agent).is_none() {
            continue;
        }
        if !store.claim_run_step(&run.id, &step.id, round + 1).await? {
            continue;
        }
        match begin(ctx, run, def, step, round + 1).await? {
            Began::Task(task) => made.push(*task),
            Began::Skipped => {}
            Began::Ended => return Ok(false),
        }
    }
    for row in rows
        .iter()
        .filter(|r| r.state == FIXING && r.round == round)
    {
        let update = StepUpdate::state(REDONE);
        store
            .set_run_step(&run.id, &row.step_id, row.round, update)
            .await?;
    }
    Ok(true)
}

async fn settle(ctx: &Context, run: &Run, def: &WorkflowDef) -> Result<bool> {
    let store = ctx.store()?;
    let tasks = store.all_tasks().await?;
    for row in store.run_steps(&run.id).await? {
        if row.state != STARTED {
            continue;
        }
        let Some(name) = row.task.as_deref() else {
            continue;
        };
        let Some(task) = tasks.iter().find(|t| t.name == name) else {
            let why = format!("task `{name}` is gone");
            fail_step(ctx, run, &row, &why).await?;
            end_run(ctx, run, RUN_FAILED, &why).await?;
            return Ok(false);
        };
        if task.state == TaskState::Error {
            let why = match &task.blocked {
                Some(blocked) => format!("task `{name}` failed: {blocked}"),
                None => format!("task `{name}` failed"),
            };
            fail_step(ctx, run, &row, &why).await?;
            end_run(ctx, run, RUN_FAILED, &why).await?;
            return Ok(false);
        }
        if !plan::finished(task.state) {
            continue;
        }
        match verdict_problem(ctx, task).await? {
            Some(Problem::Stop(why)) => {
                fail_step(ctx, run, &row, &why).await?;
                end_run(ctx, run, RUN_NEEDS_YOU, &why).await?;
                return Ok(false);
            }
            Some(Problem::Rejected { why, report }) => {
                if !hand_back(ctx, run, def, &row, &why, &report).await? {
                    return Ok(false);
                }
                continue;
            }
            None => {}
        }
        let update = StepUpdate::state(DONE);
        store
            .set_run_step(&run.id, &row.step_id, row.round, update)
            .await?;
    }
    Ok(true)
}

async fn hand_back(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    row: &RunStep,
    why: &str,
    report: &str,
) -> Result<bool> {
    let store = ctx.store()?;
    let used = row.round.saturating_sub(1);
    let rows = store.run_steps(&run.id).await?;
    let build = def
        .work
        .as_deref()
        .and_then(|work| newest_row(&rows, work))
        .and_then(|r| r.task.as_deref().map(|task| (r, task)));
    let Some((build_row, build_task)) = build.filter(|_| used < def.max_fix_rounds) else {
        let why = match def.work {
            Some(_) if used >= def.max_fix_rounds => {
                format!("{why}, and no fix rounds are left")
            }
            _ => format!("{why}, and the pipeline has no build task to hand back to"),
        };
        fail_step(ctx, run, row, &why).await?;
        end_run(ctx, run, RUN_NEEDS_YOU, &why).await?;
        return Ok(false);
    };
    let tasks = store.all_tasks().await?;
    let Some(task) = tasks.iter().find(|t| t.name == build_task) else {
        let why = format!("{why}, and build task `{build_task}` is gone");
        fail_step(ctx, run, row, &why).await?;
        end_run(ctx, run, RUN_FAILED, &why).await?;
        return Ok(false);
    };
    if plan::finished(task.state) {
        let reason = format!("{why}. Its report:\n{report}");
        match store.hand_on(build_task, &task.agent, &reason, "").await {
            Ok(_) => {}
            Err(Error::Conflict(said)) => {
                let why = format!("{why}, and `{build_task}` cannot be handed back: {said}");
                fail_step(ctx, run, row, &why).await?;
                end_run(ctx, run, RUN_NEEDS_YOU, &why).await?;
                return Ok(false);
            }
            Err(e) => return Err(e),
        }
    }
    let update = StepUpdate {
        note: Some(why),
        ..StepUpdate::state(FIXING)
    };
    store
        .set_run_step(&run.id, &row.step_id, row.round, update)
        .await?;
    let update = StepUpdate::state(STARTED);
    store
        .set_run_step(&run.id, &build_row.step_id, build_row.round, update)
        .await?;
    Ok(true)
}

async fn fail_step(ctx: &Context, run: &Run, row: &RunStep, why: &str) -> Result<()> {
    let update = StepUpdate {
        note: Some(why),
        ..StepUpdate::state(FAILED)
    };
    ctx.store()?
        .set_run_step(&run.id, &row.step_id, row.round, update)
        .await
}

async fn end_run(ctx: &Context, run: &Run, to: &str, why: &str) -> Result<()> {
    if !ctx.store()?.move_run(&run.id, RUN_RUNNING, to).await? {
        return Ok(());
    }
    let text = match to {
        RUN_FINISHED => format!("run `{}` finished: {why}", run.id),
        RUN_NEEDS_YOU => format!("run `{}` needs you: {why}", run.id),
        _ => format!("run `{}` failed: {why}", run.id),
    };
    runner::notify(ctx, &run.conversation_id, text);
    Ok(())
}

async fn ensure_task(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    step: &Step,
    round: u32,
) -> Result<Option<Task>> {
    let store = ctx.store()?;
    let name = task_name(&run.id, &step.id, round);
    let task = match store.find_task(&name).await? {
        Some(task) => task,
        None => match make_task(ctx, run, def, step, round).await? {
            Ok(task) => task,
            Err(why) => match store.find_task(&name).await? {
                Some(raced) => raced,
                None => {
                    let why = format!("step `{}` could not start: {why}", step.id);
                    let update = StepUpdate {
                        note: Some(&why),
                        ..StepUpdate::state(FAILED)
                    };
                    store.set_run_step(&run.id, &step.id, round, update).await?;
                    end_run(ctx, run, RUN_FAILED, &why).await?;
                    return Ok(None);
                }
            },
        },
    };
    if store
        .start_run_step(&run.id, &step.id, round, &task.name)
        .await?
    {
        let text = format!(
            "run `{}`: step `{}` started as task `{}`",
            run.id, step.id, task.name
        );
        store
            .append_entry(&run.conversation_id, EntryBody::Event { text })
            .await?;
    }
    Ok(Some(task))
}

fn newest_task<'a>(rows: &'a [RunStep], step: &str) -> Option<&'a str> {
    rows.iter()
        .filter(|r| r.step_id == step)
        .filter_map(|r| r.task.as_deref().map(|t| (r.round, t)))
        .max_by_key(|(round, _)| *round)
        .map(|(_, task)| task)
}

fn after_work(def: &WorkflowDef, step: &Step) -> bool {
    let Some(work) = def.work.as_deref() else {
        return false;
    };
    let mut seen = HashSet::new();
    let mut todo: Vec<&str> = step.after.iter().map(String::as_str).collect();
    while let Some(id) = todo.pop() {
        if id == work {
            return true;
        }
        if seen.insert(id) {
            if let Some(earlier) = def.steps.iter().find(|s| s.id == id) {
                todo.extend(earlier.after.iter().map(String::as_str));
            }
        }
    }
    false
}

async fn make_task(
    ctx: &Context,
    run: &Run,
    def: &WorkflowDef,
    step: &Step,
    round: u32,
) -> Result<std::result::Result<Task, String>> {
    let rows = ctx.store()?.run_steps(&run.id).await?;
    let mut after: Vec<String> = vec![];
    let mut todo: Vec<&str> = step.after.iter().rev().map(String::as_str).collect();
    let mut seen = HashSet::new();
    while let Some(earlier) = todo.pop() {
        if !seen.insert(earlier) {
            continue;
        }
        if newest_row(&rows, earlier).is_some_and(|r| r.state == SKIPPED) {
            if let Some(skipped) = def.steps.iter().find(|s| s.id == earlier) {
                todo.extend(skipped.after.iter().rev().map(String::as_str));
            }
            continue;
        }
        match newest_task(&rows, earlier) {
            Some(task) if after.iter().any(|a| a == task) => {}
            Some(task) => after.push(task.to_string()),
            None => return Ok(Err(format!("step `{earlier}` has no task to wait for"))),
        }
    }
    let base_branch = match (after_work(def, step), def.work.as_deref()) {
        (true, Some(work)) => match newest_task(&rows, work) {
            Some(task) => format!("{TASK_BRANCH_PREFIX}{task}"),
            None => return Ok(Err(format!("build step `{work}` has no task"))),
        },
        _ => run.base_branch.clone(),
    };
    let prompt = match values(ctx, run, &rows, step).await? {
        Ok(prompt) => prompt,
        Err(why) => return Ok(Err(why)),
    };
    let input = CreateTaskInput {
        name: task_name(&run.id, &step.id, round),
        title: format!("{}: {}", step.id, run.id),
        prompt,
        repo: Some(run.repo.clone()),
        base_branch: Some(base_branch),
        after,
        agent: Some(step.agent.clone()),
        pr_base: Some(run.base_branch.clone()),
        check: step.check.clone(),
        max_retries: step.max_retries,
        ..CreateTaskInput::default()
    };
    Ok(crate::mcp::create_step_task(ctx, &run.conversation_id, input).await)
}

async fn values(
    ctx: &Context,
    run: &Run,
    rows: &[RunStep],
    step: &Step,
) -> Result<std::result::Result<String, String>> {
    let Some(prompt) = step.prompt.as_deref() else {
        return Ok(Ok(run.input.clone()));
    };
    let used = match variables(prompt) {
        Ok(used) => used,
        Err(errors) => {
            let said: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Ok(Err(said.join("; ")));
        }
    };
    let mut values = Values {
        input: run.input.clone(),
        repo: run.repo.clone(),
        ..Values::default()
    };
    let tasks = ctx.store()?.all_tasks().await?;
    for var in used {
        let Some(id) = var.step() else { continue };
        let Some(name) = newest_task(rows, id) else {
            return Ok(Err(format!("{var} names step `{id}`, which has no task")));
        };
        match var {
            Var::StepBranch(_) => {
                values
                    .branches
                    .insert(id.to_string(), format!("{TASK_BRANCH_PREFIX}{name}"));
            }
            _ => {
                let said = match tasks.iter().find(|t| t.name == name) {
                    Some(task) => last_said(ctx, task).await?,
                    None => String::new(),
                };
                values.results.insert(id.to_string(), said);
            }
        }
    }
    Ok(render(prompt, &values).map_err(|e| e.to_string()))
}

async fn last_said(ctx: &Context, task: &Task) -> Result<String> {
    let who = format!("`{}` ({})", task.name, task.agent);
    Ok(report_of(ctx, task, &who).await?.unwrap_or_default())
}

enum Problem {
    Stop(String),
    Rejected { why: String, report: String },
}

async fn verdict_problem(ctx: &Context, task: &Task) -> Result<Option<Problem>> {
    if report::vocabulary(&task.agent).is_none() {
        return Ok(None);
    }
    let who = format!("`{}` ({})", task.name, task.agent);
    if let Some(why) = moved(ctx, task, &who).await? {
        return Ok(Some(Problem::Stop(why)));
    }
    let text = match report_of(ctx, task, &who).await? {
        Ok(text) => text,
        Err(why) => return Ok(Some(Problem::Stop(format!("no VERDICT line: {why}")))),
    };
    Ok(match report::parse(&text, &task.agent).verdict {
        Ok(Some(verdict)) if verdict.clears() => None,
        Ok(Some(verdict)) => Some(Problem::Rejected {
            why: format!("{who} reported {}", verdict.as_str()),
            report: text,
        }),
        Ok(None) => Some(Problem::Stop(format!("{who} report has no VERDICT line"))),
        Err(why) => Some(Problem::Stop(format!("{who} report: {why}"))),
    })
}

async fn moved(ctx: &Context, task: &Task, who: &str) -> Result<Option<String>> {
    let Some(launch) = ctx.store()?.task_launch(&task.name).await? else {
        return Ok(None);
    };
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    match commit_of(&ws, &task.branch()).await {
        Ok(tip) if tip != launch.start_commit => Ok(Some(format!(
            "{who} moved its branch: a verdict step must not commit"
        ))),
        Ok(_) | Err(Error::InvalidInput(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
