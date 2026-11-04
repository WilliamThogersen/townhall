import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { makeRepos } from "./repos.mjs";
import { CONVERSATIONS, SHELL_AGENT } from "./scenes/conversations.mjs";
import { daemonApi, waitFor } from "./seed/api.mjs";
import { seedConversation } from "./seed/conversations.mjs";
import { database } from "./seed/db.mjs";
import { driveTasks } from "./seed/tasks.mjs";
import { backdate } from "./seed/timeline.mjs";

const DAEMON_MS = 60000;
const AGENTS_WHILE_SEEDING = 20;
const AGENTS_AT_ONCE = 6;
const HERE = fileURLToPath(new URL(".", import.meta.url));

function parseArgs(argv) {
  const value = (flag) => {
    const at = argv.indexOf(flag);
    return at >= 0 ? argv[at + 1] : undefined;
  };
  const opts = { daemon: value("--daemon"), web: value("--web"), dir: value("--dir"), empty: argv.includes("--empty") };
  const missing = ["daemon", "web", "dir"].filter((key) => !opts[key]);
  if (missing.length > 0) throw new Error(`seed needs --${missing.join(", --")}`);
  return opts;
}

const shellQuote = (text) => `'${text.replaceAll("'", "'\\''")}'`;

async function startShellAgent(api) {
  const command = `node ${shellQuote(join(HERE, SHELL_AGENT.script))}`;
  await api.post("/agents", { name: SHELL_AGENT.name, adapter: "shell", command, repo: SHELL_AGENT.repo });
}

async function seedConversations(api, db) {
  const seeded = [];
  for (const plan of CONVERSATIONS) {
    seeded.push({ ...(await seedConversation(api, db, plan)), plan });
  }
  return seeded;
}

async function finishConversations(api, db, seeded) {
  for (const { id, plan } of seeded) {
    for (const send of plan.sends ?? []) await api.post(`/agents/${send.agent}/send`, { text: send.text });
    if (plan.end) await api.post(`/conversations/${id}/end`);
  }
  await new Promise((resolve) => setTimeout(resolve, 500));
  const now = Date.now();
  const details = await Promise.all(seeded.map(({ id }) => api.get(`/conversations/${id}`)));
  db.run(details.flatMap((detail, i) => backdate(detail, seeded[i].plan, now)));
}

async function seed(api, opts) {
  const db = database(join(opts.dir, "home"));
  const repos = makeRepos(join(opts.dir, "repos"));
  for (const repo of repos) await api.post("/repos", { path: repo.path });
  await api.put("/settings/agentsAtOnce", { value: AGENTS_WHILE_SEEDING });
  const seeded = await seedConversations(api, db);
  await startShellAgent(api);
  const { tasks, misses } = await driveTasks(api, db, repos);
  await finishConversations(api, db, seeded);
  await api.put("/settings/agentsAtOnce", { value: AGENTS_AT_ONCE });
  await api.get("/stats");
  const conversations = await api.get("/conversations");
  const titles = new Map(conversations.map((c) => [c.id, c.title]));
  return {
    repos: repos.map(({ name, path, base, merges }) => ({ name, path, base, merges })),
    conversations: seeded.map(({ key, id }) => ({ key, id, title: titles.get(id) })),
    tasks: tasks.map(({ name, state, repo }) => ({ name, state, repo })),
    agents: (await api.get("/agents")).map(({ name, repo, kind, state }) => ({ name, repo, kind, state: state.kind })),
    warnings: misses,
  };
}

function render(summary) {
  const rows = summary.conversations.map((c) => `  ${c.title.padEnd(32)} ${summary.web}/c/${c.id}`);
  const warnings = summary.warnings.map((w) => `warn: ${w}`);
  const counts = `${summary.conversations.length} conversations, ${summary.tasks.length} tasks, ${summary.agents.length} agents, ${summary.repos.length} repos`;
  return [`info: seeded ${counts}`, ...rows, ...warnings, ""].join("\n");
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const api = daemonApi(opts.daemon);
  await waitFor("the daemon", () => api.get("/info"), DAEMON_MS);
  const seeded = opts.empty ? { repos: [], conversations: [], tasks: [], agents: [], warnings: [] } : await seed(api, opts);
  const summary = { web: opts.web, daemon: opts.daemon, dir: opts.dir, empty: opts.empty, ...seeded };
  writeFileSync(join(opts.dir, "demo.json"), `${JSON.stringify(summary, null, 2)}\n`);
  process.stdout.write(render(summary));
}

main().catch((error) => {
  process.stderr.write(`error: demo seed failed: ${error.message}\n`);
  process.exit(1);
});
