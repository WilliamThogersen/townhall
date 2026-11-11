import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

export const DAEMON_PORT = Number(process.env.TOWNHALL_E2E_DAEMON_PORT ?? 7791);
export const WEB_PORT = Number(process.env.TOWNHALL_E2E_WEB_PORT ?? 5191);

const SCRIPT = resolve(import.meta.dirname, "../../scripts/demo.sh");
const READY_MS = 300_000;
const POLL_MS = 500;
const STOP_MS = 15_000;

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

const DEMO_DIR_ENV = "TOWNHALL_E2E_DEMO_DIR";
const AGENTS_AT_ONCE = 50;

export async function freeAgentSlots(): Promise<void> {
  const url = `http://127.0.0.1:${DAEMON_PORT}/api/settings/agentsAtOnce`;
  const response = await fetch(url, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ value: AGENTS_AT_ONCE }),
  });
  if (!response.ok) throw new Error(`PUT ${url} answered ${response.status}: ${await response.text()}`);
}

function run(args: readonly string[]): ChildProcess {
  return spawn(SCRIPT, args, { stdio: ["ignore", "inherit", "inherit"] });
}

function exited(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((done) => child.once("exit", () => done()));
}

async function waitForReady(dir: string, child: ChildProcess): Promise<void> {
  let gone = false;
  void exited(child).then(() => {
    gone = true;
  });
  const deadline = Date.now() + READY_MS;
  while (Date.now() < deadline) {
    if (existsSync(join(dir, "run", "ready"))) return;
    if (gone) throw new Error(`the demo exited before it was ready, see ${dir}/logs`);
    await sleep(POLL_MS);
  }
  throw new Error(`the demo was not ready after ${READY_MS / 1000}s, see ${dir}/logs`);
}

async function stopDemo(dir: string, child: ChildProcess): Promise<void> {
  await exited(run(["--stop", "--dir", dir]));
  await Promise.race([exited(child), sleep(STOP_MS)]);
  child.kill("SIGTERM");
}

export async function startDemo(): Promise<() => Promise<void>> {
  const dir = mkdtempSync(join(tmpdir(), "townhall-e2e."));
  process.env[DEMO_DIR_ENV] = dir;
  const child = run(["--daemon-port", String(DAEMON_PORT), "--web-port", String(WEB_PORT), "--dir", dir]);
  try {
    await waitForReady(dir, child);
  } catch (error) {
    await stopDemo(dir, child);
    throw error;
  }
  return () => stopDemo(dir, child);
}

export interface DemoRepo {
  readonly name: string;
  readonly path: string;
  readonly baseBranch: string;
}

export function townhallRepo(): DemoRepo {
  const dir = process.env[DEMO_DIR_ENV];
  if (!dir) throw new Error(`${DEMO_DIR_ENV} is not set: the demo starts in globalSetup`);
  return { name: "townhall", path: join(dir, "repos", "townhall"), baseBranch: "main" };
}
