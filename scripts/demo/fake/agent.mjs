import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { randomUUID } from "node:crypto";
import { createInterface } from "node:readline";
import { agentScene } from "../scenes/agents.mjs";
import { GIT_IDENTITY } from "../identity.mjs";
import { callLine, promptLine, resultLine, sessionLines, textLine, transcriptWriter } from "./transcript.mjs";
import { header, promptMark, sayLines, show, stepLines, workingLine } from "./screen.mjs";

const FAIL_LINGER_MS = 1500;
const YES = /^y(es)?$/i;
const NO = /^no?$/i;

function flagValue(args, name) {
  const flag = args.find((arg) => arg.startsWith(`${name}=`));
  return flag ? flag.slice(name.length + 1) : null;
}

function briefOf(prompt) {
  const path = /at (\/\S+\/brief\.md)/.exec(prompt)?.[1] ?? null;
  return path && existsSync(path) ? path : null;
}

function context(args) {
  const cut = args.indexOf("--");
  const prompt = cut >= 0 ? args.slice(cut + 1).join(" ") : "";
  const cwd = process.cwd();
  const brief = briefOf(prompt);
  return {
    cwd,
    name: basename(cwd),
    prompt: prompt.trim() || "(no prompt)",
    sessionId: flagValue(args, "--session-id") ?? randomUUID(),
    projects: process.env.TOWNHALL_CLAUDE_PROJECTS || null,
    briefStep: brief ? { tool: "Read", input: { file_path: brief }, out: [`Read ${readFileSync(brief, "utf8").split("\n").length} lines`] } : null,
  };
}

function git(cwd, args) {
  return execFileSync("git", [...GIT_IDENTITY, ...args], { cwd, stdio: ["ignore", "pipe", "pipe"] }).toString().trim();
}

function commit(cwd, change) {
  try {
    const path = join(cwd, change.file);
    mkdirSync(dirname(path), { recursive: true });
    appendFileSync(path, change.text);
    git(cwd, ["add", "-A"]);
    git(cwd, ["commit", "-q", "-m", change.message]);
    return git(cwd, ["rev-parse", "--short", "HEAD"]);
  } catch (error) {
    return `not committed: ${error.message.split("\n")[0]}`;
  }
}

function closingLines(scene, running) {
  if (scene.kind === "work") return running ? ["", workingLine(running)] : [];
  return scene.say ? sayLines(scene.say) : [];
}

async function start(ctx, scene, write) {
  if (scene.kind === "ship" && scene.commit) commit(ctx.cwd, scene.commit);
  write(sessionLines(ctx, scene, Date.now()));
  const steps = [ctx.briefStep, ...scene.steps].filter(Boolean);
  const running = steps.find((step) => step.running);
  await show([...header(ctx), ...steps.flatMap((step) => stepLines(ctx, step)), ...closingLines(scene, running)]);
}

function commitAnswer(ctx, scene, write, text) {
  const now = Date.now();
  const sha = scene.commit ? commit(ctx.cwd, scene.commit) : "nothing to commit";
  const step = { tool: "Bash", input: { command: `git commit -m "${scene.commit?.message ?? "wip"}"` }, out: [`[townhall/${ctx.name} ${sha}]`] };
  const done = `committed to townhall/${ctx.name}. done.`;
  const id = toolId();
  write([promptLine(ctx, now, text), callLine(ctx, now, id, step), resultLine(ctx, now, id, step), textLine(ctx, now, done)]);
  return [...stepLines(ctx, step), ...sayLines(done)];
}

function toolId() {
  return `toolu_${randomUUID().replace(/-/g, "").slice(0, 24)}`;
}

function sayAnswer(ctx, write, text, reply) {
  const now = Date.now();
  write([promptLine(ctx, now, text), textLine(ctx, now, reply)]);
  return sayLines(reply);
}

function workAnswer(ctx, scene, write, text) {
  write([promptLine(ctx, Date.now(), text)]);
  const running = scene.steps.find((step) => step.running);
  return [...sayLines(`noted: ${text}`), ...(running ? [workingLine(running)] : [])];
}

function answer(ctx, scene, write, phase, text) {
  if (phase === "ask" && YES.test(text)) return { phase: "done", lines: commitAnswer(ctx, scene, write, text) };
  if (phase === "ask" && NO.test(text)) {
    return { phase: "done", lines: sayAnswer(ctx, write, text, `ok, the change stays uncommitted on townhall/${ctx.name}.`) };
  }
  if (phase === "ask") return { phase, lines: sayAnswer(ctx, write, text, scene.say) };
  if (phase === "work") return { phase, lines: workAnswer(ctx, scene, write, text) };
  return { phase, lines: sayAnswer(ctx, write, text, `noted: ${text}`) };
}

function initialPhase(scene) {
  return scene.kind === "ask" || scene.kind === "work" ? scene.kind : "done";
}

async function listen(ctx, scene, write) {
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity })[Symbol.asyncIterator]();
  let phase = initialPhase(scene);
  for (let next = await lines.next(); !next.done; next = await lines.next()) {
    const text = next.value.trim();
    if (!text) continue;
    const answered = answer(ctx, scene, write, phase, text);
    await show(["", ...answered.lines]);
    if (answered.phase !== "work") promptMark();
    phase = answered.phase;
  }
  return 0;
}

export async function runAgent(args) {
  const ctx = context(args);
  const scene = agentScene(ctx.name);
  const write = transcriptWriter(ctx);
  await start(ctx, scene, write);
  if (scene.kind === "fail") {
    await new Promise((resolve) => setTimeout(resolve, FAIL_LINGER_MS));
    return scene.exitCode ?? 1;
  }
  if (scene.kind !== "work") promptMark();
  return listen(ctx, scene, write);
}
