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
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
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
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await input.press("ArrowUp");
  await expect(
    page.getByRole("listbox").getByRole("option").last(),
  ).toHaveAttribute("aria-selected", "true");
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
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .press("Meta+Enter");
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
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
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
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("slow");
  await input.press("Enter");
  expect(await actions(page)).toHaveLength(0);
  await input.fill("sa");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "Safari",
  );
  await page.waitForTimeout(300);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "Safari",
  );
});

test("shows action errors, handles empty results, and ignores IME confirmation", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
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
    page.getByRole("heading", { name: "No results found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
});

test("reopening clears or selects the prior query as configured", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
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
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
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

test("copies calculation results and offers only their supported actions", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("12 * 8");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "96",
  );
  await expect(
    page.getByRole("button", { name: "Copy result", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("menuitem", { name: "Copy result" }),
  ).toBeFocused();
  await expect(
    page.getByRole("menuitem", { name: "Show in folder" }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "calculation:1", action: "copy" },
      },
    ]);
  await page.screenshot({ path: "test-results/calculator.png" });
});

test("changes search mode during a pending query and copies an emoji", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("slow");
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("emoji");
  await expect(input).toBeFocused();
  await expect(
    page.getByRole("listbox").getByRole("option").filter({ hasText: "rocket" }),
  ).toHaveCount(1);
  await expect(page.locator(".emoji-icon")).toHaveText("🚀");
  await page.waitForTimeout(300);
  await expect(page.locator(".result-title")).toHaveText("rocket");
  await page.keyboard.press("Meta+1");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "emoji:🚀", action: "copy" },
      },
    ]);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(page.getByRole("combobox", { name: "Search mode" })).toHaveValue(
    "emoji",
  );
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(page.getByRole("combobox", { name: "Search mode" })).toHaveValue(
    "all",
  );
});

test("shows calculator errors and fits both new result types in a narrow window", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("=1 / 0");
  await expect(page.getByRole("alert")).toContainText("Division by zero");
  await input.press("Enter");
  expect(await actions(page)).toHaveLength(0);
  for (const query of ["12 * 8", ":rocket"]) {
    await input.fill(query);
    await expect(page.locator(".result-row")).toHaveCount(1);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await page.setViewportSize({ width: 320, height: 550 });
    await expect(
      page.getByRole("combobox", { name: "Search mode" }),
    ).toBeInViewport();
    await expect(
      page.getByRole("button", { name: "Actions" }),
    ).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }
  await page.setViewportSize({ width: 720, height: 550 });
  await page.screenshot({ path: "test-results/emoji.png" });
});
