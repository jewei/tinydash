import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openSettings(page: Page) {
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
  await page.goto("/?view=settings");
  await expect(page.getByRole("button", { name: "Record new" })).toBeEnabled();
}

for (const editBeforeUpdate of [false, true]) {
  test(`keeps external settings changes with a ${editBeforeUpdate ? "dirty" : "clean"} draft`, async ({
    page,
  }) => {
    await openSettings(page);
    const clearSearch = page.getByRole("switch", {
      name: "Clear the search each time",
    });
    if (editBeforeUpdate) await clearSearch.uncheck();
    await page.evaluate(async () => {
      const state = window.__launcherTest;
      state.settings = {
        ...state.settings,
        appPreferences: {
          "app:/Applications/Safari.app": { aliases: [], hidden: true },
        },
        clipboardHistoryEnabled: false,
        clipboardHistoryDecided: true,
      };
      await state.emit("settings-changed", state.settings);
    });
    if (!editBeforeUpdate) {
      await expect(
        page.getByRole("button", { name: "Save changes" }),
      ).toBeDisabled();
      await clearSearch.uncheck();
    }
    await expect(clearSearch).not.toBeChecked();
    await page.getByRole("button", { name: "Save changes" }).click();
    await expect(
      page.getByText("Changes saved.", { exact: true }),
    ).toBeVisible();
    expect(
      await page.evaluate(() => window.__launcherTest.settings),
    ).toMatchObject({
      clearQueryOnOpen: false,
      appPreferences: {
        "app:/Applications/Safari.app": { aliases: [], hidden: true },
      },
      clipboardHistoryEnabled: false,
      clipboardHistoryDecided: true,
    });
  });
}

test("keeps an edited alias when another window hides apps", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.locator('select[size="6"]').selectOption("app-0");
  await page.getByLabel("Aliases, one per line").fill("work files");
  await page.getByLabel("Aliases, one per line").blur();
  await page.evaluate(async () => {
    const state = window.__launcherTest;
    state.settings = {
      ...state.settings,
      appPreferences: {
        "app-0": { aliases: [], hidden: true },
        "app-1": { aliases: [], hidden: true },
      },
    };
    await state.emit("settings-changed", state.settings);
  });
  await expect(page.getByLabel("Aliases, one per line")).toHaveValue(
    "work files",
  );
  await expect(
    page.getByRole("checkbox", { name: "Hide this app from search" }),
  ).toBeChecked();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.appPreferences),
  ).toEqual({
    "app-0": { aliases: ["work files"], hidden: true },
    "app-1": { aliases: [], hidden: true },
  });
});

test("records, saves, and reloads the launch shortcut and window preferences", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Record new" }).click();
  await expect(page.getByText("Press your new shortcut...")).toBeVisible();
  await page.keyboard.press("Control+Alt+KeyJ");
  await expect(page.getByRole("button", { name: "Record new" })).toBeEnabled();
  await page
    .getByRole("switch", { name: "Clear the search each time" })
    .uncheck();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.shortcut),
  ).toBe("Control+Alt+KeyJ");
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls
        .filter((call) => call.command === "set_shortcut_recording")
        .map((call) => call.payload),
    ),
  ).toEqual([{ recording: true }, { recording: false }]);
  await page.reload();
  await expect(page.locator(".shortcut-keys")).toHaveText("ControlOptionJ");
  await expect(
    page.getByRole("switch", { name: "Clear the search each time" }),
  ).not.toBeChecked();
});

test("invalid recording and shortcut conflicts do not replace saved settings", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Record new" }).click();
  await page.keyboard.press("KeyK");
  await expect(page.getByRole("alert")).toContainText("Hold Control");
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("button", { name: "Save changes" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Record new" }).click();
  await page.keyboard.press("Control+Alt+KeyL");
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings =
      "Could not use this shortcut. It may be in use by another app.";
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText("in use by another app");
  expect(
    await page.evaluate(() => window.__launcherTest.settings.shortcut),
  ).toBe("Control+Shift+Space");
  await page.getByRole("button", { name: "Discard" }).click();
  await expect(page.locator(".shortcut-keys")).toHaveText("ControlShiftSpace");
});

test("saves file, clipboard, and currency preferences across sections", async ({
  page,
}) => {
  await openSettings(page);
  await page
    .getByRole("button", { name: "Clipboard history", exact: true })
    .click();
  await page.getByRole("switch", { name: "Save clipboard history" }).uncheck();
  await page.getByRole("spinbutton", { name: "History limit" }).fill("50");
  await page.getByRole("button", { name: "File search", exact: true }).click();
  await page
    .getByLabel("Search folders", { exact: true })
    .selectOption("custom");
  await page
    .getByLabel("Folder paths", { exact: true })
    .fill("~/Documents\n~/Projects");
  await page
    .getByLabel("Excluded folder names")
    .fill("node_modules\ntarget\n.git");
  await page.getByRole("spinbutton", { name: "File limit" }).fill("20000");
  await page
    .getByRole("switch", { name: "Update files automatically" })
    .uncheck();
  await page.getByRole("button", { name: "Currency", exact: true }).click();
  await page.getByRole("switch").uncheck();
  await page.keyboard.press("Meta+s");
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings),
  ).toMatchObject({
    clipboardHistoryEnabled: false,
    clipboardHistoryLimit: 50,
    fileSearchRoots: ["~/Documents", "~/Projects"],
    fileSearchExcludedDirs: ["node_modules", "target", ".git"],
    fileSearchLimit: 20000,
    fileWatchEnabled: false,
    currencyRatesEnabled: false,
  });
});

