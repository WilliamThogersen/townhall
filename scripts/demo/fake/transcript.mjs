import { appendFileSync, mkdirSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { randomUUID } from "node:crypto";

const MAX_NAME = 200;
const MODEL = "claude-demo";
const VERSION = "2.1.318";
const DEFAULT_GAP_SECONDS = 30;
const CLOSING_KINDS = new Set(["ask", "done", "ship"]);

function javaHash(text) {
  return Array.from({ length: text.length }, (_, i) => text.charCodeAt(i)).reduce(
    (hash, unit) => (Math.imul(hash, 31) + unit) | 0,
    0,
  );
}

export function projectDir(projects, cwd) {
  const name = cwd.replace(/[^A-Za-z0-9]/g, "-");
  if (name.length <= MAX_NAME) return join(projects, name);
  return join(projects, `${name.slice(0, MAX_NAME)}-${Math.abs(javaHash(cwd)).toString(36)}`);
}

export function transcriptWriter({ projects, cwd, sessionId }) {
  if (!projects) return () => {};
  const dir = projectDir(projects, cwd);
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `${sessionId}.jsonl`);
  return (lines) => appendFileSync(file, lines.map((line) => `${JSON.stringify(line)}\n`).join(""));
}

function base(ctx, at) {
  return {
    cwd: ctx.cwd,
    sessionId: ctx.sessionId,
    uuid: randomUUID(),
    timestamp: new Date(at).toISOString(),
    isSidechain: false,
    version: VERSION,
  };
}

function absoluteInput(ctx, input) {
  const file = input?.file_path;
  if (typeof file !== "string" || isAbsolute(file)) return input;
  return { ...input, file_path: join(ctx.cwd, file) };
}

function assistant(ctx, at, content, stop) {
  const message = { id: `msg_${randomUUID()}`, role: "assistant", model: MODEL, content, stop_reason: stop };
  return { ...base(ctx, at), type: "assistant", message };
}

export function promptLine(ctx, at, text) {
  return { ...base(ctx, at), type: "user", message: { role: "user", content: text } };
}

export function callLine(ctx, at, id, step) {
  const use = { type: "tool_use", id, name: step.tool, input: absoluteInput(ctx, step.input) };
  return assistant(ctx, at, [use], "tool_use");
}

export function resultLine(ctx, at, id, step) {
  const result = { type: "tool_result", tool_use_id: id, content: step.out.join("\n"), is_error: Boolean(step.error) };
  return { ...base(ctx, at), type: "user", message: { role: "user", content: [result] } };
}

export function textLine(ctx, at, text) {
  return assistant(ctx, at, [{ type: "text", text }], "end_turn");
}

function stepLines(ctx, step, at, gap) {
  const id = `toolu_${randomUUID().replace(/-/g, "").slice(0, 24)}`;
  const call = callLine(ctx, at, id, step);
  return step.running ? [call] : [call, resultLine(ctx, at + gap * 0.6, id, step)];
}

export function sessionLines(ctx, scene, now) {
  const end = now - (scene.endedSecondsAgo ?? DEFAULT_GAP_SECONDS) * 1000;
  const start = Math.min(end - 1000, now - scene.minutes * 60000);
  const steps = [ctx.briefStep, ...scene.steps].filter(Boolean);
  const gap = (end - start) / (steps.length + 1);
  const calls = steps.flatMap((step, i) => stepLines(ctx, step, start + gap * (i + 0.5), gap));
  const closing = scene.say && CLOSING_KINDS.has(scene.kind) ? [textLine(ctx, end, scene.say)] : [];
  return [promptLine(ctx, start, ctx.prompt), ...calls, ...closing];
}
