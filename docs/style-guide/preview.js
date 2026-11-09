const CONVERSATIONS = [
  { id: "login", title: "login redirect loop", repo: "townhall", time: "now", state: "waiting", glyph: "waiting", status: "fix-login needs you" },
  { id: "ci", title: "flaky ci on main", repo: "shop", time: "9m", state: "error", glyph: "error", status: "e2e-smoke failed" },
  { id: "vite", title: "upgrade to vite 8", repo: "townhall", time: "2m", glyph: "working", status: "thinking" },
  { id: "search", title: "search api is slow under load", repo: "shop", time: "14m", glyph: "alive", status: "your turn · 3 running" },
  { id: "i18n", title: "i18n keys cleanup", repo: "wexo-access", time: "1h", glyph: "alive", status: "your turn · 1 running" },
];

const SIDEBARS = {
  home: { nav: "new", current: null, grouped: false },
  convo: { nav: null, current: "login", grouped: false },
  "convo-grouped": { nav: null, current: "login", grouped: true },
  control: { nav: "control", current: null, grouped: false },
  repos: { nav: "repos", current: null, grouped: false },
  settings: { nav: "settings", current: null, grouped: false },
};

const METRICS = [
  { key: "fixes", label: "bug fixes", unit: "bug fixes", max: 6 },
  { key: "features", label: "features", unit: "features", max: 3 },
  { key: "lines", label: "lines shipped", unit: "lines", max: 900, compact: true },
  { key: "agents", label: "agents run", unit: "agents", max: 14 },
];

const TRACE = [
  [0, "login redirect loop", "townhall", 0, 58, "", "58m"],
  [1, "fix-login", "review", 42, 16, "waiting", "16m", "waiting", "needs you"],
  [1, "auth-tests", "test", 43, 15, "running", "15m", "working"],
  [1, "logout-cookie", "queued", 58, 0, "queued", "after fix-login", "queued"],
  [1, "redirect-guard", "review", 12, 28, "error", "28m · checks failed", "error"],
  [1, "session-ttl", "ship", 2, 22, "", "22m · merged #38", "merged"],
  [0, "flaky ci on main", "shop", 10, 48, "", "48m"],
  [1, "e2e-smoke", "code", 14, 35, "error", "35m · exited 1", "error"],
  [1, "ci-cache", "code", 30, 28, "running", "28m", "working"],
  [0, "search api is slow under load", "shop", 20, 38, "", "38m · your turn"],
  [1, "search-index", "code", 22, 36, "running", "36m", "working"],
  [1, "load-test", "test", 25, 26, "", "26m", "alive"],
  [1, "query-plan", "ended", 23, 15, "", "15m · ended", "dead"],
  [0, "upgrade to vite 8", "townhall", 55, 3, "running", "3m · planning"],
];
const TRACE_RANGE = 76;
const TRACE_NOW = 58;
const TRACE_TICK = 10;
const tickLabel = (m) => `${10 + Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`;

const REPOS = [
  ["townhall", "~/Developer/townhall", "main", "2 conversations", ["waiting", "working", "alive"]],
  ["shop", "~/Developer/…/clients/shop", "develop", "2 conversations", ["error", "alive", "alive", "alive", "alive"]],
  ["wexo-access", "~/Developer/wexo-access", "staging", "1 conversation", ["alive"]],
  ["data-hub", "~/Developer/data-hub", "main", "no conversations", []],
];

const SWATCHES = [
  ["bg", "app, thread, terminal, inputs"], ["surface", "sidebar, headers, your messages"], ["surface-2", "hover, current row, toast"], ["surface-3", "pressed, done bars"],
  ["line", "dividers, ticks"], ["line-strong", "blocks, chips, overlays"], ["line-control", "input and button borders (3:1)"],
  ["fg", "text, primary fill"], ["fg-muted", "secondary text"], ["fg-faint", "meta, hints, timestamps"],
  ["accent", "you: messages, focus, now, waiting"], ["ok", "alive, passed, shipped"], ["danger", "error, stop, failed"], ["info", "working, running"],
];
const ANSI = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
const SPIN = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
const END = new Date(2026, 10, 4);

const LOGO = '<svg class="th-logo" viewBox="0 0 18 14" width="18" height="14" shape-rendering="crispEdges" aria-hidden="true"><path fill="currentColor" d="M8 0h2v1h-2zM6 1h6v1h-6zM4 2h10v1h-10zM2 3h14v1h-14zM1 4h16v1h-16zM2 6h2v5h-2zM6 6h2v5h-2zM10 6h2v5h-2zM14 6h2v5h-2zM1 12h16v1h-16zM0 13h18v1h-18z"/></svg>';