test("validates hidden fields and confirms history deletion", async ({
  page,
}) => {
  await openSettings(page);
  await page
    .getByRole("button", { name: "Clipboard history", exact: true })
    .click();
  await page.getByRole("spinbutton", { name: "History limit" }).fill("0");
  await page.getByRole("button", { name: "Currency", exact: true }).click();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(
    page.getByRole("heading", { name: "Clipboard history", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText("1 to 500");
  await page
    .getByRole("button", { name: "Clear clipboard history", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  expect(
    await page.evaluate(() => window.__launcherTest.clipboardCleared),
  ).toBe(false);
  await page
    .getByRole("button", { name: "Clear clipboard history", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Clear history", exact: true })
    .click();
  expect(
    await page.evaluate(() => window.__launcherTest.clipboardCleared),
  ).toBe(true);
});

test("all sections fit narrow windows and appearance persists", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await page.getByRole("radio", { name: /Dark/ }).check();
  await expect(page.locator("html")).toHaveAttribute("data-appearance", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-appearance", "dark");
  for (const width of [320, 375, 414, 768]) {
    await page.setViewportSize({ width, height: 640 });
    for (const name of [
      "Shortcut",
      "Appearance",
      "Categories",
      "Clipboard history",
      "File search",
      "Currency",
      "Privacy",
      "About",
    ]) {
      if (width < 600)
        await page
          .getByRole("combobox", { name: "Settings section" })
          .selectOption({ label: name });
      else await page.getByRole("button", { name, exact: true }).click();
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth <= innerWidth &&
            document.querySelector(".settings-scroll")!.scrollWidth <=
              document.querySelector(".settings-scroll")!.clientWidth,
        ),
      ).toBe(true);
      await expect(
        page.getByRole("button", { name: "Save changes" }),
      ).toBeInViewport();
    }
  }
});

test("launcher provides settings through its menu and keyboard shortcut", async ({
  page,
}) => {
  await openSettings(page);
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "What will you do next?" }),
  ).toBeVisible();
  await page.keyboard.press("Meta+Comma");
  await page
    .getByRole("button", { name: /Actions/ })
    .last()
    .click();
  await page.getByRole("menuitem", { name: /Settings/ }).click();
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "open_settings",
      ),
    ),
  ).toHaveLength(2);
});

test("category checkboxes save, reload, and control the launcher bar", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Categories", exact: true }).click();
  const choices = page.getByRole("group", {
    name: "Visible categories",
    exact: true,
  });
  await expect(choices.getByRole("checkbox")).toHaveCount(11);
  for (const checkbox of await choices.getByRole("checkbox").all()) {
    if ((await checkbox.getAttribute("value")) !== "apps")
      await checkbox.uncheck();
  }
  await expect(
    choices.getByRole("checkbox", { name: "Apps", exact: true }),
  ).toBeDisabled();
  await choices
    .getByRole("checkbox", { name: "Calculator", exact: true })
    .check();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.visibleCategories),
  ).toEqual(["apps", "calculator"]);
  await page.reload();
  await page.getByRole("button", { name: "Categories", exact: true }).click();
  await expect(choices.getByRole("checkbox", { checked: true })).toHaveCount(2);
  await page.goto("/");
  const bar = page.getByRole("navigation", { name: "Search categories" });
  await expect(bar.getByRole("button")).toHaveText(["Apps", "Calculator"]);
  await expect(bar.locator("[aria-pressed=true]")).toHaveText("Apps");
  await page.getByRole("combobox", { name: "Search TinyDash" }).press("Tab");
  await expect(bar.locator("[aria-pressed=true]")).toHaveText("Calculator");
});

test("category choices support discard, failed saves, retry, and restoring all", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Categories", exact: true }).click();
  const files = page.getByRole("checkbox", { name: "Files", exact: true });
  await files.uncheck();
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(files).toBeChecked();
  await files.uncheck();
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings = "Could not save settings.";
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Could not save settings",
  );
  expect(
    await page.evaluate(() =>
      window.__launcherTest.settings.visibleCategories.includes("files"),
    ),
  ).toBe(true);
  await expect(files).not.toBeChecked();
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings = null;
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "Show all categories", exact: true })
    .click();
  await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(11);
  await page.getByRole("button", { name: "Save changes" }).click();
  expect(
    await page.evaluate(
      () => window.__launcherTest.settings.visibleCategories.length,
    ),
  ).toBe(11);
});

