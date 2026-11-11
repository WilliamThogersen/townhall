const task = (name, title, prompt, after = []) => ({ name, title, prompt, after });

export const TURNS = [
  {
    match: "redirects forever",
    title: "login redirect loop",
    context: 41200,
    say: "Your network screenshot shows it: `/login` answers 302 to `/`, and `/` answers 302 back, twenty times. The cause is in `src/auth/redirect.ts`: an expired session is sent to /login without clearing the cookie.\n\nThree tasks. logout-cookie waits for fix-login, since both touch the session:",
    tasks: [
      task("fix-login", "clear the session before redirecting", "An expired session is sent to /login without clearing it, so / sends the browser straight back and the two redirect forever. In src/auth/redirect.ts clear the session before returning /login.\n\nDone when /login loads once after the session expires and npm test passes."),
      task("auth-tests", "cover expiry, logout and the redirect target", "Add tests in src/auth for an expired session, for logout and for the redirect target, so the loop cannot come back.\n\nDone when the new tests fail on main and pass with the fix."),
      task("logout-cookie", "clear the cookie on logout", "Logout leaves the session cookie behind. Clear it in src/auth/session.ts when someone logs out.", ["fix-login"]),
    ],
  },
  {
    match: "session ttl from config",
    context: 56300,
    say: "Two more, each on its own branch: session-ttl reads the ttl from `src/config.ts` instead of the hard-coded 30 minutes, and redirect-guard sends people who are signed in from /login to /.",
    tasks: [
      task("redirect-guard", "stop /login redirecting signed-in users", "Someone who is signed in and opens /login should land on /. Add the check to src/auth/guard.ts.\n\nDone when the guard has a test and npm run typecheck passes."),
      task("session-ttl", "read the session ttl from config", "The session ttl is hard-coded to 30 minutes in src/auth/session.ts. Read it from src/config.ts, keeping 30 as the default."),
    ],
  },
  {
    match: "status flag",
    title: "status flag",
    context: 14200,
    say: "One task for the status flag in townhall:",
    tasks: [task("status-flag", "add a status flag to the cli", "Add a --status flag to the townhall cli that prints whether the daemon runs.\n\nDone when the flag is covered by a test.")],
  },
  {
    match: "vite 8",
    title: "upgrade to vite 8",
    hang: true,
  },
  {
    match: "every third run",
    title: "flaky ci on main",
    context: 23800,
    say: "The smoke test waits a fixed 5 s for the server, and a cold build cache makes the first start slower than that. Two tasks:",
    tasks: [
      task("e2e-smoke", "wait for the server in the smoke test", "e2e/smoke.spec.ts waits a fixed 5 s before it opens the page. Wait for the server to answer instead.\n\nDone when the smoke test passes ten runs in a row."),
      task("ci-cache", "keep the build cache between ci runs", "Every ci run builds from a cold cache. Cache node_modules/.vite and the npm cache in .github/workflows/ci.yml, keyed by the lockfile."),
    ],
  },
  {
    match: "p95 is 4",
    title: "search api is slow under load",
    context: 88100,
    say: "`src/search/query.ts` builds the filter with a leading wildcard (`ilike '%term%'`), so Postgres never uses the index. Three tasks, side by side:",
    tasks: [
      task("search-index", "add a trigram index for product search", "Product search filters with ilike '%term%', which no btree index serves. Add a pg_trgm index on products.name in a new migration."),
      task("load-test", "load test search at 200 requests a second", "Write a k6 script in load/search.js that searches at 200 requests a second for a minute and reports p95."),
      task("query-plan", "explain the slow search queries", "Run explain analyze on the three slowest search queries and write down what the planner does."),
    ],
  },
  {
    match: "slow query log",
    context: 97400,
    say: "Thanks. The log matches the plan: 92% of the time is a sequential scan on `products`, which is what search-index fixes.",
  },
  {
    match: "unused i18n keys",
    title: "i18n keys cleanup",
    context: 18400,
    say: "312 keys in `locales/en.json`, 41 of them unused. One task removes them from en and da together, so the two files stay in step:",
    tasks: [task("i18n-keys", "remove unused i18n keys from en and da", "Find the keys in locales/en.json that no source file uses and remove them from en.json and da.json.")],
  },
  {
    match: "whole kroner",
    title: "checkout coupon rounding",
    context: 21000,
    say: "`src/cart/total.ts` rounds after the coupon is applied, so 12.50 off becomes 12. One task:",
    tasks: [task("coupon-rounding", "round coupon discounts to øre", "Coupon discounts are rounded to whole kroner in src/cart/total.ts. Round to øre instead and cover 12.50 off in the cart tests.")],
  },
  {
    match: "dark mode contrast",
    title: "dark mode contrast",
    context: 15800,
    say: [
      "## Contrast on the dark theme",
      "",
      "Both text tokens pass **WCAG AA** for body text, so nothing needs to change.",
      "",
      "| Token | Color | On `--th-bg` | AA body |",
      "|---|---|---:|---|",
      "| `--th-fg-muted` | `#a8a6a0` | 7.1:1 | pass |",
      "| `--th-fg-faint` | `#8a8882` | 4.6:1 | pass, close to the 4.5 line |",
      "",
      "Worth watching:",
      "",
      "- faint text on `--th-surface-3` drops to 4.2:1",
      "- keep faint for times and hints, never for body copy",
      "",
      "> The light theme was not checked yet.",
    ].join("\n"),
  },
  {
    match: "light theme too",
    fail: "API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}",
  },
];

const ATTACHMENT_NOTE = "attachments of this message (pass their ids to create_task):";
const TITLE_WORDS = 6;
const NAME_WORDS = 3;

function ownText(text) {
  const cut = text.indexOf(ATTACHMENT_NOTE);
  return (cut >= 0 ? text.slice(0, cut) : text).trim();
}

const FILLER = new Set(["a", "an", "the", "to", "of", "in", "on", "for", "and", "or", "with", "please"]);

function words(text) {
  return ownText(text)
    .toLowerCase()
    .split(/[^a-z0-9æøå]+/)
    .filter(Boolean);
}

function trimFiller(list) {
  const end = list.findLastIndex((word) => !FILLER.has(word));
  return list.slice(0, end + 1);
}

function letters(count) {
  return Array.from({ length: count }, () => String.fromCharCode(97 + Math.floor(Math.random() * 26))).join("");
}

function taskName(text) {
  const slug = words(text)
    .filter((word) => !FILLER.has(word))
    .map((word) => word.replace(/[^a-z0-9]/g, ""))
    .filter(Boolean)
    .slice(0, NAME_WORDS)
    .join("-")
    .slice(0, 40);
  return `${slug || "task"}-${letters(3)}`;
}

export function fallbackTurn(text) {
  const own = ownText(text);
  const title = trimFiller(words(text).slice(0, TITLE_WORDS)).join(" ").slice(0, 80) || "demo conversation";
  return {
    title,
    context: 12000 + own.length * 4,
    say: "On it. This is the townhall demo: a stand-in answers instead of claude, and its agents only pretend to work.",
    tasks: [task(taskName(text), own.split("\n")[0].slice(0, 80) || "demo task", own || "demo task")],
    noTask: "No repo is settled for this conversation yet, so no task started. Name a repo and ask again.",
  };
}

export function findTurn(text) {
  const own = ownText(text);
  return TURNS.find((turn) => own.includes(turn.match)) ?? fallbackTurn(text);
}
