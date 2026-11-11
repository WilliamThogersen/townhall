# Design: user-defined pipelines for townhall

## Goal
Replace the fixed "plan › code › test › review › ship" line with pipelines the user defines. A pipeline is a saved TOML definition: ordered steps, the agent type that runs each step, and an optional condition on each step. Townhall's scheduler moves a run from step to step on its own, so neither the user nor the conversation agent has to chain tasks by hand.

## Findings on main
- `definitions/workflow.rs` already has `WorkflowDef`: `[[step]]` entries with `id`, `agent`, `after`, `from` and `prompt`, cycle checks (`validate.rs`), a start order (`order.rs`) and prompt variables (`{{input}}`, `{{steps.<id>.result}}`). It was never used.
- Storage and runs ("2.8c") were never built. `/api/library/workflows/*`, `/api/workflows/{name}/runs` and `/api/runs/{id}` all answer 501. `store/unit_2_8c.rs` has no migrations; the 850–899 range is reserved for it. The original plan is in `docs/plan/agents-and-workflows.md`.
- Precedent for the scheduler creating follow-up work itself: `scheduler/reflect.rs` hands every ended task on to `reflector` with `Store::hand_on`. `create_task_as_plugin` (`mcp/task_tools.rs`) creates tasks without the conversation agent.
- Verdicts: `scheduler/report.rs` parses `VERDICT:` and `COMMIT:` lines, but only for the types `tester` (PASS/FAIL) and `reviewer` (APPROVE/REJECT).
- `requires`: `scheduler/gate.rs` looks only through a task's `after` chain. A REJECT or FAIL found by the gate is final: the waiting task ends in `error`.
- Hand-backs are capped at 5 per task (`MAX_HANDOFFS`), and check retries count against the same cap (`handoffs.rs`).

## Adviser corrections (accepted)
- Every step starts from the build step's branch, not the previous step's. The reaper deletes branches git calls merged, and a tester branch with no commits counts as merged.
- A failed test or review hands the work back to the build task with `hand_on`. When the 5-hand-back cap is reached, the run needs the user.
- The landing step needs `pr_base` set, or its PR targets the build branch.
- The driver must read verdicts itself, because the gate's rejection ends a task for good.
- Reflect must skip pipeline tasks.
- The conversation agent must not be woken on every step.
- Moving a run forward must be safe to repeat, so it needs a side table.

## 1. Definition
- TOML, saved as text in sqlite like agent types. It extends `WorkflowDef`.
- One global table, `workflows`. `repos = [...]` limits a pipeline to some repos; empty means every repo.
- Plugins ship pipelines later through `contributes.pipelines`. These are read-only and named `<plugin>-<name>`.

Example:
```toml
name = "standard"
description = "plan, build, test, review when risky, land"
repos = []
default = true
work = "build"
max_fix_rounds = 1

[[step]]
id = "plan"
agent = "planner"
prompt = "Write an implementation plan for:\n\n{{input}}"

[[step]]
id = "build"
agent = "builder"
after = ["plan"]
prompt = "Implement this plan:\n\n{{steps.plan.result}}"
check = "cargo test --workspace"
max_retries = 2

[[step]]
id = "test"
agent = "tester"
after = ["build"]

[[step]]
id = "review"
agent = "reviewer"
after = ["test"]
when = { paths = ["**/migrations/**", "**/auth/**"] }

[[step]]
id = "land"
agent = "lander"
after = ["test", "review"]
```

## 2. Steps and conditions
- Steps form a graph through `after`. A step with no `when` always runs.
- Conditions:
  - v1: `when = { paths = [globs] }` matches the files changed between the build task's start commit and its branch tip. It uses a small built-in matcher (`*`, `**`, `?`), because there is no glob crate.
  - later: `when = { ask = "yes/no question" }`, answered once per step and round by the adviser.
  - later: `when = { verdict = {...} }` and `labels`.
- A skipped step counts as finished, and steps after it ignore it.
- Steps run by `tester` or `reviewer` are verdict steps and must not commit. When a verdict step's branch tip is not its start commit, the run needs the user.

## 3. Driver: townhall's scheduler
A new module, `scheduler/pipeline.rs`, runs in `run_once` before `reflect::queue`.
- When every `after` step of a step is finished, skipped or merged, it evaluates the step's `when`. It then creates the task through the same path as create_task:
  - name `<run>-<step>`, with `-r<round>` added for round 2 and later;
  - `after` = the newest task of each `after` step;
  - `base_branch` = the build task's branch, or the repo's base branch for steps before the build step;
  - `pr_base` = the run's base branch.