const glyph = (state) => (state === "working"
  ? '<span class="th-status" data-state="working"><span class="th-spin"></span></span>'
  : `<span class="th-status" data-state="${state}"></span>`);

const glyphs = (states) => `<span class="th-glyphs" aria-hidden="true">${states.map(glyph).join("")}</span>`;

function convoRow(c, current, showRepo) {
  const attrs = [c.state ? `data-state="${c.state}"` : "", c.id === current ? 'aria-current="page"' : ""].join(" ");
  return `<li><button class="th-convo" ${attrs}>${glyph(c.glyph)}<span class="th-convo-title">${c.title}</span>` +
    `<span class="th-convo-time">${c.time}</span><span class="th-convo-status">${c.status}</span>` +
    `<span class="th-convo-repo">${showRepo ? c.repo : ""}</span></button></li>`;
}

function convoList(opts) {
  if (!opts.grouped) return `<ul class="th-list">${CONVERSATIONS.map((c) => convoRow(c, opts.current, true)).join("")}</ul>`;
  const repos = [...new Set(CONVERSATIONS.map((c) => c.repo))];
  return repos.map((repo) => {
    const rows = CONVERSATIONS.filter((c) => c.repo === repo);
    return `<button class="th-group-head" aria-expanded="true"><span>${repo}</span><span>${rows.length}</span></button>` +
      `<ul class="th-list">${rows.map((c) => convoRow(c, opts.current, false)).join("")}</ul>`;
  }).join("");
}

function navItem(id, icon, label, extra, current) {
  return `<a class="th-nav-item" href="#" ${current === id ? 'aria-current="page"' : ""}>` +
    `<span class="th-nav-icon" aria-hidden="true">${icon}</span><span>${label}</span>${extra}</a>`;
}

function renderSidebar(el) {
  const opts = SIDEBARS[el.dataset.pvSidebar];
  const agents = `<span class="th-nav-count">${glyph("waiting")}1 ${glyph("error")}1 ${glyph("alive")}6</span>`;
  el.innerHTML =
    `<a class="th-sidebar-top" href="#control" title="control center">${LOGO}<span class="th-wordmark">townhall</span></a>` +
    '<div class="th-nav">' +
    navItem("new", "+", "new conversation", '<kbd class="th-kbd">n</kbd>', opts.nav) +
    navItem("search", "/", "search", '<kbd class="th-kbd">/</kbd>', opts.nav) +
    navItem("repos", "~", "repos", '<span class="th-nav-count">4</span>', opts.nav) +
    navItem("control", "$", "control center", agents, opts.nav) +
    "</div>" +
    '<div class="th-sidebar-scroll">' +
    `<h3 class="th-section">active 5<button class="th-section-action" aria-pressed="${opts.grouped}">by repo</button></h3>` +
    convoList(opts) +
    '<h3 class="th-section">older 12</h3>' +
    "</div>" +
    `<div class="th-sidebar-foot"><span title="William Thøgersen">William</span><button class="th-btn th-btn--ghost th-btn--sm" ${opts.nav === "settings" ? 'aria-current="page"' : ""}>settings</button></div>`;
}

function renderStatus(el) {
  el.innerHTML = `<span>${glyph("waiting")}1 waiting</span><span>${glyph("error")}1 error</span><span>${glyph("alive")}6 running</span>`;
}

function seeded(seed) {
  let t = seed >>> 0;
  return () => {
    t += 0x6d2b79f5;
    let r = Math.imul(t ^ (t >>> 15), 1 | t);
    r ^= r + Math.imul(r ^ (r >>> 7), 61 | r);
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}

function days(year) {
  const end = new Date(END.getFullYear() - year, END.getMonth(), END.getDate());
  const offset = (end.getDay() + 6) % 7;
  const start = new Date(end);
  start.setDate(end.getDate() - offset - 52 * 7);
  const count = Math.round((end - start) / 86400000) + 1;
  return Array.from({ length: count }, (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i));
}

function series(metric, year) {
  const rand = seeded(metric.max * 97 + year * 13 + metric.key.length);
  const list = days(year);
  return list.map((date, i) => {
    const growth = year === 0 ? Math.min(1, Math.max(0, (i - 150) / 200)) : Math.max(0, (i - 330) / 300);
    const weekend = date.getDay() === 0 || date.getDay() === 6 ? 0.2 : 1;
    const v = rand() < 0.72 * growth * weekend + 0.02 ? Math.round(rand() * metric.max * (0.4 + growth)) : 0;
    return { date, v };
  });
}

const fmt = (n, compact) => (compact && n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n));

