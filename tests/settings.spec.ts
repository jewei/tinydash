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
