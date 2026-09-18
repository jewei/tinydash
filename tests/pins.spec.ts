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
  await expect(
    page.getByRole("listbox", { name: "Search results" }).getByRole("option"),
  ).toHaveCount(8);
}

test("app pins persist, preserve selection, and can be removed in compact mode", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page
    .getByRole("listbox", { name: "Search results" })
    .getByRole("option");
  await input.press("ArrowDown");
  await page.getByRole("button", { name: "Pin to All", exact: true }).click();
  await expect(rows.first()).toContainText("Safari");
  await expect(rows.first()).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("button", { name: "Unpin from All", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
  await page.reload();
  await expect(rows.first()).toContainText("Safari");
  await input.fill("sa");
  await expect(rows).toHaveCount(1);
  await expect(page.locator(".result-group")).toHaveText("Applications1");
  await page.getByRole("button", { name: "Clear search", exact: true }).click();
  await expect(rows).toHaveCount(8);
  await input.press("Meta+k");
  await page
    .getByRole("menuitemradio", { name: "Compact", exact: true })
    .click();
  await input.press("Meta+k");
  await page
    .getByRole("menuitem", { name: "Unpin from All", exact: true })
    .click();
  await expect(rows.first()).toContainText("Finder");
  await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
  await expect(input).toBeFocused();
  await page.reload();
  await expect(rows.first()).toContainText("Finder");
});

test("a failed pin write leaves the list unchanged and permits retry", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await page.evaluate(() => {
    window.__launcherTest.rejectPin = true;
  });
  const pin = page.getByRole("button", {
    name: "Pin to All",
    exact: true,
  });
  await pin.click();
  await expect(page.getByRole("alert")).toContainText("Could not save the pin");
  await expect(pin).toBeEnabled();
  await expect(pin).toHaveAttribute("aria-pressed", "false");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Finder");
  await page.evaluate(() => {
    window.__launcherTest.rejectPin = false;
  });
  await pin.click();
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Safari");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("All and Apps have independent pins, including after restart", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page.getByRole("listbox").getByRole("option");
  const tabs = page.getByRole("navigation", { name: "Search categories" });
  await input.press("ArrowDown");
  await page.getByRole("button", { name: "Pin to Apps", exact: true }).click();
  await expect(rows.first()).toContainText("Finder");
  await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".result-group").first()).toHaveText(
    "Applications8",
  );
  await tabs.getByRole("button", { name: "Apps", exact: true }).click();
  await expect(rows.first()).toContainText("Safari");
  await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
  await page.getByRole("button", { name: "Pin to All", exact: true }).click();
  await tabs.getByRole("button", { name: "All", exact: true }).click();
  await expect(rows.first()).toContainText("Safari");
  await page
    .getByRole("button", { name: "Unpin from All", exact: true })
    .click();
  await expect(rows.first()).toContainText("Finder");
  await expect(
    page.getByRole("button", { name: "Unpin from Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.reload();
  await expect(rows.first()).toContainText("Finder");
  await tabs.getByRole("button", { name: "Apps", exact: true }).click();
  await expect(rows.first()).toContainText("Safari");
  await expect(rows).toHaveCount(8);
});

for (const [category, query, title] of [
  ["Files", "Launch notes.md", "Launch notes.md"],
  ["Clipboard", "", "Meeting notes"],
  ["Emoji", "rocket", "rocket"],
  ["Calculator", "12 * 8", "96"],
  ["System", "", "Restart"],
  ["Passwords", "password 24", "SamplePassword0"],
  ["Datetime", "time in Tokyo", "Tokyo"],
  ["URLs", "https://example.com/?utm_source=test", "youtube.com"],
  ["Web", "web rust", "Search Google"],
]) {
  test(`${category} items can be pinned to their category and All`, async ({
    page,
  }) => {
    await openLauncher(page);
    const input = page.getByRole("combobox", { name: "Search TinyDash" });
    const rows = page.getByRole("listbox").getByRole("option");
    const tabs = page.getByRole("navigation", { name: "Search categories" });
    await tabs.getByRole("button", { name: category, exact: true }).click();
    await input.fill(query);
    await expect(rows.first()).toContainText(title);
    await input.press("Meta+k");
    await page
      .getByRole("menuitem", { name: `Pin to ${category}`, exact: true })
      .click();
    await input.fill("");
    await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
    await expect(rows.first()).toContainText(title);
    await input.press("Meta+k");
    await page
      .getByRole("menuitem", { name: "Pin to All", exact: true })
      .click();
    await tabs.getByRole("button", { name: "All", exact: true }).click();
    await expect(rows.first()).toContainText(title);
    await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
    await page.reload();
    await expect(rows.first()).toContainText(title);
    await input.press("Meta+k");
    await page
      .getByRole("menuitem", { name: `Unpin from ${category}`, exact: true })
      .click();
    await expect(rows.first()).toContainText(title);
    await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
    await input.press("Meta+k");
    await expect(
      page.getByRole("menuitem", { name: `Pin to ${category}`, exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("menuitem", { name: "Unpin from All", exact: true }),
    ).toBeVisible();
    expect(
      await page.evaluate(() =>
        window.__launcherTest.calls.filter(
          ({ command }) => command === "execute_action",
        ),
      ),
    ).toEqual([]);
  });
}

test("emoji arrows follow columns across the pinned section", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.emojiGrid = true;
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const tabs = page.getByRole("navigation", { name: "Search categories" });
  await tabs.getByRole("button", { name: "Emoji", exact: true }).click();
  const rows = page.getByRole("listbox").getByRole("option");
  await expect(rows).toHaveCount(20);
  await input.press("ArrowRight");
  await input.press("ArrowRight");
  await input.press("Meta+k");
  await page
    .getByRole("menuitem", { name: "Pin to Emoji", exact: true })
    .click();
  await expect(rows.first()).toContainText("Emoji 3");
  await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
  for (const width of [980, 360]) {
    await page.setViewportSize({ width, height: 620 });
    await input.press("ArrowDown");
    await expect(input).toHaveAttribute("aria-activedescendant", "result-1");
    await input.press("ArrowUp");
    await expect(input).toHaveAttribute("aria-activedescendant", "result-0");
  }
  await page.screenshot({ path: "test-results/category-pins-emoji.png" });
});

test("pin controls fit the detail panel and the compact menu", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await page.getByRole("button", { name: "Pin to Apps", exact: true }).click();
  await page.getByRole("button", { name: "Pin to All", exact: true }).click();
  await expect(page.getByRole("group", { name: "Pin item" })).toBeVisible();
  await page.screenshot({ path: "test-results/category-pins.png" });
  await page.setViewportSize({ width: 360, height: 620 });
  await input.press("Meta+k");
  await expect(
    page.getByRole("menuitem", { name: "Unpin from Apps", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", { name: "Unpin from All", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({ path: "test-results/category-pins-compact.png" });
});

test("category buttons and option-arrow shortcuts select the same search scope", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const categories = page.getByRole("navigation", {
    name: "Search categories",
  });
  await categories.getByRole("button", { name: "Apps", exact: true }).click();
  await expect(input).toBeFocused();
  await expect(
    categories.getByRole("button", { name: "Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await input.press("Alt+ArrowRight");
  await expect(
    categories.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(input).toHaveAttribute(
    "placeholder",
    "Search filenames and paths...",
  );
  await input.press("Alt+ArrowLeft");
  await expect(
    categories.getByRole("button", { name: "Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
});
