import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";

const MARKET = "skill-market";
const PLUGIN = "skillful";
const ID = `${PLUGIN}@${MARKET}`;

function makeMarketplace(): string {
  const dir = mkdtempSync(join(tmpdir(), "townhall-skill-market."));
  const plugin = join(dir, "plugins", PLUGIN);
  mkdirSync(join(dir, ".claude-plugin"), { recursive: true });
  mkdirSync(join(plugin, ".claude-plugin"), { recursive: true });
  mkdirSync(join(plugin, "skills", "guide"), { recursive: true });
  mkdirSync(join(plugin, "commands"), { recursive: true });
  writeFileSync(
    join(dir, ".claude-plugin", "marketplace.json"),
    JSON.stringify({
      name: MARKET,
      owner: { name: "e2e" },
      plugins: [{ name: PLUGIN, description: "a skills only plugin", source: `./plugins/${PLUGIN}` }],
    }),
  );
  writeFileSync(join(plugin, ".claude-plugin", "plugin.json"), JSON.stringify({ name: PLUGIN, version: "1.0.0" }));
  writeFileSync(join(plugin, "skills", "guide", "SKILL.md"), "---\nname: guide\ndescription: guides\n---\nbe guided\n");
  writeFileSync(join(plugin, "commands", "go.md"), "go\n");
  const git = (...args: string[]) => execFileSync("git", args, { cwd: dir, stdio: "ignore" });
  git("init", "-q", "-b", "main");
  git("add", "-A");
  git("-c", "user.name=e2e", "-c", "user.email=e2e@example.com", "commit", "-q", "-m", "init");
  return dir;
}

async function shot(page: Page, name: string): Promise<void> {
  await page.screenshot({ path: test.info().outputPath(`${name}.png`), fullPage: true });
}

async function fitsWidth(page: Page): Promise<boolean> {
  return page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
}

async function installFromFolder(page: Page, dir: string): Promise<void> {
  await page.goto("/plugins");
  const markets = page.getByRole("region", { name: "marketplaces" });
  await markets.getByRole("textbox", { name: "marketplace link, owner/repo or folder" }).fill(dir);
  await markets.getByRole("button", { name: "add marketplace" }).click();
  await expect(markets.getByRole("article", { name: MARKET, exact: true })).toBeVisible();
  await page.getByRole("button", { name: `install ${PLUGIN}` }).click();
  const dialog = page.getByRole("dialog", { name: `install ${PLUGIN}` });
  await expect(dialog.getByRole("group", { name: /adds skills and slash commands/ })).toContainText("skill guide");
  await expect(dialog).toContainText("slash command /go");
  await shot(page, "prompt");
  await dialog.getByRole("button", { name: "install and enable" }).click();
  await expect(dialog).toBeHidden();
}

async function chooseOnPluginPage(page: Page): Promise<void> {
  const row = page.getByRole("region", { name: "installed plugins" }).getByRole("article", { name: ID });
  await expect(row).toContainText("adds 1 skill, 1 command");
  await row.getByText(/^used by/).click();
  await row.getByRole("checkbox", { name: "builder" }).click();
  await expect(row.getByRole("checkbox", { name: "builder" })).toBeChecked();
  await expect(row.getByText("used by 1 agent type")).toBeVisible();
  await shot(page, "plugin-page");
}

async function seeOnAgentPage(page: Page): Promise<void> {
  await page.goto("/agents");
  await page.getByRole("button", { name: /^builder/ }).click();
  const picker = page.getByRole("list", { name: "plugins this type uses" });
  await expect(picker.getByRole("checkbox", { name: PLUGIN })).toBeChecked();
  await expect(picker).toContainText("1 skill, 1 command");
  await picker.scrollIntoViewIfNeeded();
  await shot(page, "agent-page");
  await page.getByText("toml", { exact: true }).click();
  await expect(page.locator(".th-agent-source")).toContainText(`plugins = ["${ID}"]`);
}

async function untickOnAgentPage(page: Page): Promise<void> {
  const picker = page.getByRole("list", { name: "plugins this type uses" });
  await picker.getByRole("checkbox", { name: PLUGIN }).click();
  await expect(picker.getByRole("checkbox", { name: PLUGIN })).not.toBeChecked();
  await expect(page.locator(".th-agent-source")).not.toContainText("plugins =");
}

async function cleanUp(page: Page): Promise<void> {
  await page.goto("/plugins");
  await page.getByRole("button", { name: `uninstall ${PLUGIN}` }).click();
  await page.getByRole("button", { name: "confirm uninstall" }).click();
  await expect(page.getByRole("region", { name: "installed plugins" }).getByRole("article", { name: ID })).toBeHidden();
  await page.getByRole("button", { name: `remove ${MARKET}` }).click();
  await expect(page.getByRole("article", { name: MARKET, exact: true })).toBeHidden();
}

async function flow(page: Page): Promise<void> {
  await installFromFolder(page, makeMarketplace());
  await chooseOnPluginPage(page);
  await seeOnAgentPage(page);
  await untickOnAgentPage(page);
  await cleanUp(page);
}

test("install a skills plugin and choose it for a type at desktop width", async ({ page }) => {
  await flow(page);
});

test.describe("at 400px", () => {
  test.use({ viewport: { width: 400, height: 800 } });

  test("install a skills plugin and choose it for a type", async ({ page }) => {
    await installFromFolder(page, makeMarketplace());
    await chooseOnPluginPage(page);
    expect(await fitsWidth(page)).toBe(true);
    await seeOnAgentPage(page);
    expect(await fitsWidth(page)).toBe(true);
    await untickOnAgentPage(page);
    await cleanUp(page);
  });
});
