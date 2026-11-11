import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { freeAgentSlots, townhallRepo, type DemoRepo } from "./demo";

const TASK = "status-flag";
const BRANCH = `townhall/${TASK}`;

function git(repo: DemoRepo, args: readonly string[]): string {
  const identity = ["-c", "user.name=e2e", "-c", "user.email=e2e@example.com", "-c", "commit.gpgsign=false"];
  return execFileSync("git", [...identity, ...args], { cwd: repo.path, encoding: "utf8" }).trim();
}

function commitsAhead(repo: DemoRepo): number {
  return Number(git(repo, ["rev-list", "--count", `${repo.baseBranch}..${BRANCH}`]));
}

test("a conversation becomes an agent, asks, and its work merges", async ({ page }) => {
  const repo = townhallRepo();
  await freeAgentSlots();

  await page.goto("/");
  await page.getByRole("textbox", { name: "Describe the work" }).fill("townhall needs a status flag on the cli, add it");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/c\/[^/]+$/);

  const open = page.getByRole("button", { name: `open ${TASK}` });
  await expect(open).toBeVisible();
  await open.click();

  const terminal = page.getByRole("article", { name: `${TASK} terminal` });
  await expect(terminal).toBeVisible();
  await expect(terminal).toContainText("commit the status flag to townhall/status-flag? (y/n)");

  const ask = page.getByRole("group", { name: `${TASK} needs you` });
  await expect(ask).toBeVisible();
  await ask.getByRole("button", { name: "send y", exact: true }).click();
  await expect(page.getByRole("group", { name: `${TASK} asked` })).toBeVisible();
  await expect(terminal).toContainText(`committed to ${BRANCH}. done.`);

  await expect.poll(() => commitsAhead(repo)).toBeGreaterThan(0);
  git(repo, ["merge", "--no-ff", "-q", "-m", `Merge branch '${BRANCH}'`, BRANCH]);

  await expect(page.getByRole("log", { name: "Thread" })).toContainText(`${TASK} merged into ${repo.baseBranch}`);
});
