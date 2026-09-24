import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openLauncher(
  page: Page,
  options: { firstUse?: boolean; hidden?: boolean } = {},
) {
  if (options.firstUse || options.hidden) {
    await page.addInitScript(({ firstUse, hidden }) => {
      if (firstUse && !sessionStorage.getItem("tinydash.test.firstUseSet")) {
        localStorage.setItem(
          "tinydash.test.settings",
          JSON.stringify({
            clipboardHistoryDecided: false,
            clipboardHistoryEnabled: false,
          }),
        );
        sessionStorage.setItem("tinydash.test.firstUseSet", "true");
      }
      if (hidden) localStorage.setItem("tinydash.test.initialVisible", "false");
    }, options);
  }
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
  await expect(page.locator(".list-count")).toHaveText("0 results");
}

async function selectCategory(page: Page, name: string) {
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name, exact: true })
    .click();
}

async function actions(page: Page) {
  return page.evaluate(() => window.__launcherTest.calls);
}

test("empty Clipboard selects the newest copy while pinned history stays first", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page.getByRole("listbox").getByRole("option");
  await input.fill("Project link");
  await page
    .getByRole("button", { name: "Pin to Clipboard", exact: true })
    .click();
  await input.fill("");
  await expect(rows).toHaveCount(2);
  await expect(rows.first()).toContainText("Project link");
  await expect(rows.nth(1)).toContainText("Meeting notes");
  await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
});

for (const navigate of [false, true]) {
  test(`delayed clipboard capture ${navigate ? "keeps the user's selection" : "selects the newest copy"}`, async ({
    page,
  }) => {
    await openLauncher(page);
    await page.evaluate(() => {
      window.__launcherTest.pins.clipboard = ["clipboard:2"];
      window.__launcherTest.clipboardDeleted = ["clipboard:1"];
    });
    await selectCategory(page, "Clipboard");
    const input = page.getByRole("combobox", { name: "Search TinyDash" });
    const rows = page.getByRole("listbox").getByRole("option");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("Project link");
    await expect(rows.first()).toHaveAttribute("aria-selected", "true");
    if (navigate) await input.press("ArrowDown");
    await page.evaluate(async () => {
      window.__launcherTest.clipboardDeleted = [];
      await window.__launcherTest.emit("clipboard-changed");
    });
    await expect(rows).toHaveCount(2);
    await expect(rows.first()).toContainText("Project link");
    await expect(rows.nth(navigate ? 0 : 1)).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await input.press("Enter");
    expect(
      (await actions(page))
        .filter((call) => call.command === "execute_action")
        .pop(),
    ).toMatchObject({
      payload: { id: navigate ? "clipboard:2" : "clipboard:1", action: "copy" },
    });
  });
}

test("clear unpinned keeps All and Clipboard pins, then clear all removes them", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page.getByRole("listbox").getByRole("option");
  await input.fill("Meeting notes");
  await page
    .getByRole("button", { name: "Pin to Clipboard", exact: true })
    .click();
  await page.getByRole("button", { name: "Pin to All", exact: true }).click();
  await input.fill("");
  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Clear unpinned history", exact: true })
    .click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Clear unpinned", exact: true })
    .click();
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText("Meeting notes");
  await selectCategory(page, "All");
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText("Meeting notes");
  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Clear all clipboard history", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Clear all history", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "What will you do next?" }),
  ).toBeVisible();
});

test("typed queries select the best result and refresh keeps the selected result", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page.getByRole("listbox").getByRole("option");
  await input.fill("sa");
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText("Safari");
  await expect(rows.first()).toHaveAttribute("aria-selected", "true");
  await selectCategory(page, "Apps");
  await input.fill("");
  await input.press("ArrowDown");
  await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
  await page.evaluate(() => {
    window.__launcherTest.usedAppFirst = true;
    void window.__launcherTest.emit("apps-changed");
  });
  await expect(rows.first()).toContainText("Safari");
  await expect(rows.first()).toHaveAttribute("aria-selected", "true");
});