let metricIndex = 0;
let yearIndex = 0;

function renderTiles() {
  document.getElementById("pv-tiles").innerHTML = METRICS.map((m, i) => {
    const now = series(m, yearIndex).reduce((a, d) => a + d.v, 0);
    const prev = series(m, yearIndex + 1).reduce((a, d) => a + d.v, 0);
    const diff = now - prev;
    const dir = diff > 0 ? "up" : diff < 0 ? "down" : "flat";
    const sign = diff > 0 ? "+" : diff < 0 ? "−" : "±";
    return `<button class="th-tile" aria-pressed="${i === metricIndex}" data-metric="${i}">` +
      `<span class="th-tile-label">${m.label}</span><span class="th-tile-value">${m.compact ? "+" : ""}${fmt(now, m.compact)}</span>` +
      `<span class="th-tile-delta" data-dir="${dir}">${sign}${fmt(Math.abs(diff), m.compact)} vs ${2025 - yearIndex}</span></button>`;
  }).join("");
  document.querySelectorAll(".th-tile").forEach((tile) => tile.addEventListener("click", () => {
    metricIndex = Number(tile.dataset.metric);
    renderStats();
  }));
}

function renderHeatmap() {
  const metric = METRICS[metricIndex];
  const data = series(metric, yearIndex);
  const top = Math.max(1, ...data.map((d) => d.v));
  const cells = [];
  let lastMonth = -1;
  data.forEach((d, i) => {
    const col = Math.floor(i / 7) + 2;
    const row = (i % 7) + 2;
    if (row === 2 && d.date.getMonth() !== lastMonth && col < 53) {
      lastMonth = d.date.getMonth();
      cells.push(`<span class="th-heat-month" style="grid-column: ${col} / span 4">${MONTHS[lastMonth]}</span>`);
    }
    const level = d.v === 0 ? 0 : Math.min(4, Math.ceil((d.v / top) * 4));
    const tip = `${fmt(d.v, metric.compact)} ${metric.unit} · ${DAYS[i % 7]} ${d.date.getDate()} ${MONTHS[d.date.getMonth()]}`;
    cells.push(`<span class="th-heat-cell" data-level="${level}" data-tip="${tip}" style="grid-column: ${col}; grid-row: ${row}"></span>`);
  });
  [0, 2, 4].forEach((r) => cells.push(`<span class="th-heat-day" style="grid-row: ${r + 2}">${DAYS[r]}</span>`));
  document.getElementById("pv-heat-grid").innerHTML = cells.join("");
  const total = data.reduce((a, d) => a + d.v, 0);
  const when = yearIndex === 0 ? "in the last 12 months" : `in ${2025}`;
  document.getElementById("pv-heat-caption").textContent = `${fmt(total, metric.compact)} ${metric.unit} shipped ${when}`;
  const months = data.reduce((acc, d) => {
    const key = `${MONTHS[d.date.getMonth()]} ${d.date.getFullYear()}`;
    return { ...acc, [key]: (acc[key] || 0) + d.v };
  }, {});
  document.getElementById("pv-heat-table").innerHTML = `<caption>${metric.unit} per month</caption>` +
    Object.entries(months).map(([m, v]) => `<tr><th scope="row">${m}</th><td>${fmt(v, metric.compact)}</td></tr>`).join("");
}

function renderStats() {
  renderTiles();
  renderHeatmap();
}

function wireHeatTip() {
  const map = document.getElementById("pv-heatmap");
  const tip = document.getElementById("pv-heat-tip");
  map.addEventListener("mouseover", (e) => {
    const cell = e.target.closest(".th-heat-grid .th-heat-cell");
    if (!cell) return;
    const box = map.getBoundingClientRect();
    const r = cell.getBoundingClientRect();
    tip.textContent = cell.dataset.tip;
    tip.style.left = `${r.left - box.left + r.width / 2}px`;
    tip.style.top = `${r.top - box.top}px`;
    tip.hidden = false;
  });
  map.addEventListener("mouseleave", () => { tip.hidden = true; });
  document.querySelectorAll('input[name="pv-year"]').forEach((input) => input.addEventListener("change", () => {
    yearIndex = Number(input.value);
    renderStats();
  }));
}

