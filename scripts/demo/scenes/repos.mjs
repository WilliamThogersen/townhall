const json = (value) => `${JSON.stringify(value, null, 2)}\n`;
const lines = (...rows) => `${rows.join("\n")}\n`;

const TOWNHALL_FILES = {
  "README.md": lines("# townhall", "", "Plan work in a conversation and let agents do it in their own worktrees."),
  "package.json": json({ name: "townhall", private: true, scripts: { test: "vitest run", typecheck: "tsc --noEmit" } }),
  "src/config.ts": lines("export const config = {", "  port: 7777,", "  home: \"~/.townhall\",", "};"),
  "src/auth/session.ts": lines(
    "export interface Session {",
    "  readonly user: string;",
    "  readonly expired: boolean;",
    "}",
    "",
    "const SESSION_TTL_MINUTES = 30;",
    "",
    "export function clearSession(): void {",
    "  document.cookie = \"session=; max-age=0; path=/\";",
    "}",
    "",
    "export function expiresAt(start: number): number {",
    "  return start + SESSION_TTL_MINUTES * 60_000;",
    "}",
  ),
  "src/auth/redirect.ts": lines(
    "import type { Session } from \"./session\";",
    "",
    "export function redirectFor(session: Session | null, path: string): string | null {",
    "  if (session === null) return path === \"/login\" ? null : \"/login\";",
    "  if (session.expired) return \"/login\";",
    "  return null;",
    "}",
  ),
  "src/auth/guard.ts": lines("import type { Session } from \"./session\";", "", "export function guard(session: Session): boolean {", "  return !session.expired;", "}"),
  "src/auth/redirect.test.ts": lines("import { redirectFor } from \"./redirect\";", "", "test(\"sends a missing session to /login\", () => {", "  expect(redirectFor(null, \"/\")).toBe(\"/login\");", "});"),
};

const SHOP_FILES = {
  "README.md": lines("# shop", "", "The storefront: catalog, search, cart and checkout."),
  "package.json": json({ name: "shop", version: "2.4.0", private: true, scripts: { dev: "vite --port 4100", test: "vitest run" } }),
  "src/search/query.ts": lines(
    "export function productFilter(term: string): string {",
    "  return `name ilike '%${term.replace(/'/g, \"''\")}%'`;",
    "}",
  ),
  "src/cart/total.ts": lines(
    "export function totalAfterCoupon(total: number, coupon: number): number {",
    "  return Math.round(total - coupon);",
    "}",
  ),
  "e2e/smoke.spec.ts": lines("test(\"home page loads\", async ({ page }) => {", "  await page.waitForTimeout(5000);", "  await page.goto(\"/\");", "});"),
  ".github/workflows/ci.yml": lines("name: ci", "on: [push, pull_request]", "jobs:", "  test:", "    runs-on: ubuntu-latest", "    steps:", "      - uses: actions/checkout@v4", "      - run: npm ci", "      - run: npm test"),
};

const ACCESS_FILES = {
  "README.md": lines("# acme-access", "", "Roles, users and single sign-on for the Example Co apps."),
  "locales/en.json": json({ "roles.title": "Roles", "users.invite": "Invite user", "audit.empty": "Nothing happened yet" }),
  "locales/da.json": json({ "roles.title": "Roller", "users.invite": "Inviter bruger", "audit.empty": "Der er ikke sket noget endnu" }),
  "src/i18n.ts": lines("import en from \"../locales/en.json\";", "", "export const t = (key: keyof typeof en): string => en[key];"),
};

const HUB_FILES = {
  "README.md": lines("# data-hub", "", "Nightly imports and exports between the shop and the warehouse."),
  "jobs/export.py": lines("def export_orders(since):", "    return [row for row in fetch_orders(since) if row.status == 'paid']"),
  "jobs/import.py": lines("def import_stock(rows):", "    return {row.sku: row.count for row in rows}"),
  "sql/sessions.sql": lines("create table sessions (id text primary key, started_at timestamptz not null);"),
};

export const REPOS = [
  { name: "townhall", base: "main", weight: 1, ext: "ts", files: TOWNHALL_FILES, areas: ["auth", "agents", "conversations", "repos", "settings", "stats", "terminal", "sidebar"] },
  { name: "shop", base: "develop", weight: 0.9, ext: "ts", files: SHOP_FILES, areas: ["cart", "checkout", "search", "catalog", "orders", "payments", "customers"] },
  { name: "acme-access", base: "staging", weight: 0.5, ext: "ts", files: ACCESS_FILES, areas: ["roles", "users", "audit", "invites", "sso"] },
  { name: "data-hub", base: "main", weight: 0.35, ext: "py", files: HUB_FILES, areas: ["exports", "imports", "schemas", "warehouse"] },
];

export const FIXES = [
  "keep {a} order stable after reload",
  "handle an empty {a} list",
  "stop the {a} form submitting twice",
  "use the shop timezone for {a} dates",
  "round {a} totals once",
  "escape {a} names in exports",
  "retry {a} requests on 503",
  "clear the stale {a} cache on logout",
  "show {a} errors inline",
  "trim whitespace in {a} search",
];

export const FEATURES = [
  "add {a} filters",
  "export {a} to csv",
  "paginate the {a} list",
  "bulk edit {a}",
  "add keyboard shortcuts to {a}",
  "show {a} history",
  "send {a} webhooks",
  "sort {a} by date",
];

export const CHORES = ["test: cover {a} edge cases", "refactor: split {a} helpers", "docs: explain {a} settings"];
