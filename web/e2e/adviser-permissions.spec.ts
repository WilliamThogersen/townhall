import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";

const QUESTION = [
  "Bash: npm publish --access public",
  "Do you want to proceed?",
  "1. Yes",
  "2. Yes, and don't ask again for npm publish commands",
  "3. No, and tell Claude what to do differently (esc)",
].join("\n");

function sql(statements: readonly string[]): void {
  const dir = process.env.TOWNHALL_E2E_DEMO_DIR;
  if (!dir) throw new Error("TOWNHALL_E2E_DEMO_DIR is not set: the demo starts in globalSetup");
  const script = [".timeout 5000", "begin immediate;", ...statements, "commit;"].join("\n");
  execFileSync("sqlite3", ["-bail", join(dir, "home", "townhall.db")], { input: script });
}

const quote = (text: string): string => `'${text.replaceAll("'", "''")}'`;

function allowed(pattern: string, times: number): string[] {
  const command = quote(`Bash: ${pattern.replace(/^Bash\(|( \*)?\)$/g, "")}`);
  return Array.from(
    { length: times },
    (_, i) =>
      `insert into permission_decisions (task, agent_type, command, pattern, verdict, reason, created_at) values ('fix-login', 'builder', ${command}, ${quote(pattern)}, 'allow', 'read-only, inside its worktree', '2026-11-13T10:0${i}:00.000Z');`,
  );
}

function seedConversation(id: string): void {
  const data = JSON.stringify({
    agent: "fix-login",
    question: QUESTION,
    choices: ["1", "2", "3"],
    advice: "it publishes to a registry, only you can decide that",
  });
  sql([
    `insert into conversations (id, session_id, title, status, last_entry_id, created_at, updated_at) values (${quote(id)}, ${quote(`session-${id}`)}, 'publish the package', 'yourTurn', 1, '2026-11-13T10:00:00.000Z', '2026-11-13T10:05:00.000Z');`,
    `insert into events (conversation_id, id, kind, data, at) values (${quote(id)}, 1, 'needsYou', ${quote(data)}, '2026-11-13T10:05:00.000Z');`,
  ]);
}

async function shot(page: Page, name: string): Promise<void> {
  await page.screenshot({ path: test.info().outputPath(`${name}.png`), fullPage: true });
}

async function fitsWidth(page: Page): Promise<boolean> {
  return page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
}

async function seesSuggestion(page: Page, pattern: string): Promise<void> {
  await page.goto("/settings");
  const group = page.getByRole("region", { name: "adviser" }).or(page.locator("section.th-setting-group", { hasText: "adviser" }));
  await expect(group.getByRole("switch", { name: "Adviser answers permission prompts" })).toBeChecked();
  await expect(group.getByRole("radiogroup", { name: "Adviser time limit" })).toBeVisible();
  const row = page.getByRole("group", { name: `Suggestion ${pattern} for builder` });
  await expect(row).toBeVisible();
  await expect(row).toContainText("allowed 3 times for builder");
  await expect(page.getByRole("list", { name: "Recent permission decisions" })).toContainText("read-only, inside its worktree");
  await row.scrollIntoViewIfNeeded();
}

async function acceptsSuggestion(page: Page, pattern: string): Promise<void> {
  const row = page.getByRole("group", { name: `Suggestion ${pattern} for builder` });
  await row.getByRole("button", { name: "add to builder's tools" }).click();
  await expect(row).toBeHidden();
  await page.goto("/agents");
  await page.getByRole("button", { name: /^builder/ }).click();
  await page.getByText("toml", { exact: true }).click();
  await expect(page.locator(".th-agent-source")).toContainText(pattern);
}

async function seesAdviceOnTheNeedsYouEntry(page: Page, id: string): Promise<void> {
  await page.goto(`/c/${id}`);
  const ask = page.getByRole("group", { name: "fix-login needs you" });
  await expect(ask).toBeVisible();
  await expect(ask.locator(".th-ask-advice")).toHaveText("it publishes to a registry, only you can decide that");
  await expect(ask.getByText("adviser", { exact: true })).toBeVisible();
  await expect(ask.getByText("Do you want to proceed?")).toBeVisible();
}

test("the adviser's reasoning, its settings and a suggestion are visible at desktop width", async ({ page }) => {
  const pattern = "Bash(git status *)";
  sql(allowed(pattern, 3));
  seedConversation("e2e-advice-desktop");
  await seesSuggestion(page, pattern);
  await shot(page, "settings-desktop");
  expect(await fitsWidth(page)).toBe(true);
  await acceptsSuggestion(page, pattern);
  await seesAdviceOnTheNeedsYouEntry(page, "e2e-advice-desktop");
  await shot(page, "needs-you-desktop");
});

test.describe("at 400px", () => {
  test.use({ viewport: { width: 400, height: 800 } });

  test("the adviser's reasoning, its settings and a suggestion are visible", async ({ page }) => {
    const pattern = "Bash(cargo test *)";
    sql(allowed(pattern, 3));
    seedConversation("e2e-advice-narrow");
    await seesSuggestion(page, pattern);
    await shot(page, "settings-narrow");
    expect(await fitsWidth(page)).toBe(true);
    await seesAdviceOnTheNeedsYouEntry(page, "e2e-advice-narrow");
    await shot(page, "needs-you-narrow");
    expect(await fitsWidth(page)).toBe(true);
  });
});
