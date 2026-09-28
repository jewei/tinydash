import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openWindows(context: BrowserContext, main: Page) {
  await context.route(
    (url) => url.pathname === "/src/index.tsx",
    async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        body: `import "/tests/mock-backend.ts";\n${await response.text()}`,
      });
    },
  );
  await context.addInitScript(() => {
    localStorage.setItem("tinydash.test.nativeGlass", "true");
    // Isolate the IPC relay path: cross-webview storage notifications are not
    // available on every desktop platform. Without this, storage could mask a
    // missing sync_appearance call. Rust tests verify the real relay and ACL.
    window.addEventListener("storage", (event) =>
      event.stopImmediatePropagation(),
    );
  });
  await main.goto("/");
  await expect(main.locator(".list-count")).toHaveText("0 results");
  const settings = await context.newPage();
  await settings.goto("/?view=settings");
  await expect(
    settings.getByRole("button", { name: "Record new" }),
  ).toBeEnabled();
  return settings;
}

async function lastChange(page: Page) {
  return page.evaluate(
    () =>
      window.__launcherTest.calls
        .filter((call) => call.command === "sync_appearance")
        .at(-1)?.payload,
  );
}

async function mainChoice(
  page: Page,
  name: string,
  role: "menuitemradio" | "menuitemcheckbox",
) {
  await page.keyboard.press("Meta+k");
  await page.getByRole(role, { name, exact: true }).click();
}

test("typed appearance relay keeps both windows synchronized without storage events", async ({
  context,
  page: main,
}) => {
  const settings = await openWindows(context, main);
  await settings
    .getByRole("button", { name: "Appearance", exact: true })
    .click();

  await mainChoice(main, "Sage", "menuitemradio");
  await expect
    .poll(() => lastChange(main))
    .toEqual({ change: { kind: "appearance", value: "sage" } });
  await expect(settings.locator("html")).toHaveAttribute(
    "data-appearance",
    "sage",
  );
  await expect(settings.getByRole("radio", { name: /Sage/ })).toBeChecked();
  await expect(
    settings.getByRole("button", { name: "Save changes" }),
  ).toBeDisabled();

  await mainChoice(main, "Compact", "menuitemcheckbox");
  await expect
    .poll(() => lastChange(main))
    .toEqual({ change: { kind: "compact", value: true } });
  await expect(
    settings.getByRole("switch", { name: "Compact layout" }),
  ).toBeChecked();

  await settings.getByRole("radio", { name: /Rose/ }).check();
  await expect
    .poll(() => lastChange(settings))
    .toEqual({ change: { kind: "appearance", value: "rose" } });
  await expect(main.locator("html")).toHaveAttribute("data-appearance", "rose");
  await settings.getByRole("switch", { name: "Compact layout" }).uncheck();
  await expect(main.locator("html")).toHaveAttribute("data-compact", "false");

  const glass = settings.getByRole("switch", {
    name: "Follow macOS Liquid Glass",
  });
  await glass.uncheck();
  await expect
    .poll(() => lastChange(settings))
    .toEqual({ change: { kind: "systemGlass", value: false } });
  await expect(main.locator(".launcher")).not.toHaveAttribute(
    "data-native-glass",
    "true",
  );
  await glass.check();
  await expect(main.locator(".launcher")).toHaveAttribute(
    "data-native-glass",
    "true",
  );
  await expect(
    settings.getByRole("button", { name: "Save changes" }),
  ).toBeDisabled();

  await main.screenshot({ path: test.info().outputPath("relay-main.png") });
  await settings.screenshot({
    path: test.info().outputPath("relay-settings.png"),
  });
});

test("appearance import stays local until Save changes then reaches the launcher", async ({
  context,
  page: main,
}) => {
  const settings = await openWindows(context, main);
  await settings.evaluate(() => {
    window.__launcherTest.importPreview = {
      settings: window.__launcherTest.settings,
      ignoredKeys: [],
      appearance: "ink",
      compact: true,
      followSystemGlass: false,
    };
  });
  await settings.getByRole("button", { name: "Privacy", exact: true }).click();
  await settings
    .getByRole("button", { name: "Import settings", exact: true })
    .click();
  await settings
    .getByRole("button", { name: "Apply import", exact: true })
    .click();
  await expect(settings.locator("html")).toHaveAttribute(
    "data-appearance",
    "ink",
  );
  await expect(main.locator("html")).toHaveAttribute(
    "data-appearance",
    "light",
  );
  expect(await lastChange(settings)).toBeUndefined();

  await settings.getByRole("button", { name: "Save changes" }).click();
  await expect(
    settings.getByText("Changes saved.", { exact: true }),
  ).toBeVisible();
  await expect(main.locator("html")).toHaveAttribute("data-appearance", "ink");
  await expect(main.locator("html")).toHaveAttribute("data-compact", "true");
  await expect(main.locator(".launcher")).not.toHaveAttribute(
    "data-native-glass",
    "true",
  );
  await main.screenshot({
    path: test.info().outputPath("import-relay-main.png"),
  });
  await settings.screenshot({
    path: test.info().outputPath("import-relay-settings.png"),
  });
});
