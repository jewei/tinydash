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
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            ({ command }) => command === "search",
          ).length,
      ),
    )
    .toBeGreaterThan(0);
  await expect(page.locator("#search-results")).toHaveAttribute(
    "aria-busy",
    "false",
  );
}

const input = (page: Page) =>
  page.getByRole("combobox", { name: "Search TinyDash" });
const welcome = (page: Page) =>
  page.getByRole("region", { name: "What will you do next?" });
const calls = (page: Page) =>
  page.evaluate(() =>
    window.__launcherTest.calls.filter(
      ({ command }) => command === "execute_action",
    ),
  );

for (const platform of ["macos", "windows", "linux"]) {
  test(`${platform} opens with welcome examples and the correct keyboard hints`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 980, height: 620 });
    await page.addInitScript((platform) => {
      localStorage.setItem("tinydash.test.platform", platform);
    }, platform);
    await openLauncher(page);
    await expect(welcome(page)).toBeVisible();
    await expect(welcome(page).getByRole("button")).toHaveText([
      "Find an app↵",
      "128 × 1.08↵",
      "Find an emoji↵",
    ]);
    await expect(
      page.getByRole("heading", { name: "Start typing." }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "Actions" })).toContainText(
      platform === "macos" ? "⌘ K" : "Ctrl K",
    );
    await expect(page.getByRole("option")).toHaveCount(0);
    await expect(input(page)).toBeFocused();
    await expect(page.locator(".list-count")).toHaveText("0 results");
    await expect(page.locator(".footer-prompt")).toHaveText("Type to search");
    await input(page).press("Enter");
    await input(page).press(platform === "macos" ? "Meta+1" : "Control+1");
    expect(await calls(page)).toEqual([]);
    await page.evaluate(async () => {
      for (const event of ["apps-changed", "files-changed", "usage-changed"]) {
        await window.__launcherTest.emit(event);
      }
    });
    await expect(welcome(page)).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(0);
  });
}

test("welcome examples start searches, keep input focus, and do not execute results", async ({
  page,
}) => {
  await openLauncher(page);
  await welcome(page).getByRole("button", { name: "128 × 1.08" }).click();
  await expect(input(page)).toHaveValue("128 * 1.08");
  await expect(input(page)).toBeFocused();
  await expect(page.getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("option")).toContainText("138.24");
  await expect(welcome(page)).toHaveCount(0);

  await page.getByRole("button", { name: "Clear search", exact: true }).click();
  await expect(welcome(page)).toBeVisible();
  await welcome(page).getByRole("button", { name: "Find an emoji" }).click();
  await expect(input(page)).toHaveValue(":smile");
  await expect(input(page)).toBeFocused();
  await expect(page.getByRole("option").locator(".emoji-icon")).toBeVisible();
  await input(page).fill("   ");
  await expect(welcome(page)).toBeVisible();
  await expect(page.getByRole("option")).toHaveCount(0);
  expect(await calls(page)).toEqual([]);
});

test("arrow keys select welcome buttons and Enter browses apps", async ({
  page,
}) => {
  await openLauncher(page);
  await input(page).press("ArrowUp");
  await expect(
    welcome(page).getByRole("button", { name: "Find an emoji" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const findApp = welcome(page).getByRole("button", { name: "Find an app" });
  await expect(findApp).toBeFocused();
  await findApp.press("Enter");
  await expect(input(page)).toBeFocused();
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("option")).toHaveCount(8);
  expect(await calls(page)).toEqual([]);
});

test("hidden Apps uses a web example without selecting a hidden category", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(async () => {
    window.__launcherTest.settings.visibleCategories = ["all"];
    await window.__launcherTest.emit(
      "settings-changed",
      window.__launcherTest.settings,
    );
  });
  await expect(page.getByRole("navigation").getByRole("button")).toHaveText([
    "All",
  ]);
  await welcome(page).getByRole("button", { name: "Search the web" }).click();
  await expect(input(page)).toHaveValue("web weather");
  await expect(input(page)).toBeFocused();
  await expect(page.getByRole("option")).toHaveCount(6);
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "All", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(await calls(page)).toEqual([]);
});

test("clearing on reopen removes previous apps before a pending search finishes", async ({
  page,
}) => {
  await openLauncher(page);
  await welcome(page).getByRole("button", { name: "Find an app" }).click();
  await expect(page.getByRole("option")).toHaveCount(8);
  await input(page).press("Escape");
  await page.evaluate(async () => {
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("launcher-opened", true);
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await expect(welcome(page)).toBeVisible();
  await expect(page.getByRole("option")).toHaveCount(0);
  await expect(input(page)).toHaveValue("");
  await input(page).press("Enter");
  expect(await calls(page)).toEqual([]);
  await page.evaluate(() => window.__launcherTest.releaseSearch?.());
  await expect(welcome(page)).toBeVisible();
});

test("welcome fits all appearances at narrow and desktop widths", async ({
  page,
}) => {
  await openLauncher(page);
  for (const appearance of ["Light", "Dark", "Compact"]) {
    await page.getByRole("button", { name: "Actions" }).click();
    await page
      .getByRole("menuitemradio", { name: appearance, exact: true })
      .click();
    for (const width of [320, 375, 414, 768, 980]) {
      await page.setViewportSize({ width, height: 620 });
      await expect(welcome(page)).toBeInViewport({ ratio: 1 });
      for (const button of await welcome(page).getByRole("button").all()) {
        await expect(button).toBeInViewport({ ratio: 1 });
      }
      await expect(
        page.getByRole("button", { name: "Actions" }),
      ).toBeInViewport({ ratio: 1 });
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      if (width === 375 || width === 980) {
        await page.mouse.move(0, 0);
        await page.screenshot({
          path: `test-results/welcome-${appearance.toLowerCase()}-${width}.png`,
        });
      }
    }
  }
});
