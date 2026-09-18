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
  await page.setViewportSize({ width: 980, height: 620 });
  await page.goto("/");
  await expect(page.locator(".list-count")).toHaveText("0 results");
  await expect(
    page.getByRole("heading", { name: "What will you do next?" }),
  ).toBeVisible();
}
const input = (page: Page) =>
  page.getByRole("combobox", { name: "Search TinyDash" });
const calls = (page: Page) =>
  page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "execute_action")
      .map((call) => call.payload),
  );

test("password results show strength and regenerate the selected type before copying", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).fill("password 32");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(4);
  await expect(
    page.getByText("Strength estimate", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("meter", { name: "Password entropy" }),
  ).toHaveAttribute("value", "99");
  await expect(
    page.getByRole("button", { name: /^Copy generated password/ }),
  ).toBeInViewport({ ratio: 1 });
  await input(page).press("ArrowDown");
  await page
    .getByRole("button", { name: "Generate another", exact: true })
    .click();
  await expect(
    page.getByRole("listbox").getByRole("option").nth(1),
  ).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("listbox").getByRole("option").nth(1),
  ).toContainText("Revision1");
  await expect(input(page)).toBeFocused();
  await input(page).press("Enter");
  expect(await calls(page)).toEqual([
    { id: "password:1-0", action: "regenerate" },
    { id: "password:1-1", action: "copy" },
  ]);
});

test("time results show both dates and preserve selection through a clock refresh", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).fill("time in europe");
  await input(page).press("ArrowDown");
  await expect(
    page.getByText("Your local time", { exact: true }),
  ).toBeVisible();
  await page.evaluate(() => window.__launcherTest.emit("apps-changed"));
  await expect(
    page.getByRole("listbox").getByRole("option").nth(1),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: /^Copy this time/ }).click();
  expect(await calls(page)).toMatchObject([{ action: "copy" }]);
  await input(page).fill("time ambiguous");
  await expect(page.getByText(/This time occurs twice/)).toBeVisible();
  await expect(
    page.getByRole("button", { name: /^Copy this time/ }),
  ).toBeInViewport({ ratio: 1 });
});

test("Datetime shows date arithmetic and Pacific conversion with the correct copy action", async ({
  page,
}) => {
  await openLauncher(page);
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Datetime", exact: true })
    .click();
  await input(page).fill("next friday + 2 week");
  await expect(
    page.getByText("Based on your local date", { exact: true }),
  ).toBeVisible();
  await expect(page.locator(".time-details")).toContainText("Thu, 17 Sep 2026");
  await expect(page.locator(".preview-title")).toHaveText("Fri, 02 Oct 2026");
  const copyDate = page.getByRole("button", { name: /^Copy this date/ });
  await expect(copyDate).toBeInViewport({ ratio: 1 });
  await copyDate.click();
  expect((await calls(page)).at(-1)).toMatchObject({
    id: "tool:date-calculation",
    action: "copy",
  });
  await input(page).fill("10:00 a.m. Pacific Time");
  await expect(page.locator(".preview-title")).toHaveText(
    "01:00 · your local time",
  );
  await expect(page.locator(".time-details")).toContainText("UTC-07:00");
  await expect(page.locator(".time-details")).toContainText(
    "Fri, 18 Sep 2026 · 01:00 · UTC+08:00",
  );
  await page.getByRole("button", { name: /^Copy this time/ }).click();
  expect((await calls(page)).at(-1)).toMatchObject({
    id: "tool:pacific-conversion",
    action: "copy",
  });
  await page.goto("/?view=settings");
  await page.getByRole("button", { name: "Categories", exact: true }).click();
  await expect(
    page.getByRole("checkbox", { name: "Datetime", exact: true }),
  ).toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "Time zones", exact: true }),
  ).toHaveCount(0);
});

test("cleaned URLs offer separate copy and open actions and accept long input", async ({
  page,
}) => {
  await openLauncher(page);
  const query = `https://www.youtube.com/watch?v=demo&t=30&si=${"x".repeat(300)}&utm_source=share`;
  await input(page).fill(query);
  await expect(input(page)).toHaveValue(query);
  await expect(page.locator(".preview-subtitle")).toHaveText(
    "2 tracking fields removed",
  );
  await expect(
    page.getByRole("button", { name: /^Copy cleaned URL/ }),
  ).toBeInViewport({ ratio: 1 });
  await page.getByText("Original URL", { exact: true }).click();
  await expect(page.locator(".original-url code")).toHaveText(query);
  await page.getByRole("button", { name: /^Copy cleaned URL/ }).click();
  await page
    .getByRole("button", { name: "Open cleaned URL", exact: true })
    .click();
  expect(await calls(page)).toEqual([
    { id: "tool:url", action: "copy" },
    { id: "tool:url", action: "open" },
  ]);
});

test("web search presents all six engines and opens the selected engine by ID", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).fill("web rust & c++");
  const options = page.getByRole("listbox").getByRole("option");
  await expect(options).toHaveCount(6);
  await expect(
    page.getByRole("button", { name: /^Open search in browser/ }),
  ).toBeInViewport({ ratio: 1 });
  for (const engine of [
    "Google",
    "DuckDuckGo",
    "Bing",
    "Brave",
    "YouTube",
    "GitHub",
  ])
    await expect(options.filter({ hasText: `Search ${engine}` })).toHaveCount(
      1,
    );
  await input(page).press("Meta+6");
  expect(await calls(page)).toEqual([{ id: "tool:web-5", action: "open" }]);
  await page
    .getByRole("button", { name: "Copy search URL", exact: true })
    .click();
  expect(await calls(page)).toHaveLength(2);
});

test("tools fit Light, Dark, and Compact at desktop and narrow widths", async ({
  page,
}) => {
  await openLauncher(page);
  for (const appearance of ["Light", "Dark", "Compact"]) {
    await input(page).press("Meta+k");
    await page
      .getByRole("menuitemradio", { name: appearance, exact: true })
      .click();
    for (const width of [320, 375, 414, 768, 980]) {
      await page.setViewportSize({ width, height: 620 });
      for (const query of [
        "password 64",
        "time in tokyo",
        "https://example.com/?utm_source=test",
        "web tinydash",
      ]) {
        await input(page).fill(query);
        await expect(
          page.getByRole("listbox").getByRole("option").first(),
        ).toBeVisible();
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await expect(
          page.getByRole("button", { name: /^Actions/ }),
        ).toBeInViewport();
      }
    }
  }
});
