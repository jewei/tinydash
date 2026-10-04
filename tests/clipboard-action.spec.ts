import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function open(page: Page, settings = false) {
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
  await page.goto(settings ? "/?view=settings" : "/");
}

async function actionCalls(page: Page) {
  return page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "execute_action")
      .map((call) => call.payload),
  );
}

async function preferPaste(page: Page) {
  await page.evaluate(async () => {
    const settings = {
      ...window.__launcherTest.settings,
      clipboardDefaultAction: "paste" as const,
    };
    window.__launcherTest.settings = settings;
    await window.__launcherTest.emit("settings-changed", settings);
  });
}

test("clipboard default saves, reloads, discards, and retains failed edits", async ({
  page,
}) => {
  await open(page, true);
  const section = page.getByRole("button", {
    name: "Clipboard history",
    exact: true,
  });
  await section.click();
  const choice = page.getByRole("combobox", { name: "Default text action" });
  const save = page.getByRole("button", { name: "Save changes" });
  await expect(choice).toHaveValue("copy");
  await choice.selectOption("paste");
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(choice).toHaveValue("copy");
  await choice.selectOption("paste");
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings = "Cannot save settings.";
  });
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Cannot save settings");
  await expect(choice).toHaveValue("paste");
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings = null;
  });
  await save.click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  await page.reload();
  await section.click();
  await expect(choice).toHaveValue("paste");
  await choice.selectOption("copy");
  await save.click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  await page.reload();
  await section.click();
  await expect(choice).toHaveValue("copy");
});

test("clipboard Enter follows live preferences and leaves both explicit actions available", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Clipboard", exact: true }).click();
  const search = page.getByRole("combobox", { name: "Search TinyDash" });
  const footer = page.locator(".footer .open-button");
  await expect(footer).toHaveText("Copy text");
  await search.press("Enter");
  await expect
    .poll(() => actionCalls(page))
    .toContainEqual({ id: "clipboard:1", action: "copy", confirmed: false });
  await preferPaste(page);
  await expect(footer).toHaveText("Paste to previous app");
  await search.press("Enter");
  await expect
    .poll(() => actionCalls(page))
    .toContainEqual({ id: "clipboard:1", action: "paste", confirmed: false });
  await page.getByRole("button", { name: /^Actions/ }).click();
  await expect(
    page.getByRole("menuitem", { name: /^Paste to previous app/ }),
  ).toHaveCount(1);
  await page.getByRole("menuitem", { name: "Copy text", exact: true }).click();
  await expect
    .poll(async () => (await actionCalls(page)).at(-1))
    .toEqual({ id: "clipboard:1", action: "copy", confirmed: false });
  await page
    .getByRole("button", { name: "Copy saved text", exact: true })
    .click();
  await search.press("ControlOrMeta+Shift+Enter");
  await expect
    .poll(async () => (await actionCalls(page)).at(-1))
    .toEqual({ id: "clipboard:1", action: "paste", confirmed: false });
});

test("paste default applies to clipboard rows in All and keeps failed paste available for Copy", async ({
  page,
}) => {
  await open(page);
  await preferPaste(page);
  const search = page.getByRole("combobox", { name: "Search TinyDash" });
  await search.fill("Meeting");
  await expect(
    page.getByRole("option", { name: /Meeting notes/ }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__launcherTest.rejectActions =
      "Direct paste needs Accessibility access. Use Copy.";
  });
  await search.press("Enter");
  await expect(
    page.getByText(/Direct paste needs Accessibility/),
  ).toBeVisible();
  await expect(search).toHaveValue("Meeting");
  await expect(page.locator(".footer .open-button")).toBeEnabled();
  await page.evaluate(() => {
    window.__launcherTest.rejectActions = false;
  });
  await page
    .getByRole("button", { name: "Copy saved text", exact: true })
    .click();
  await expect
    .poll(async () => (await actionCalls(page)).at(-1))
    .toEqual({ id: "clipboard:1", action: "copy", confirmed: false });
  await page.getByRole("option", { name: /Meeting notes/ }).click();
  await expect
    .poll(async () => (await actionCalls(page)).at(-1))
    .toEqual({ id: "clipboard:1", action: "paste", confirmed: false });
  await search.fill("12 * 8");
  await expect(page.locator(".footer .open-button")).toHaveText("Copy result");
  await search.press("Enter");
  await expect
    .poll(async () => (await actionCalls(page)).at(-1))
    .toMatchObject({ action: "copy" });
});