test("Actions filtering supports focus, arrows, Enter, Escape, and IME Escape", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await page.getByRole("button", { name: "Actions" }).click();
  const filter = page.getByRole("searchbox", { name: "Search actions" });
  await expect(filter).toBeFocused();
  await filter.fill("Refresh applications");
  await filter.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: /Refresh applications/ }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(filter).toBeHidden();
  await expect(input).toBeFocused();
  await page.getByRole("button", { name: "Actions" }).click();
  await filter.fill("Settings");
  await filter.press("Enter");
  await expect(
    page.getByRole("searchbox", { name: "Search actions" }),
  ).toBeHidden();
  expect(
    (await actions(page)).some((call) => call.command === "open_settings"),
  ).toBe(true);
  await input.dispatchEvent("keydown", {
    key: "Escape",
    code: "Escape",
    isComposing: true,
    bubbles: true,
  });
  expect(
    (await actions(page)).some((call) => call.command === "hide_launcher"),
  ).toBe(false);
  await input.press("Escape");
  expect(
    (await actions(page)).some((call) => call.command === "hide_launcher"),
  ).toBe(true);
});

test("Edit a copy preserves the original and batch copy keeps order and separator", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Edit a copy", exact: true })
    .click();
  const edit = page.getByRole("dialog", { name: "Edit a copy" });
  await expect(edit.getByLabel("Edited clipboard text")).toHaveValue(
    /Meeting notes/,
  );
  const edited = "Meeting notes\nChanged only in the copied value.\n🚀";
  await edit.getByLabel("Edited clipboard text").fill(edited);
  await edit
    .getByRole("button", { name: "Copy edited text", exact: true })
    .click();
  await expect(edit).toBeHidden();
  expect(
    await page.evaluate(() => window.__launcherTest.editedClipboard),
  ).toEqual({
    id: "clipboard:1",
    text: edited,
  });
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Meeting notes");

  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Copy selected entries", exact: true })
    .click();
  const batch = page.getByRole("dialog", { name: "Copy selected entries" });
  const checks = batch.getByRole("checkbox");
  await checks.nth(1).check();
  await checks.nth(0).check();
  await batch.getByRole("combobox").selectOption(", ");
  await batch
    .getByRole("button", { name: "Copy 2 entries", exact: true })
    .click();
  await expect(batch).toBeHidden();
  expect(
    await page.evaluate(() => window.__launcherTest.copiedSelection),
  ).toEqual({
    ids: ["clipboard:2", "clipboard:1"],
    separator: ", ",
  });
});

test("failed clipboard copy keeps the dialog and original entry", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Edit a copy", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Edit a copy" });
  await page.evaluate(() => {
    window.__launcherTest.rejectClipboardCopy = "Could not copy edited text.";
  });
  await dialog.getByLabel("Edited clipboard text").fill("failed copy");
  await dialog
    .getByRole("button", { name: "Copy edited text", exact: true })
    .click();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("alert")).toContainText(
    "Could not copy edited text",
  );
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Meeting notes");
});

test("first-use clipboard choice persists after accept and decline", async ({
  page,
}) => {
  await openLauncher(page, { firstUse: true });
  const choice = page.getByRole("complementary", {
    name: "Clipboard history choice",
  });
  await expect(choice).toBeVisible();
  await choice
    .getByRole("button", { name: "Keep history off", exact: true })
    .click();
  await expect(choice).toBeHidden();
  await page.reload();
  await expect(
    page.getByRole("complementary", { name: "Clipboard history choice" }),
  ).toBeHidden();
  expect(
    await page.evaluate(
      () => window.__launcherTest.settings.clipboardHistoryDecided,
    ),
  ).toBe(true);
});

test("first-use enable choice persists after reload", async ({ page }) => {
  await openLauncher(page, { firstUse: true });
  const choice = page.getByRole("complementary", {
    name: "Clipboard history choice",
  });
  await choice
    .getByRole("button", { name: "Enable history", exact: true })
    .click();
  await expect(choice).toBeHidden();
  await page.reload();
  await expect(
    page.getByRole("complementary", { name: "Clipboard history choice" }),
  ).toBeHidden();
  expect(
    await page.evaluate(
      () => window.__launcherTest.settings.clipboardHistoryEnabled,
    ),
  ).toBe(true);
});

