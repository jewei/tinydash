import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openApps(page: Page, native = true) {
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
  await page.evaluate((value) => {
    window.__launcherTest.nativeIcons = value;
  }, native);
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Apps", exact: true })
    .click();
  await expect(page.getByRole("option").first()).toContainText("Finder");
}

for (const native of [false, true]) {
  test(`${native ? "native" : "static"} app avatars bound display observers`, async ({
    page,
  }) => {
    await page.addInitScript(() => {
      const counts = { intersection: 0, resize: 0, resolution: 0 };
      const Intersection = window.IntersectionObserver;
      const Resize = window.ResizeObserver;
      window.IntersectionObserver = class extends Intersection {
        constructor(...args: ConstructorParameters<typeof Intersection>) {
          super(...args);
          counts.intersection++;
        }
      };
      window.ResizeObserver = class extends Resize {
        constructor(...args: ConstructorParameters<typeof Resize>) {
          super(...args);
          counts.resize++;
        }
      };
      const media = window.matchMedia.bind(window);
      window.matchMedia = (query) => {
        if (query.startsWith("(resolution:")) counts.resolution++;
        return media(query);
      };
      Object.assign(window, { iconDisplayCounts: counts });
    });
    await openApps(page, native);
    await expect(page.getByRole("option").first().locator("img")).toBeVisible();
    const counts = await page.evaluate(
      () =>
        (
          window as unknown as {
            iconDisplayCounts: {
              intersection: number;
              resize: number;
              resolution: number;
            };
          }
        ).iconDisplayCounts,
    );
    expect(counts).toEqual({
      intersection: native ? 1 : 0,
      resize: 0,
      resolution: native ? 1 : 0,
    });
  });
}

test("visible icons load before intersection delivery without loading clipped rows", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 350 });
  await page.addInitScript(() => {
    const Original = window.IntersectionObserver;
    window.IntersectionObserver = class extends Original {
      constructor() {
        super(() => {});
      }
    };
  });
  await openApps(page);
  await expect(page.getByRole("option").first().locator("img")).toBeVisible({
    timeout: 2000,
  });
  const last = page.getByRole("option").last();
  await expect(last).toContainText("System Settings");
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.some(
        (call) =>
          call.command === "app_icon" &&
          (call.payload as { key: string }).key === "app-icon:test:app-7",
      ),
    ),
  ).toBe(false);
});

test("a clipped app loads when scrolling makes it visible", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 350 });
  await openApps(page);
  const last = page.getByRole("option").last();
  await expect(last.locator("img")).toHaveCount(0);
  await last.scrollIntoViewIfNeeded();
  await expect(last.locator("img")).toBeVisible();
});

test("appearance and viewport changes keep native sizes and preview visibility correct", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openApps(page);
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  const row = page.getByRole("option").locator(".app-avatar");
  const preview = page.locator(".preview-icon .app-avatar");
  await expect(row.locator("img")).toBeVisible();
  await expect(preview).toHaveCSS("width", "64px");
  await page.setViewportSize({ width: 700, height: 620 });
  await expect(preview).toHaveCSS("width", "56px");
  await expect(preview.locator("img")).toBeVisible();
  await page.setViewportSize({ width: 560, height: 620 });
  await expect(preview.locator("img")).toHaveCount(0);
  await page.setViewportSize({ width: 980, height: 620 });
  await expect(preview.locator("img")).toBeVisible();
  await page.evaluate(() =>
    window.__launcherTest.emit("appearance-changed", "compact"),
  );
  await expect(row).toHaveCSS("width", "32px");
  await expect(row.locator("img")).toBeVisible();
  await expect(preview.locator("img")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.some(
        (call) =>
          call.command === "app_icon" &&
          (call.payload as { pixels: number }).pixels === 32,
      ),
    ),
  ).toBe(true);
});
