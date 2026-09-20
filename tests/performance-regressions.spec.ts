import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

// These are work/identity regressions, not physical-footprint benchmarks.

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
  await expect(page.locator(".list-count")).toHaveText("0 results");
  await expect(
    page.getByRole("heading", { name: "What will you do next?" }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__launcherTest.resultOverrides["app-1"] = {
      icon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jC1sAAAAASUVORK5CYII=",
    };
  });
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Apps", exact: true })
    .click();
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect(page.getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("option")).toContainText("Safari");
  await settle(page);
}

async function settle(page: Page) {
  await expect(page.locator("#search-results")).toHaveAttribute(
    "aria-busy",
    "false",
  );
  await expect(page.getByRole("option").locator("img")).toBeVisible();
  await expect(page.locator(".preview-icon img")).toBeVisible();
  // Allow the initial layout/size observers to settle before counting requests.
  // This is not a paint-time or latency measurement.
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      }),
  );
}

async function searchCount(page: Page) {
  return page.evaluate(
    () =>
      window.__launcherTest.calls.filter((call) => call.command === "search")
        .length,
  );
}

test("an unchanged refresh preserves the row and image", async ({ page }) => {
  await openLauncher(page);
  const row = await page.getByRole("option").elementHandle();
  const image = await page.getByRole("option").locator("img").elementHandle();
  if (!row || !image)
    throw new Error("Expected a mounted Safari row and image");

  try {
    for (let iteration = 0; iteration < 3; iteration += 1) {
      const searches = await searchCount(page);
      await page.evaluate(() =>
        window.__launcherTest.emit("usage-changed", null),
      );
      await expect.poll(() => searchCount(page)).toBe(searches + 1);
      await settle(page);
      expect(
        await row.evaluate(
          (node) =>
            node === document.querySelector('#search-results [role="option"]'),
        ),
      ).toBe(true);
      expect(
        await image.evaluate(
          (node) =>
            node ===
            document.querySelector('#search-results [role="option"] img'),
        ),
      ).toBe(true);
    }
  } finally {
    await row.dispose();
    await image.dispose();
  }
});

test("a narrower query retains the same row and image", async ({ page }) => {
  await openLauncher(page);
  const row = await page.getByRole("option").elementHandle();
  const image = await page.getByRole("option").locator("img").elementHandle();
  if (!row || !image)
    throw new Error("Expected a mounted Safari row and image");
  const searches = await searchCount(page);

  try {
    await page.getByRole("combobox", { name: "Search TinyDash" }).fill("saf");
    await expect.poll(() => searchCount(page)).toBe(searches + 1);
    await expect(page.getByRole("option")).toHaveCount(1);
    await expect(page.getByRole("option")).toContainText("Safari");
    await settle(page);
    expect(
      await row.evaluate(
        (node) =>
          node === document.querySelector('#search-results [role="option"]'),
      ),
    ).toBe(true);
    expect(
      await image.evaluate(
        (node) =>
          node ===
          document.querySelector('#search-results [role="option"] img'),
      ),
    ).toBe(true);
  } finally {
    await row.dispose();
    await image.dispose();
  }
});

test("a removed result releases its row and a later result gets a new row", async ({
  page,
}) => {
  await openLauncher(page);
  const row = await page.getByRole("option").elementHandle();
  if (!row) throw new Error("Expected a mounted Safari row");
  try {
    const input = page.getByRole("combobox", { name: "Search TinyDash" });
    await input.fill("missing");
    await expect(page.getByRole("option")).toHaveCount(0);
    expect(await row.evaluate((node) => node.isConnected)).toBe(false);
    await input.fill("sa");
    await settle(page);
    expect(
      await row.evaluate(
        (node) =>
          node === document.querySelector('#search-results [role="option"]'),
      ),
    ).toBe(false);
  } finally {
    await row.dispose();
  }
});