test("saves app preferences, custom web searches, category shortcuts, and login setting", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.setViewportSize({ width: 320, height: 640 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: test.info().outputPath("settings-search-320.png"),
  });
  await page.setViewportSize({ width: 768, height: 640 });
  await page.locator('select[size="6"]').selectOption("app-0");
  await page.getByLabel("Aliases, one per line").fill("files\nwork files");
  await page
    .getByRole("checkbox", { name: "Hide this app from search" })
    .check();
  await page
    .getByRole("button", { name: "Add web search", exact: true })
    .click();
  await page.getByLabel("Search name").last().fill("Project docs");
  await page.getByLabel("Keyword").last().fill("docs");
  await page
    .getByLabel("Search URL")
    .last()
    .fill("https://example.com/search?q={query}");
  await page.getByRole("button", { name: "Shortcut", exact: true }).click();
  await page
    .getByRole("button", { name: "Record", exact: true })
    .first()
    .click();
  await page.keyboard.press("Control+Alt+KeyA");
  await page
    .getByRole("switch", { name: "Start TinyDash when you sign in" })
    .check();
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings),
  ).toMatchObject({
    startAtLogin: true,
    appPreferences: {
      "app-0": { aliases: ["files", "work files"], hidden: true },
    },
    webSearches: [
      {
        name: "Project docs",
        keyword: "docs",
        template: "https://example.com/search?q={query}",
        enabled: true,
      },
    ],
  });
  expect(
    await page.evaluate(() => window.__launcherTest.settings.categoryShortcuts),
  ).toEqual([{ mode: "apps", shortcut: "Control+Alt+KeyA" }]);
});

test("previews imports without replacing saved settings and preserves failed imports", async ({
  page,
}) => {
  await openSettings(page);
  await page.evaluate(() => {
    window.__launcherTest.importPreview = {
      settings: { ...window.__launcherTest.settings, hideOnBlur: false },
      ignoredKeys: ["futureField"],
      appearance: "dark",
    };
  });
  await page.getByRole("button", { name: "Privacy", exact: true }).click();
  await page
    .getByRole("button", { name: "Import settings", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Import preview" }),
  ).toBeVisible();
  await page.setViewportSize({ width: 320, height: 640 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: test.info().outputPath("settings-import-320.png"),
  });
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.hideOnBlur),
  ).toBe(true);
  await page
    .getByRole("button", { name: "Import settings", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Import preview" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Apply import", exact: true }).click();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.hideOnBlur),
  ).toBe(true);
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.hideOnBlur),
  ).toBe(false);

  await page.evaluate(() => {
    window.__launcherTest.rejectImport = "Import file is invalid.";
  });
  await page
    .getByRole("button", { name: "Import settings", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("Import file is invalid");
  expect(
    await page.evaluate(() => window.__launcherTest.settings.hideOnBlur),
  ).toBe(false);
});

test("removing a category shortcut stays in the draft until Save", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      "tinydash.test.settings",
      JSON.stringify({
        categoryShortcuts: [{ mode: "apps", shortcut: "Control+Alt+KeyR" }],
      }),
    );
  });
  await openSettings(page);
  await page.getByRole("button", { name: "Shortcut", exact: true }).click();
  await page
    .getByRole("button", { name: "Change", exact: true })
    .first()
    .click();
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("button", { name: "Remove Apps shortcut", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Remove Apps shortcut", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Remove Apps shortcut", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() => window.__launcherTest.settings.categoryShortcuts),
  ).toEqual([{ mode: "apps", shortcut: "Control+Alt+KeyR" }]);
  await page.getByRole("button", { name: "Save changes", exact: true }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => window.__launcherTest.settings.categoryShortcuts),
  ).toEqual([]);
});

test("export is disabled for a dirty draft and works after discard", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Privacy", exact: true }).click();
  const exportButton = page.getByRole("button", {
    name: "Export settings",
    exact: true,
  });
  await expect(exportButton).toBeEnabled();
  await page
    .getByRole("button", { name: "Clipboard history", exact: true })
    .click();
  await page.getByRole("switch", { name: "Save clipboard history" }).uncheck();
  await page.getByRole("button", { name: "Privacy", exact: true }).click();
  await expect(exportButton).toBeDisabled();
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.filter(
          (call) => call.command === "export_settings",
        ).length,
    ),
  ).toBe(0);
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(exportButton).toBeEnabled();
  await exportButton.click();
  await expect(
    page.getByText("Saved settings exported.", { exact: true }),
  ).toBeVisible();
});

test("clears unpinned clipboard entries separately and keeps pinned entries", async ({
  page,
}) => {
  await openSettings(page);
  await page
    .getByRole("button", { name: "Clipboard history", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Clear unpinned entries", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Clear unpinned entries?" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Clear unpinned", exact: true })
    .click();
  expect(
    await page.evaluate(() => window.__launcherTest.clipboardClearKeepPinned),
  ).toBe(true);
  expect(
    await page.evaluate(() => window.__launcherTest.calls.at(-1)?.payload),
  ).toEqual({ keepPinned: true });
});
