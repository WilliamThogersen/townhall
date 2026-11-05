import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { GIT_IDENTITY, PERSON_EMAIL, PERSON_NAME } from "./identity.mjs";
import { CHORES, FEATURES, FIXES, REPOS } from "./scenes/repos.mjs";

const DAY_MS = 86400000;
const HOUR_MS = 3600000;
const HISTORY_DAYS = 730;
const YEAR_DAYS = 365;
const TIMEZONE = "+0100";
const OFFSET_MS = HOUR_MS;
const KEEP_LINES = 240;
const FIRST_BRANCH = 100;
const TOPICS = ["rules", "store", "view"];

function random(seed) {
  let t = seed >>> 0;
  return () => {
    t += 0x6d2b79f5;
    let r = Math.imul(t ^ (t >>> 15), 1 | t);
    r ^= r + Math.imul(r ^ (r >>> 7), 61 | r);
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (rng, list) => list[Math.floor(rng() * list.length)];
const fill = (phrase, area) => phrase.replaceAll("{a}", area);
const capital = (word) => word.charAt(0).toUpperCase() + word.slice(1).replace(/-./g, (m) => m[1].toUpperCase());

function localMidnight(ms) {
  const shifted = new Date(ms + OFFSET_MS);
  return Date.UTC(shifted.getUTCFullYear(), shifted.getUTCMonth(), shifted.getUTCDate()) - OFFSET_MS;
}

function activity(day) {
  const recent = day >= HISTORY_DAYS - YEAR_DAYS;
  const progress = recent ? (day - (HISTORY_DAYS - YEAR_DAYS)) / YEAR_DAYS : day / (HISTORY_DAYS - YEAR_DAYS);
  return recent ? 0.3 + 0.7 * progress : 0.06 + 0.16 * progress;
}

function mergesOn(rng, chance) {
  if (rng() >= chance) return 0;
  return 1 + (rng() < 0.35 ? 1 : 0) + (rng() < 0.12 ? 1 : 0);
}

function planCommits(rng, kind, area, mergeAt) {
  const count = 1 + (rng() < 0.45 ? 1 : 0) + (rng() < 0.15 ? 1 : 0);
  const first = `${kind}: ${fill(pick(rng, kind === "fix" ? FIXES : FEATURES), area)}`;
  return Array.from({ length: count }, (_, i) => ({
    at: mergeAt - (count - i) * (15 + Math.floor(rng() * 70)) * 60000,
    subject: i === 0 ? first : fill(pick(rng, [...CHORES, `fix: ${pick(rng, FIXES)}`, `feat: ${pick(rng, FEATURES)}`]), area),
    lines: 6 + Math.floor(rng() ** 2 * 170),
  }));
}

function planDay(rng, repo, dayStart, chance, seq) {
  return Array.from({ length: mergesOn(rng, chance) }, (_, i) => {
    const kind = rng() < 0.6 ? "fix" : "feat";
    const area = pick(rng, repo.areas);
    const at = dayStart + (9 + i * 2.5 + rng() * 2) * HOUR_MS;
    return { branch: `${kind}-${area}-${seq + i}`, area, topic: pick(rng, TOPICS), at, commits: planCommits(rng, kind, area, at) };
  });
}

export function planHistory(repo, now, seed) {
  const rng = random(seed);
  const today = localMidnight(now);
  return Array.from({ length: HISTORY_DAYS }, (_, day) => day).reduce((merges, day) => {
    const dayStart = today - (HISTORY_DAYS - day) * DAY_MS;
    const weekday = new Date(dayStart + OFFSET_MS).getUTCDay();
    const quiet = weekday === 0 || weekday === 6 ? 0.12 : 1;
    const chance = Math.min(0.95, 0.62 * activity(day) * quiet * repo.weight);
    return [...merges, ...planDay(rng, repo, dayStart, chance, FIRST_BRANCH + merges.length)];
  }, []);
}

function codeLines(repo, area, count, seed) {
  const name = `${capital(area)}${seed}`;
  const unit = repo.ext === "py"
    ? (n) => [`def check_${area.replace(/-/g, "_")}_${seed}_${n}(rows):`, `    return [row for row in rows if row.get('${area}') is not None][:${n + 10}]`, ""]
    : (n) => [`export function normalize${name}x${n}(value: string): string {`, `  return value.trim().slice(0, ${n + 40});`, "}", ""];
  return Array.from({ length: Math.ceil(count / 4) }, (_, n) => unit(n)).flat().slice(0, count);
}

function grow(content, added) {
  const all = [...(content ? content.split("\n").filter(Boolean) : []), ...added];
  return `${all.slice(-KEEP_LINES).join("\n")}\n`;
}

const stamp = (ms) => `${Math.floor(ms / 1000)} ${TIMEZONE}`;
const data = (text) => `data ${Buffer.byteLength(text)}\n${text}\n`;

function commitBlock({ ref, mark, at, message, from, merge, files }) {
  const who = `${PERSON_NAME} <${PERSON_EMAIL}> ${stamp(at)}`;
  const parents = `${from ? `from :${from}\n` : ""}${merge ? `merge :${merge}\n` : ""}`;
  const changes = Object.entries(files).map(([path, text]) => `M 100644 inline ${path}\n${data(text)}`).join("");
  return `commit ${ref}\nmark :${mark}\nauthor ${who}\ncommitter ${who}\n${data(message)}${parents}${changes}\n`;
}

function mergeBlocks(repo, state, merge, index) {
  const path = `src/${merge.area}/${merge.topic}.${repo.ext}`;
  const ref = `refs/heads/townhall/${merge.branch}`;
  const branch = merge.commits.reduce((acc, commit, i) => {
    const text = grow(acc.files[path], codeLines(repo, merge.area, commit.lines, index * 10 + i));
    const mark = acc.mark + 1;
    const block = commitBlock({ ref, mark, at: commit.at, message: commit.subject, from: acc.tip, files: { [path]: text } });
    return { mark, tip: mark, files: { ...acc.files, [path]: text }, blocks: [...acc.blocks, block] };
  }, { mark: state.mark, tip: state.base, files: state.files, blocks: [] });
  const mark = branch.mark + 1;
  const message = `Merge branch 'townhall/${merge.branch}'`;
  const block = commitBlock({ ref: `refs/heads/${repo.base}`, mark, at: merge.at, message, from: state.base, merge: branch.tip, files: { [path]: branch.files[path] } });
  return { mark, base: mark, files: branch.files, blocks: [...branch.blocks, block] };
}

export function fastImportStream(repo, merges, now) {
  const start = localMidnight(now) - (HISTORY_DAYS + 1) * DAY_MS + 9 * HOUR_MS;
  const first = commitBlock({ ref: `refs/heads/${repo.base}`, mark: 1, at: start, message: `chore: start ${repo.name}`, files: repo.files });
  const initial = { mark: 1, base: 1, files: repo.files, chunks: [first] };
  const done = merges.reduce((state, merge, index) => {
    const next = mergeBlocks(repo, state, merge, index);
    return { mark: next.mark, base: next.base, files: next.files, chunks: [...state.chunks, next.blocks.join("")] };
  }, initial);
  return `${done.chunks.join("")}done\n`;
}

function git(cwd, args, input) {
  return execFileSync("git", [...GIT_IDENTITY, ...args], { cwd, input, stdio: ["pipe", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 }).toString();
}

export function makeRepo(parent, repo, now, seed) {
  const dir = join(parent, repo.name);
  mkdirSync(dir, { recursive: true });
  git(dir, ["init", "-q", "-b", repo.base]);
  const merges = planHistory(repo, now, seed);
  git(dir, ["fast-import", "--quiet", "--done"], fastImportStream(repo, merges, now));
  git(dir, ["reset", "-q", "--hard"]);
  git(dir, ["pack-refs", "--all"]);
  return { name: repo.name, path: dir, base: repo.base, merges: merges.length };
}

export function makeRepos(parent, now = Date.now()) {
  return REPOS.map((repo, i) => makeRepo(parent, repo, now, 4242 + i * 97));
}