- If a verdict is FAIL or REJECT and fix rounds are left, it hands the build task back with `hand_on`, using the report as the reason. When the build task ends again, every verdict step that already ran is created again in a new round.
- If no fix rounds are left, or `hand_on` hits the cap, the run needs the user.

## 4. Fit with what exists
- `after` and `requires` keep working unchanged.
- A step's `check` is copied onto its task.
- The lander is the last step, with `pr_base` set.
- Fix rounds are the hand-backs.
- Events:
  - the agent watcher (`agent/watch/event.rs::for_entry`) stops sending `ended` events for run tasks;
  - the driver writes one thread entry per step it starts or skips (`EntryBody::Event`);
  - `runner::notify` fires only when the run finishes, fails or needs the user;
  - `needsYou` and `failed` events of step tasks still go through.
- Reflect skips run tasks.

## 5. UI and editing
- `LoopStrip` shows the newest run's own steps with their state, for example `plan ✓ › build 1 › test · › review skipped › land ·`, plus the "needs you" badge. A conversation with no run keeps today's line.
- Editing comes first through the conversation: `list_pipelines`, `read_pipeline` and `save_pipeline`, which confirm before saving. Later, a Pipelines section on the agents page.

## Slices
1. Store pipelines, plus the conversation tools (builder).
2. The driver for unconditional steps:
   - `create_task` with `pipeline` starts a run (a `runs` row with a snapshot of the definition), then the first steps;
   - forward moves are idempotent: claim the `run_steps` row first, and look the task up by its fixed name before creating it;
   - a verdict step that ends with FAIL, REJECT or no verdict, or whose branch moved, makes the run need the user, with one notify;
   - thread entries, suppressed `ended` events, and reflect skipping run tasks;
   - tests with shell-adapter types: plan → build → land in order, no duplicates when the driver runs twice, and a FAIL stops the run with one notify.
3. `when.paths` (`git diff --name-only <build start_commit>..townhall/<build>`, with skipped steps recorded along with the reason), and `max_fix_rounds` via `hand_on`, re-running verdict steps in round + 1 (a `Conflict` from `hand_on` means the run needs the user). Tests:
   - a migrations-only review is skipped for a README diff and runs for `db/migrations/1.sql`;
   - FAIL, fix, PASS lands;
   - a second FAIL needs the user.
4. The UI strip: `GET /api/runs/{id}` and `GET /api/runs?conversation=`, `web/src/data/useRuns.ts`, `LoopStrip.tsx` and `stage.ts`, with `loopSummary` as the fallback, plus docs/daemon-api.md.
5. A default pipeline per repo:
   - create_task with no pipeline uses the `default = true` pipeline matching the repo; one that names the repo wins over one with empty `repos`;
   - a second default for the same scope is refused;
   - `pipeline = ""` means none.
6. Adviser conditions `when.ask`:
   - reuse `runner::consult` and `Store::record_adviser_call`; the adviser gets the question plus the diff's file list;
   - the answer must start with YES or NO, and anything else counts as NO;
   - one call per step and round; `max_adviser_calls` (default 3) caps the run, and past it the step is skipped with the note "adviser budget used";
   - the answer is stored in `run_steps.note`.
7. A Pipelines section on the agents page with the TOML editor, and plugin pipelines via `contributes.pipelines` (`plugins/pipelines.rs`), read-only and named `<plugin>-<name>`.

## Acceptance criteria (whole feature)
- Saving is refused, with field-level errors, for:
  - an unknown agent;
  - `work` naming a missing step;
  - an `after` cycle;
  - a step requiring `reviewer` while the only reviewer step has a `when`.
- `create_task` with `pipeline = "standard"` makes the first task at once. Each following task appears only after the previous one ends, with no message in between.
- Test, review and land tasks start from the build branch tip, and the land PR targets the repo's base branch.
- A paths-skipped review shows as "skipped" in the thread and the strip, and land still starts.
- A FAIL with `max_fix_rounds = 1` hands back once and reruns the verdict steps. A second FAIL means the run needs the user, with exactly one event.
- A daemon restart mid-run creates no duplicates.
- A conversation with no run shows the old line.
- The adviser is never called without `ask` conditions, and at most `max_adviser_calls` times per run.

## Risks
- Runaway loops: capped by `max_fix_rounds`, the 5-hand-back cap and a unique key.
- Cost: verdict steps rerun every fix round; adviser calls are capped.
- Migration is purely additive, and no pipeline is seeded.
- Plugin-namespaced verdict types cannot give verdicts yet; a per-step `verdict =` is a later addition.
- Out of scope:
  - labels;
  - extra parallelism;
  - renaming workflow to pipeline in code;
  - changing the gate's rules.

## Done when

design doc committed, pipelines storable and savable via list/read/save_pipeline with validation, workflow routes work, tests pass
