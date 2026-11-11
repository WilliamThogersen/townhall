import { expect, test, type Page } from "@playwright/test";

async function openHomeWithoutTextFocus(page: Page): Promise<void> {
  await page.goto("/");
  const composer = page.getByRole("textbox", { name: "Describe the work" });
  await expect(composer).toBeFocused();
  await composer.blur();
}

test("? opens the shortcut overlay and esc closes it", async ({ page }) => {
  await openHomeWithoutTextFocus(page);
  const help = page.getByRole("dialog", { name: "keyboard shortcuts" });
  await expect(help).toBeHidden();
  await page.keyboard.press("?");
  await expect(help).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(help).toBeHidden();
});

test.describe("at 400px", () => {
  test.use({ viewport: { width: 400, height: 800 } });

  test("m opens the menu drawer and esc closes it", async ({ page }) => {
    await openHomeWithoutTextFocus(page);
    const menu = page.getByRole("button", { name: "menu" });
    const shell = page.locator(".th-shell");
    await expect(shell).toHaveAttribute("data-view", "main");
    await expect(menu).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("m");
    await expect(shell).toHaveAttribute("data-view", "side");
    await expect(menu).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("navigation", { name: "townhall" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(shell).toHaveAttribute("data-view", "main");
  });
});