function renderTrace() {
  const pct = (m) => `${(m / TRACE_RANGE) * 100}%`;
  const count = Math.floor(TRACE_RANGE / TRACE_TICK) + 1;
  const ticks = Array.from({ length: count }, (_, i) => `<span class="th-tick" style="--at: ${pct(i * TRACE_TICK)}">${tickLabel(i * TRACE_TICK)}</span>`).join("");
  const grid = `--th-trace-ticks: ${TRACE_RANGE / TRACE_TICK}`;
  const axis = `<div class="th-span th-span--axis"><div class="th-span-label">conversation · task</div><div class="th-span-track" style="${grid}">${ticks}</div></div>`;
  const rows = TRACE.map(([depth, name, meta, start, len, state, label, status, tag]) => {
    const toggle = depth === 0 ? '<button class="th-span-toggle" aria-expanded="true">▾</button>' : '<button class="th-span-toggle" aria-expanded="false">▸</button>';
    const tagHtml = tag ? `<span class="th-tag th-tag--waiting" style="margin-left: auto">${tag}</span>` : "";
    const pos = `--start: ${pct(start)}; --len: ${pct(len)}`;
    return `<div class="th-span" data-depth="${depth}" style="--depth: ${depth}">` +
      `<div class="th-span-label">${toggle}${status ? glyph(status) : ""}<span class="th-span-name">${name}</span><span class="th-span-meta">${meta}</span>${tagHtml}</div>` +
      `<div class="th-span-track" style="${grid}"><span class="th-bar" ${state ? `data-state="${state}"` : ""} style="${pos}"></span>` +
      `<span class="th-bar-label" style="${pos}">${label}</span></div></div>`;
  }).join("");
  document.getElementById("pv-trace").innerHTML = axis + rows + `<div class="th-now" style="--now: ${TRACE_NOW / TRACE_RANGE}"></div>`;
}

function renderClones() {
  document.querySelectorAll("[data-pv-clone]").forEach((slot) => {
    const copy = document.getElementById(slot.dataset.pvClone).cloneNode(true);
    copy.removeAttribute("id");
    slot.replaceWith(copy);
  });
}

function renderRepos() {
  document.getElementById("pv-repos").innerHTML = REPOS.map(([name, path, branch, convos, states]) =>
    `<div class="th-repo"><div class="th-repo-name"><b>${name}</b><span>${path}</span></div>` +
    `<div class="th-repo-meta"><span>${branch}</span><span>${convos}</span></div>` +
    `<span class="pv-cap">${states.length ? `${glyphs(states)} ${states.length} agent${states.length === 1 ? "" : "s"}` : "no agents"}</span>` +
    '<button class="th-btn th-btn--ghost th-btn--sm">new conversation</button></div>').join("");
}

function wireLightbox() {
  const box = document.getElementById("pv-lightbox");
  const el = (id) => document.getElementById(`pv-lb-${id}`);
  const state = { items: [], index: 0 };
  const show = (i) => {
    state.index = i;
    const thumb = state.items[i];
    el("img").src = thumb.dataset.full;
    el("img").alt = thumb.querySelector("img")?.alt || "";
    el("name").textContent = thumb.dataset.name || "image";
    el("meta").textContent = thumb.dataset.meta || "";
    el("count").textContent = `${i + 1} / ${state.items.length}`;
    el("prev").disabled = i === 0;
    el("next").disabled = i === state.items.length - 1;
    el("strip").innerHTML = state.items.map((t, n) =>
      `<button class="th-thumb" type="button" data-index="${n}" ${n === i ? 'aria-current="true"' : ""}><img src="${t.dataset.full}" alt=""></button>`).join("");
  };
  const step = (d) => {
    const next = state.index + d;
    if (next >= 0 && next < state.items.length) show(next);
  };
  document.addEventListener("click", (e) => {
    const thumb = e.target.closest(".th-thumb[data-full]");
    if (!thumb || box.contains(thumb) || e.target.closest(".th-thumb-remove")) return;
    const gallery = thumb.closest("[data-gallery]");
    state.items = gallery ? [...gallery.querySelectorAll(".th-thumb[data-full]")] : [thumb];
    show(state.items.indexOf(thumb));
    box.showModal();
  });
  el("strip").addEventListener("click", (e) => {
    const pick = e.target.closest("[data-index]");
    if (pick) show(Number(pick.dataset.index));
  });
  el("prev").addEventListener("click", () => step(-1));
  el("next").addEventListener("click", () => step(1));
  el("close").addEventListener("click", () => box.close());
  box.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft") step(-1);
    if (e.key === "ArrowRight") step(1);
  });
  box.addEventListener("click", (e) => {
    if (e.target === box || e.target.classList.contains("th-lightbox-stage")) box.close();
  });
}

