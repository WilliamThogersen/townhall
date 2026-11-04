import { execFileSync } from "node:child_process";
import { GIT_IDENTITY } from "../identity.mjs";
import { PR_URL_BASE, TASK_GOALS } from "../scenes/conversations.mjs";
import { waitFor } from "./api.mjs";
import { sqlText } from "./db.mjs";

const STATE_MS = 60000;
const STARTED = new Set(["working", "needsYou", "ended", "error"]);

const goalNames = () => Object.keys(TASK_GOALS);

async function tasksByName(api) {
  const tasks = await api.get("/tasks");
  return new Map(tasks.map((task) => [task.name, task]));
}

async function waitForState(api, name, wanted) {
  const check = async () => {
    const task = (await tasksByName(api)).get(name);
    return task && wanted(task.state) ? task : null;
  };
  return waitFor(`task ${name}`, check, STATE_MS);
}

function prColumns(task, pr) {
  const url = `${PR_URL_BASE}/${task.repo}/pull/${pr.number}`;
  return `pr_number = ${pr.number}, pr_url = ${sqlText(url)}, pr_checks = ${sqlText(pr.checks)}`;
}

async function setByHand(api, db, name, goal) {
  const task = await waitForState(api, name, (state) => STARTED.has(state));
  db.run([`update tasks set state = ${sqlText(goal.state)}, ${prColumns(task, goal.pr)} where name = ${sqlText(name)};`]);
  return waitForState(api, name, (state) => state === goal.state);
}

async function mergeBranch(api, repos, name) {
  const task = await waitForState(api, name, (state) => state === "ended" || state === "merged");
  if (task.state === "merged") return task;
  const repo = repos.find((r) => r.name === task.repo);
  const message = `Merge branch 'townhall/${name}'`;
  execFileSync("git", [...GIT_IDENTITY, "merge", "--no-ff", "-q", "-m", message, `townhall/${name}`], { cwd: repo.path, stdio: "pipe" });
  return waitForState(api, name, (state) => state === "merged");
}

async function reach(api, db, repos, name) {
  const goal = TASK_GOALS[name];
  if (goal.set) return setByHand(api, db, name, goal);
  const task = goal.merge ? await mergeBranch(api, repos, name) : await waitForState(api, name, (state) => state === goal.state);
  if (goal.pr) db.run([`update tasks set ${prColumns(task, goal.pr)} where name = ${sqlText(name)};`]);
  return task;
}

export async function driveTasks(api, db, repos) {
  await waitFor("every demo task", async () => {
    const known = await tasksByName(api);
    return goalNames().every((name) => known.has(name));
  }, STATE_MS);
  const results = await Promise.allSettled(goalNames().map((name) => reach(api, db, repos, name)));
  const misses = results.flatMap((result, i) => (result.status === "rejected" ? [`${goalNames()[i]}: ${result.reason.message}`] : []));
  const final = await tasksByName(api);
  return { tasks: [...final.values()], misses };
}
