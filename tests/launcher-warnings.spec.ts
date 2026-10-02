import { expect, test } from "@playwright/test";
import type { LauncherInfo, LauncherWarning } from "../src/bridge";
import { receiveLauncherSettings } from "../src/launcherController";
import { contracts } from "./fixtures/ipc-contract";
import type {} from "./mock-backend";

const warnings: LauncherWarning[] = [
  { code: "settingsRead", message: "Paramètres illisibles", retryable: false },
  {
    code: "shortcutRegistration",
    message: "Raccourci occupé",
    retryable: true,
  },
  // Deliberately resemble the OLD English prefix: code, not prose, is authority.
  {
    code: "trayUnavailable",
    message: "Could not register tray icon",
    retryable: false,
  },
  {
    code: "shortcutsUnavailable",
    message: "Shortcut service unavailable",
    retryable: false,
  },
];

test("settings warnings clear by code while unrelated and unrepaired warnings survive", () => {
  const info: LauncherInfo = { ...contracts.launcher, warnings };
  const unchanged = receiveLauncherSettings(info, info.settings);
  expect(unchanged.warnings.map((warning) => warning.code)).toEqual([
    "shortcutRegistration",
    "trayUnavailable",
    "shortcutsUnavailable",
  ]);
  const changed = receiveLauncherSettings(info, {
    ...info.settings,
    shortcut: "Control+KeyT",
  });
  expect(changed.warnings.map((warning) => warning.code)).toEqual([
    "trayUnavailable",
    "shortcutsUnavailable",
  ]);
  const categoryChanged = receiveLauncherSettings(info, {
    ...info.settings,
    categoryShortcuts: [{ mode: "apps", shortcut: "Control+KeyA" }],
  });
  expect(categoryChanged.warnings).toEqual(changed.warnings);
  const itemChanged = receiveLauncherSettings(info, {
    ...info.settings,
    itemPreferences: {
      "library:example": {
        aliases: [],
        shortcut: "Control+KeyL",
        hidden: false,
        disabled: false,
      },
    },
  });
  expect(itemChanged.warnings).toEqual(changed.warnings);
  const aliasChanged = receiveLauncherSettings(info, {
    ...info.settings,
    itemPreferences: {
      "library:example": {
        aliases: ["local"],
        shortcut: "",
        hidden: false,
        disabled: false,
      },
    },
  });
  expect(aliasChanged.warnings).toEqual(unchanged.warnings);
  expect(info.warnings).toEqual(warnings);
});

test("launcher displays structured messages and clears only repaired warning categories", async ({
  page,
}) => {
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        body: `import "/tests/mock-backend.ts";\nwindow.__launcherTest.warnings = ${JSON.stringify(warnings)};\n${await response.text()}`,
      });
    },
  );
  await page.goto("/");
  await expect(page.getByRole("alert")).toHaveText("Paramètres illisibles");
  await page.evaluate(() =>
    window.__launcherTest.emit(
      "settings-changed",
      window.__launcherTest.settings,
    ),
  );
  await expect(page.getByRole("alert")).toHaveText("Raccourci occupé");
  await page.evaluate(() => {
    window.__launcherTest.settings = {
      ...window.__launcherTest.settings,
      shortcut: "Control+KeyT",
    };
    return window.__launcherTest.emit(
      "settings-changed",
      window.__launcherTest.settings,
    );
  });
  await expect(page.getByRole("alert")).toHaveText(
    "Could not register tray icon",
  );
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Apps", exact: true })
    .click();
  await expect(page.getByRole("option").first()).toBeVisible();
  await page.evaluate(() => {
    window.__launcherTest.storageError = "Local storage is busy. Try again.";
    return window.__launcherTest.emit("usage-changed", null);
  });
  await expect(page.getByRole("alert")).toHaveText(
    "Local storage is busy. Try again.",
  );
  await expect(page.getByRole("option").first()).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Retry", exact: true }),
  ).toHaveCount(0);
});