test("Hide application action records the hidden preference", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  await page.getByRole("button", { name: "Actions" }).click();
  await page
    .getByRole("menuitem", { name: "Hide application", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__launcherTest.calls.find(
          (call) => call.command === "set_app_preference",
        ),
      ),
    )
    .toEqual({
      command: "set_app_preference",
      payload: { id: "app-0", aliases: [], hidden: true },
    });
  await expect(page.getByText("Application hidden.")).toBeVisible();
});

test("launcher category event clears query while an initially hidden launcher stays hidden", async ({
  page,
}) => {
  await openLauncher(page, { hidden: true });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await page.evaluate(
    () => void window.__launcherTest.emit("launcher-category", "clipboard"),
  );
  await expect(input).toHaveValue("");
  await expect(
    page
      .getByRole("navigation", { name: "Search categories" })
      .getByRole("button", { name: "Clipboard", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    (await actions(page)).filter((call) => call.command === "search"),
  ).toHaveLength(0);
});

test("new launcher panels fit supported widths and appearances", async ({
  page,
}) => {
  await openLauncher(page, { firstUse: true });
  const choice = page.getByRole("complementary", {
    name: "Clipboard history choice",
  });
  const expectNoOverflow = async () => {
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  };
  for (const width of [320, 375, 414, 768]) {
    await page.setViewportSize({ width, height: 640 });
    await expect(choice).toBeVisible();
    await expectNoOverflow();
    await page.screenshot({
      path: test.info().outputPath(`tinycast-first-use-${width}.png`),
    });
  }
  for (const appearance of [
    "Dark",
    "Sage",
    "Rose",
    "Ink",
    "Compact",
    "Light",
  ]) {
    await page.setViewportSize({ width: 320, height: 640 });
    await page.getByRole("button", { name: "Actions" }).click();
    await page
      .getByRole(
        appearance === "Compact" ? "menuitemcheckbox" : "menuitemradio",
        { name: appearance, exact: true },
      )
      .click();
    await page.getByRole("button", { name: "Actions" }).click();
    const filter = page.getByRole("searchbox", { name: "Search actions" });
    await expect(filter).toBeFocused();
    await expect(
      page.getByRole(
        appearance === "Compact" ? "menuitemcheckbox" : "menuitemradio",
        { name: appearance, exact: true },
      ),
    ).toHaveAttribute("aria-checked", "true");
    await expectNoOverflow();
    await page.screenshot({
      path: test
        .info()
        .outputPath(`tinycast-actions-${appearance.toLowerCase()}.png`),
    });
    await page.keyboard.press("Escape");
  }
  await choice
    .getByRole("button", { name: "Enable history", exact: true })
    .click();
  await selectCategory(page, "Clipboard");
  for (const width of [320, 768]) {
    await page.setViewportSize({ width, height: 640 });
    await page.getByRole("button", { name: "Actions" }).click();
    await page
      .getByRole("menuitem", { name: "Edit a copy", exact: true })
      .click();
    await expect(
      page.getByRole("dialog", { name: "Edit a copy" }),
    ).toBeVisible();
    await expectNoOverflow();
    await page.screenshot({
      path: test.info().outputPath(`tinycast-edit-copy-${width}.png`),
    });
    await page
      .getByRole("dialog", { name: "Edit a copy" })
      .getByRole("button", { name: "Cancel", exact: true })
      .click();

    await page.getByRole("button", { name: "Actions" }).click();
    await page
      .getByRole("menuitem", { name: "Copy selected entries", exact: true })
      .click();
    await expect(
      page.getByRole("dialog", { name: "Copy selected entries" }),
    ).toBeVisible();
    await expectNoOverflow();
    await page.screenshot({
      path: test.info().outputPath(`tinycast-combine-copy-${width}.png`),
    });
    await page
      .getByRole("dialog", { name: "Copy selected entries" })
      .getByRole("button", { name: "Cancel", exact: true })
      .click();
  }
});
