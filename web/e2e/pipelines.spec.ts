import { expect, test, type Page } from "@playwright/test";

async function openPipelines(page: Page): Promise<void> {
  await page.goto("/agents");
  await page.getByRole("radiogroup", { name: "section" }).getByText("pipelines").click();
  await expect(page.getByRole("button", { name: "new pipeline" })).toBeVisible();
}

async function shot(page: Page, name: string): Promise<void> {
  await page.screenshot({ path: test.info().outputPath(`${name}.png`), fullPage: true });
}

async function createRefusedThenValid(page: Page, name: string): Promise<void> {
  await openPipelines(page);
  await page.getByRole("button", { name: "new pipeline" }).click();
  const editor = page.getByRole("textbox", { name: "pipeline toml" });
  await editor.fill(`name = "${name}"\nwork = "build"\n\n[[step]]\nid = "build"\nagent = "ghost-agent"\n`);
  await page.getByRole("button", { name: "save" }).click();
  const errors = page.getByRole("list", { name: "pipeline errors" });
  await expect(errors).toContainText("ghost-agent");
  await shot(page, `${name}-refused`);
  await editor.fill(`name = "${name}"\nwork = "build"\n\n[[step]]\nid = "build"\nagent = "builder"\n`);
  await expect(errors).toBeHidden();
  await page.getByRole("button", { name: "save" }).click();
  await expect(page.getByRole("region", { name })).toBeVisible();
  await expect(page.getByRole("button", { name: new RegExp(`^${name}`) })).toBeVisible();
  await shot(page, `${name}-saved`);
}

async function deleteIt(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: "delete", exact: true }).click();
  await page.getByRole("button", { name: "confirm delete" }).click();
  await expect(page.getByRole("button", { name: new RegExp(`^${name}`) })).toBeHidden();
}

test("create, refuse with field errors, fix and delete a pipeline at desktop width", async ({ page }) => {
  await createRefusedThenValid(page, "e2e-wide");
  await deleteIt(page, "e2e-wide");
});

test.describe("at 400px", () => {
  test.use({ viewport: { width: 400, height: 800 } });

  test("create, refuse with field errors, fix and delete a pipeline", async ({ page }) => {
    await createRefusedThenValid(page, "e2e-narrow");
    await expect(page.locator(".th-pipeline-source")).toBeVisible();
    const fits = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
    expect(fits).toBe(true);
    await deleteIt(page, "e2e-narrow");
  });
});
