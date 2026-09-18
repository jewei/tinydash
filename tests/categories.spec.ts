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

const coreCategoryNames = [
  "All",
  "Apps",
  "Files",
  "Clipboard",
  "Emoji",
  "Calculator",
  "System",
];

test("Tab opens all categories, cycles them, and keeps the category while typing", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const categories = page.getByRole("listbox", { name: "Search categories" });
  const category = page.getByRole("button", { name: /^Search category:/ });

  await input.press("Tab");
  await expect(categories).toBeVisible();
  expect(await categories.getByRole("option").count()).toBeGreaterThanOrEqual(
    coreCategoryNames.length,
  );
  for (const [index, name] of coreCategoryNames.entries()) {
    await expect(
      categories.getByRole("option").nth(index),
    ).toHaveAccessibleName(name);
  }
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("All");
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute("aria-controls", "search-categories");
  await expect(input).toHaveAttribute("aria-activedescendant", "category-all");

  await page.keyboard.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("Apps");
  await expect(input).toBeFocused();
  await page.keyboard.type("safari");
  await expect(input).toHaveValue("safari");
  await expect(categories).toBeHidden();
  await expect(category).toHaveText("Apps");
  await expect(input).toHaveAttribute("aria-controls", "search-results");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "search")
            .at(-1)?.payload,
      ),
    )
    .toEqual({ query: "safari", mode: "apps" });

  await input.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("Files");
  await expect(input).toHaveValue("safari");
  await expect(input).toBeFocused();
  await page.keyboard.type(" notes");
  await expect(categories).toBeHidden();
  await expect(category).toHaveText("Files");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "search")
            .at(-1)?.payload,
      ),
    )
    .toEqual({ query: "safari notes", mode: "files" });
  await input.fill("");
  await expect(category).toHaveText("Files");
});

test("category keys wrap in both directions and Enter closes the list without opening a result", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const categories = page.getByRole("listbox", { name: "Search categories" });
  const selected = categories.getByRole("option", { selected: true });

  await input.press("Shift+Tab");
  const categoryNames = await categories
    .getByRole("option")
    .locator("span:first-child")
    .allTextContents();
  const lastCategoryName = categoryNames.at(-1)!;
  await expect(selected).toHaveAccessibleName(lastCategoryName);
  for (const name of [...categoryNames, "All"]) {
    await page.keyboard.press("Tab");
    await expect(selected).toHaveAccessibleName(name);
    await expect(input).toBeFocused();
  }
  await input.press("ArrowUp");
  await expect(selected).toHaveAccessibleName(lastCategoryName);
  await input.press("ArrowDown");
  await expect(selected).toHaveAccessibleName("All");
  await input.press("Enter");
  await expect(categories).toBeHidden();
  await expect(input).toBeFocused();
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "execute_action",
      ),
    ),
  ).toEqual([]);
  await input.press("ArrowDown");
  await expect(
    page.locator("#search-results").getByRole("option").nth(1),
  ).toHaveAttribute("aria-selected", "true");
});

test("the category list supports clicks and closes before Escape hides the launcher", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 550 });
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const category = page.getByRole("button", { name: /^Search category:/ });
  const categories = page.getByRole("listbox", { name: "Search categories" });

  await category.click();
  for (const option of await categories.getByRole("option").all()) {
    await expect(option).toBeInViewport({ ratio: 1 });
  }
  await category.click();
  await expect(categories).toBeHidden();
  await category.click();
  await categories
    .getByRole("option", { name: "Clipboard", exact: true })
    .click();
  await expect(category).toHaveText("Clipboard");
  await expect(categories).toBeHidden();
  await expect(input).toBeFocused();
  await input.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("Emoji");
  await input.press("Escape");
  await expect(categories).toBeHidden();
  await expect(category).toHaveText("Emoji");
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "hide_launcher",
      ),
    ),
  ).toEqual([]);
  await input.press("Escape");
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

test("category cycling resets when the launcher reopens and does not intercept menu keys", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const categories = page.getByRole("listbox", { name: "Search categories" });
  await input.press("Tab");
  await input.press("Tab");
  await page.keyboard.press("Meta+k");
  await expect(categories).toBeHidden();
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(input).toBeFocused();
  await input.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("Files");
  await page.evaluate(() => window.__launcherTest.emit("launcher-hidden"));
  await expect(categories).toBeHidden();
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await input.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("Files");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(categories).toBeHidden();
  await input.press("Tab");
  await expect(
    categories.getByRole("option", { selected: true }),
  ).toHaveAccessibleName("All");
});
