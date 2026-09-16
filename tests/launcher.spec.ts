import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openLauncher(page: Page) {
  // A reused Vite server can append an HMR timestamp to the entry URL.
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        body: `import "/tests/mock-backend.ts";\n${await response.text()}`,
      });
    },
  );
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

test("opens and reveals files by ID, and refreshes files with the mode shortcut", async ({
  page,
}) => {
  await openLauncher(page);
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("files");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute(
    "placeholder",
    "Search filenames and paths...",
  );
  await expect(page.locator(".list-count")).toHaveText("1 file indexed");
  await expect(page.locator(".result-title")).toHaveText("Launch notes.md");
  await input.press("Enter");
  await input.press("Meta+Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "file:/Documents/Launch notes.md", action: "open" },
      },
      {
        command: "execute_action",
        payload: { id: "file:/Documents/Launch notes.md", action: "reveal" },
      },
    ]);
  await input.press("Meta+r");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "refresh_files",
          ).length,
      ),
    )
    .toBe(1);
  await page.keyboard.press("Meta+k");
  await expect(page.getByRole("menuitem", { name: "Open file" })).toBeFocused();
  await expect(
    page.getByRole("menuitem", { name: "Show in folder" }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", { name: "Refresh files" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.screenshot({ path: "test-results/files.png" });
});

test("keeps results usable during a file scan and shows scan warnings and empty results", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.fileIndexing = true;
    return window.__launcherTest.emit("files-changed", null);
  });
  await expect(page.locator(".query-hint")).toContainText("Scanning files...");
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("missing");
  await expect(
    page.getByRole("heading", { name: "No results yet" }),
  ).toBeVisible();
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("");
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("files");
  await expect(page.locator(".list-count")).toHaveText("Scanning files...");
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__launcherTest.fileIndexing = false;
    window.__launcherTest.fileWarning =
      "File scan skipped 1 item. Permission denied.";
    return window.__launcherTest.emit("files-changed", null);
  });
  await expect(page.getByRole("alert")).toContainText("Permission denied");
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("missing");
  await expect(
    page.getByRole("heading", { name: "No files found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("apps");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("previews plain text, copies by ID, and deletes without hiding the launcher", async ({
  page,
}) => {
  await openLauncher(page);
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("clipboard");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "<script>literal text</script>",
  );
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "  Keep the original spacing. 🚀",
  );
  await page.screenshot({ path: "test-results/clipboard.png" });
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "clipboard:1", action: "copy" },
      },
    ]);
  await input.press("Meta+Backspace");
  await expect(page.locator(".result-title")).toHaveText("Project link");
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  await expect
    .poll(async () => (await actions(page)).at(-1))
    .toEqual({
      command: "execute_action",
      payload: { id: "clipboard:1", action: "delete" },
    });
  await expect(input).toBeFocused();
});

test("ignores late clipboard previews and keeps the layout usable at narrow widths", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.slowPreview = true;
  });
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("clipboard");
  await expect(page.locator(".result-row")).toHaveCount(2);
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .press("ArrowDown");
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  await page.waitForTimeout(300);
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  for (const width of [320, 720]) {
    await page.setViewportSize({ width, height: 550 });
    await expect(page.getByLabel("Saved clipboard text")).toBeInViewport();
    await expect(
      page.getByRole("button", { name: "Copy text", exact: true }),
    ).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }
});

test("clear requires confirmation, keeps Cancel focused, and reports storage errors", async ({
  page,
}) => {
  await openLauncher(page);
  await page
    .getByRole("combobox", { name: "Search mode" })
    .selectOption("clipboard");
  const clear = page.getByRole("button", {
    name: "Clear history",
    exact: true,
  });
  await clear.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".result-row")).toHaveCount(2);
  await clear.click();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.evaluate(() => {
    window.__launcherTest.rejectClear = true;
  });
  await clear.click();
  await dialog.getByRole("button", { name: "Clear history" }).click();
  await expect(dialog.getByRole("alert")).toHaveText(
    "Error: Could not delete clipboard history.",
  );
  await expect(page.locator(".result-row")).toHaveCount(2);
  await page.evaluate(() => {
    window.__launcherTest.rejectClear = false;
  });
  await dialog.getByRole("button", { name: "Clear history" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "No saved clipboard text" }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
});

test("shows a storage warning while search and launch remain available", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.storageError =
      "Usage history could not be saved. Ranking changes will be lost when TinyDash quits.";
    return window.__launcherTest.emit("usage-changed", null);
  });
  await expect(page.getByRole("alert")).toContainText(
    "Usage history could not be saved",
  );
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    ]);
});

test("refreshes Rust ranking when the launcher preserves the query", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("app");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Finder");
  await page.evaluate(() => {
    window.__launcherTest.usedAppFirst = true;
    return window.__launcherTest.emit("launcher-opened", false);
  });
  await expect(input).toHaveValue("app");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Safari");
  await expect(input).toBeFocused();
});

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
