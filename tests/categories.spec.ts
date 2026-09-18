import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openLauncher(page: Page) {
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
  await expect(page.locator("#search-results").getByRole("option")).toHaveCount(
    8,
  );
}

const bar = (page: Page) =>
  page.getByRole("navigation", { name: "Search categories" });
const active = (page: Page) => bar(page).locator("[aria-pressed=true]");
const input = (page: Page) =>
  page.getByRole("combobox", { name: "Search TinyDash" });

async function setCategories(
  page: Page,
  values: import("../src/bridge").SearchMode[],
) {
  await page.evaluate(async (visibleCategories) => {
    window.__launcherTest.settings.visibleCategories = visibleCategories;
    await window.__launcherTest.emit(
      "settings-changed",
      window.__launcherTest.settings,
    );
  }, values);
}

test("Tab changes the category on the first press and keeps focus and query", async ({
  page,
}) => {
  await openLauncher(page);
  await expect(bar(page).getByRole("button")).toHaveCount(11);
  await expect(page.locator(".scope-chip, .scope-popover")).toHaveCount(0);
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(input(page)).toBeFocused();
  await expect(page.getByRole("listbox")).toHaveCount(1);
  await input(page).fill("safari");
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Files");
  await expect(input(page)).toHaveValue("safari");
  await expect(input(page)).toBeFocused();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "search")
            .at(-1)?.payload,
      ),
    )
    .toEqual({ query: "safari", mode: "files" });
  await input(page).press("Shift+Tab");
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(input(page)).toHaveAttribute("aria-controls", "search-results");
});

test("category keys wrap and result keys keep their normal actions", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).press("Shift+Tab");
  await expect(active(page)).toHaveAccessibleName("Web");
  const names = await bar(page).getByRole("button").allTextContents();
  for (const name of names) {
    await input(page).press("Tab");
    await expect(active(page)).toHaveText(name);
    await expect(input(page)).toBeFocused();
  }
  await input(page).press("Tab");
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(page.locator("#search-results")).toHaveAttribute(
    "aria-busy",
    "false",
  );
  await input(page).press("ArrowDown");
  await expect(page.getByRole("option").nth(1)).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(active(page)).toHaveAccessibleName("Apps");
  await input(page).press("Enter");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "execute_action")
            .at(-1)?.payload,
      ),
    )
    .toEqual({ id: "app-1", action: "launch" });
});

test("the category bar stays usable in narrow windows and Escape hides the launcher", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 550 });
  await openLauncher(page);
  await bar(page)
    .getByRole("button", { name: "Clipboard", exact: true })
    .click();
  await expect(input(page)).toBeFocused();
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Emoji");
  await expect(active(page)).toBeInViewport({ ratio: 1 });
  await input(page).press("Shift+Tab");
  await expect(active(page)).toHaveAccessibleName("Clipboard");
  await input(page).press("Escape");
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
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});

test("Tab cycles only visible categories and menus do not change the category", async ({
  page,
}) => {
  await openLauncher(page);
  await setCategories(page, ["all", "calculator", "web"]);
  await expect(bar(page).getByRole("button")).toHaveText([
    "All",
    "Calculator",
    "Web",
  ]);
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Calculator");
  await input(page).press("Meta+k");
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(active(page)).toHaveAccessibleName("Calculator");
  await input(page).focus();
  await input(page).press("Alt+ArrowRight");
  await expect(active(page)).toHaveAccessibleName("Web");
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("All");
  await input(page).press("Shift+Tab");
  await expect(active(page)).toHaveAccessibleName("Web");
});

test("settings select a visible fallback and reopening keeps the saved category choices", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).fill("sa");
  await setCategories(page, ["apps", "web"]);
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(input(page)).toHaveValue("sa");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "search")
            .at(-1)?.payload,
      ),
    )
    .toEqual({ query: "sa", mode: "apps" });
  await input(page).press("Tab");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(active(page)).toHaveAccessibleName("Web");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(input(page)).toHaveValue("");
  await setCategories(page, ["apps"]);
  await input(page).press("Tab");
  await input(page).press("Shift+Tab");
  await expect(active(page)).toHaveAccessibleName("Apps");
  await expect(input(page)).toBeFocused();
});

test("legacy and empty category settings keep a usable category bar", async ({
  page,
}) => {
  await openLauncher(page);
  await setCategories(page, []);
  await expect(bar(page).getByRole("button")).toHaveCount(11);
  await page.evaluate(() => {
    const { visibleCategories, ...legacy } = window.__launcherTest.settings;
    localStorage.setItem("tinydash.test.settings", JSON.stringify(legacy));
  });
  await page.reload();
  await expect(bar(page).getByRole("button")).toHaveCount(11);
  await input(page).press("Tab");
  await expect(active(page)).toHaveAccessibleName("Apps");
});
