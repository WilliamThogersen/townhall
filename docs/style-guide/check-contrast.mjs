// Contrast check for tokens.css. Usage: node docs/style-guide/check-contrast.mjs
// Parses the light and dark blocks, checks WCAG 2.x ratios, verifies the two dark blocks match.
// Exit code 1 on any failure. No dependencies.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const css = readFileSync(fileURLToPath(new URL("./tokens.css", import.meta.url)), "utf8");
const vars = (block) => Object.fromEntries([...block.matchAll(/--th-([\w-]+):\s*(#[0-9a-f]{6})\s*;/gi)].map((m) => [m[1], m[2]]));
const light = vars(css.slice(css.indexOf(":root {"), css.indexOf(':root[data-density="compact"]')));
const darkMedia = vars(css.slice(css.indexOf("@media (prefers-color-scheme: dark)")));
const darkForced = vars(css.slice(css.indexOf(':root[data-theme="dark"]')));
const dark = { ...light, ...darkForced };

const lum = (hex) => {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
};
const ratio = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

const surfaces = ["bg", "surface", "surface-raised", "surface-sunken"];
const statuses = ["ok", "working", "waiting", "danger", "dead"];
const pairs = []; // [fg, bg, min, label]
for (const s of surfaces) {
  pairs.push(["text", s, 4.5], ["text-muted", s, 4.5]);
}
pairs.push(["text-subtle", "bg", 4.5], ["text-subtle", "surface", 4.5], ["text-subtle", "surface-raised", 4.5]);
pairs.push(["accent-text", "bg", 4.5], ["accent-text", "surface", 4.5], ["accent-text", "accent-subtle", 4.5], ["text", "accent-subtle", 4.5]);
for (const a of ["accent", "accent-hover", "accent-active"]) pairs.push(["on-accent", a, 4.5]);
for (const a of ["danger-solid", "danger-solid-hover"]) pairs.push(["on-danger", a, 4.5]);
for (const st of statuses) {
  pairs.push([st, "surface", 4.5], [st, "bg", 4.5], [st, `${st}-bg`, 4.5], [`${st}`, "surface-raised", 4.5]);
}
// Non-text 3:1
pairs.push(["border-strong", "bg", 3], ["border-strong", "surface", 3], ["border-strong", "surface-raised", 3]);
pairs.push(["focus", "bg", 3], ["focus", "surface", 3], ["focus", "surface-sunken", 3], ["accent", "bg", 3], ["accent", "surface", 3]);
// Terminal
for (const c of ["fg", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "bright-black", "bright-red", "bright-green", "bright-yellow", "bright-blue", "bright-magenta", "bright-cyan", "bright-white", "cursor"]) {
  pairs.push([`term-${c}`, "term-bg", c === "cursor" ? 3 : 4.5]);
}
pairs.push(["term-black", "term-bg", 3]); // ANSI black is deliberately dim on dark; still >= 3
pairs.push(["term-fg", "term-selection", 4.5]);

let fail = 0;
for (const [name, theme] of [["light", light], ["dark", dark]]) {
  console.log(`\n== ${name} ==`);
  for (const [fg, bg, min] of pairs) {
    if (!theme[fg] || !theme[bg]) { console.log(`MISSING ${fg} / ${bg}`); fail++; continue; }
    const r = ratio(theme[fg], theme[bg]);
    const ok = r >= min;
    if (!ok) fail++;
    console.log(`${ok ? "pass" : "FAIL"} ${r.toFixed(2).padStart(5)} (min ${min})  ${fg} on ${bg}`);
  }
}
const same = JSON.stringify(darkMedia) === JSON.stringify(Object.fromEntries(Object.entries(darkForced)));
console.log(`\ndark blocks identical: ${same ? "yes" : "NO"}`);
if (!same) fail++;
console.log(fail ? `\n${fail} failure(s)` : "\nall checks pass");
process.exit(fail ? 1 : 0);
