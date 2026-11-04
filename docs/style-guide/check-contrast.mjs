import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const css = readFileSync(fileURLToPath(new URL("./tokens.css", import.meta.url)), "utf8");

const TEXT = 4.5;
const UI = 3;

const PAIRS = [
  ["fg", "bg", TEXT],
  ["fg", "surface-2", TEXT],
  ["fg-muted", "bg", TEXT],
  ["fg-muted", "surface", TEXT],
  ["fg-muted", "surface-2", TEXT],
  ["fg-faint", "bg", TEXT],
  ["fg-faint", "surface", TEXT],
  ["fg-faint", "surface-2", TEXT],
  ["on-inverse", "fg", TEXT],
  ["on-inverse", "fg-muted", TEXT],
  ["accent", "bg", TEXT],
  ["accent", "surface", TEXT],
  ["on-accent", "accent", TEXT],
  ["ok", "bg", TEXT],
  ["ok", "surface", TEXT],
  ["danger", "bg", TEXT],
  ["danger", "surface", TEXT],
  ["on-danger", "danger", TEXT],
  ["on-danger", "danger-strong", TEXT],
  ["info", "bg", TEXT],
  ["info", "surface", TEXT],
  ["line-control", "bg", UI],
  ["line-control", "surface", UI],
  ["accent", "surface-2", UI],
];

function block(selector) {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`missing block ${selector}`);
  const end = css.indexOf("\n}", start);
  const tokens = {};
  for (const [, name, value] of css.slice(start, end).matchAll(/--th-([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
    tokens[name] = value;
  }
  return tokens;
}

function luminance(hex) {
  const channels = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const dark = block(":root");
const themes = { dark, light: { ...dark, ...block(':root[data-theme="light"]') } };

let failed = 0;
for (const [theme, tokens] of Object.entries(themes)) {
  console.log(`\n${theme}`);
  for (const [fg, bg, min] of PAIRS) {
    const value = ratio(tokens[fg], tokens[bg]);
    const ok = value >= min;
    if (!ok) failed++;
    console.log(`  ${ok ? "pass" : "FAIL"}  ${value.toFixed(2).padStart(5)}:1  ${fg} on ${bg} (min ${min})`);
  }
}

if (failed > 0) {
  console.error(`\n${failed} pair(s) below the minimum`);
  process.exit(1);
}
console.log("\nall pairs pass");
