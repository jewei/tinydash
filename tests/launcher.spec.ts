import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openLauncher(page: Page) {
  await page.route("**/src/index.tsx", async (route) => {
    const response = await route.fetch();
    await route.fulfill({
      response,
      body: `import "/tests/mock-backend.ts";\n${await response.text()}`,
    });
  });
  await page.goto("/");
  await expect(page.getByRole("option")).toHaveCount(8);
}

async function actions(page: Page) {
  return page.evaluate(() =>
    window.__launcherTest.calls.filter(
      (call) => call.command === "execute_action",
    ),
  );
}

test("focuses the query, wraps selection, and launches the selected app", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox");
  await expect(input).toBeFocused();
  await input.press("ArrowUp");
  await expect(page.getByRole("option").last()).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await input.press("ArrowDown");
  await input.press("ArrowDown");
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    ]);
});

test("uses result shortcuts and the reveal action", async ({ page }) => {
  await openLauncher(page);
  await page.keyboard.press("Meta+3");
  await expect.poll(async () => (await actions(page)).length).toBe(1);
  await page.getByRole("combobox").press("Meta+Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-2", action: "launch" } },
      { command: "execute_action", payload: { id: "app-0", action: "reveal" } },
    ]);
});

test("the actions menu keeps native button keyboard behavior and closes with Escape", async ({
  page,
}) => {
  await openLauncher(page);
  const button = page.getByRole("button", { name: "Actions" });
  await button.focus();
  await button.press("Enter");
  await expect(page.getByRole("menu")).toBeVisible();
  await expect(
    page.getByRole("menuitem", { name: "Open application" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-0", action: "reveal" } },
    ]);
  await page.keyboard.press("Meta+k");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(page.getByRole("combobox")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "hide_launcher",
          ).length,
      ),
    )
    .toBe(1);
});

test("ignores late results and prevents launching stale results", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox");
  await input.fill("slow");
  await input.press("Enter");
  expect(await actions(page)).toHaveLength(0);
  await input.fill("sa");
  await expect(page.getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("option")).toContainText("Safari");
  await page.waitForTimeout(300);
  await expect(page.getByRole("option")).toContainText("Safari");
});

test("shows action errors, handles empty results, and ignores IME confirmation", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox");
  await input.dispatchEvent("keydown", {
    key: "Enter",
    isComposing: true,
    bubbles: true,
  });
  expect(await actions(page)).toHaveLength(0);
  await page.evaluate(() => {
    window.__launcherTest.rejectActions = true;
  });
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText(
    "Could not open the application",
  );
  await input.fill("missing");
  await expect(
    page.getByRole("heading", { name: "No applications found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(page.getByRole("option")).toHaveCount(8);
});

test("reopening clears or selects the prior query as configured", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox");
  await input.fill("sa");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(input).toHaveValue("sa");
  await expect(input).toBeFocused();
  expect(
    await input.evaluate((element) => [
      (element as HTMLInputElement).selectionStart,
      (element as HTMLInputElement).selectionEnd,
    ]),
  ).toEqual([0, 2]);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(input).toHaveValue("");
  await expect(page.getByRole("option")).toHaveCount(8);
});

test("the layout fits narrow windows and the desktop window", async ({
  page,
}) => {
  await openLauncher(page);
  for (const width of [320, 375, 414, 720, 768]) {
    await page.setViewportSize({ width, height: 550 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(
      page.getByRole("button", { name: "Actions" }),
    ).toBeInViewport();
  }
  await page.setViewportSize({ width: 720, height: 550 });
  await page.screenshot({ path: "test-results/launcher.png" });
});