function wireSessionToggle() {
  let open = true;
  try { open = localStorage.getItem("th-preview-session") !== "closed"; } catch (e) { console.warn("panel state not read", e); }
  const apply = () => {
    document.querySelectorAll(".th-convo-layout:not([data-pane])").forEach((layout) => {
      layout.dataset.rail = open ? "" : "closed";
      if (open) delete layout.dataset.rail;
    });
    document.querySelectorAll(".th-rail-toggle").forEach((btn) => btn.setAttribute("aria-expanded", String(open)));
  };
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".th-rail-toggle")) return;
    open = !open;
    try { localStorage.setItem("th-preview-session", open ? "open" : "closed"); } catch (err) { console.warn("panel state not saved", err); }
    apply();
  });
  apply();
}

function wireSettings() {
  document.querySelectorAll(".th-switch").forEach((sw) => sw.addEventListener("click", () => {
    sw.setAttribute("aria-checked", String(sw.getAttribute("aria-checked") !== "true"));
  }));
  document.querySelectorAll("[data-pv-stepper]").forEach((stepper) => {
    const out = stepper.querySelector("output");
    const [less, plus] = stepper.querySelectorAll("button");
    const set = (n) => {
      out.textContent = String(n);
      less.disabled = n <= 1;
      plus.disabled = n >= 50;
    };
    less.addEventListener("click", () => set(Math.max(1, Number(out.textContent) - 1)));
    plus.addEventListener("click", () => set(Math.min(50, Number(out.textContent) + 1)));
  });
}

function token(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(`--th-${name}`).trim();
}

function renderSwatches() {
  document.getElementById("swatches").innerHTML = SWATCHES.map(([name, use]) =>
    `<div class="pv-cell"><div class="pv-swatch" style="background: var(--th-${name})"></div>` +
    `<div>--th-${name}</div><div class="pv-cap">${token(name)} · ${use}</div></div>`).join("");
  const heat = [0, 1, 2, 3, 4].map((n) => `<div title="--th-heat-${n}" style="background: var(--th-heat-${n})"></div>`).join("") + "<span></span><span></span><span></span>";
  const ansi = (prefix) => ANSI.map((c) => `<div title="--th-ansi-${prefix}${c}" style="background: var(--th-ansi-${prefix}${c})"></div>`).join("");
  document.getElementById("ramps").innerHTML = heat + ansi("") + ansi("bright-");
}

function setTheme(theme) {
  if (theme === "light") document.documentElement.dataset.theme = "light";
  else delete document.documentElement.dataset.theme;
  try { localStorage.setItem("th-preview-theme", theme); } catch (e) { console.warn("theme not saved", e); }
  renderSwatches();
}

function startSpinners() {
  const spinners = () => document.querySelectorAll(".th-spin");
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    spinners().forEach((s) => { s.textContent = "⠿"; });
    return;
  }
  let frame = 0;
  const tick = () => {
    frame = (frame + 1) % SPIN.length;
    spinners().forEach((s) => { s.textContent = SPIN[frame]; });
  };
  tick();
  setInterval(tick, 80);
}

function init() {
  renderClones();
  document.querySelectorAll("[data-pv-sidebar]").forEach(renderSidebar);
  document.querySelectorAll("[data-pv-status]").forEach(renderStatus);
  renderStats();
  wireHeatTip();
  renderTrace();
  renderRepos();
  wireLightbox();
  wireSessionToggle();
  wireSettings();
  let saved = "dark";
  try { saved = localStorage.getItem("th-preview-theme") || "dark"; } catch (e) { console.warn("theme not read", e); }
  document.querySelectorAll('input[name="pv-theme"]').forEach((input) => {
    input.checked = input.value === saved;
    input.addEventListener("change", () => setTheme(input.value));
  });
  setTheme(saved);
  startSpinners();
}

init();
