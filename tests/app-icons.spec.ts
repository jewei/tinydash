import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function openLauncher(page: Page) {
  // A reused Vite server can append an HMR timestamp to the entry URL.
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
}

async function selectCategory(page: Page, name: string) {
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name, exact: true })
    .click();
}

test("text search and selection do not wait for native icons", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.holdIcons = true;
  });
  await selectCategory(page, "Apps");
  await expect(page.getByRole("option").first()).toContainText("Finder");
  await expect
    .poll(() => page.evaluate(() => window.__launcherTest.heldIcons.length))
    .toBeGreaterThan(0);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await expect(page.locator(".result-row.selected")).toContainText("Safari");
  await expect(page.locator(".app-avatar img")).toHaveCount(0);
  await input.fill("sa");
  await expect(page.getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("option")).toContainText("Safari");
  await page.evaluate(() =>
    window.__launcherTest.heldIcons
      .filter((icon) => icon.key.endsWith("app-0"))
      .forEach((icon) => icon.release()),
  );
  await expect(page.locator(".app-avatar img")).toHaveCount(0);
  await page.evaluate(() =>
    window.__launcherTest.heldIcons.forEach((icon) => icon.release()),
  );
  await expect(page.getByRole("option").locator("img")).toBeVisible();
  const requests = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "app_icon")
      .map((call) => call.payload as { pixels: number }),
  );
  expect(
    requests.every((request) => request.pixels >= 16 && request.pixels <= 256),
  ).toBe(true);
  expect(requests.some((request) => request.pixels >= 64)).toBe(true);
});

test("hidden launcher releases settled icons without cancellation requests", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
  });
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect(page.getByRole("option").locator("img")).toBeVisible();
  await expect(page.locator(".preview-icon img")).toBeVisible();
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-hidden", null),
  );
  await expect(page.locator(".app-avatar img")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.some(
        (call) => call.command === "cancel_app_icon",
      ),
    ),
  ).toBe(false);
});

test("hidden launcher cancels live icons and ignores late replies", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.holdIcons = true;
  });
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect
    .poll(() => page.evaluate(() => window.__launcherTest.heldIcons.length))
    .toBe(2);
  const requests = await page.evaluate(() =>
    window.__launcherTest.heldIcons.map((icon) => icon.request),
  );
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-hidden", null),
  );
  const cancellations = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "cancel_app_icon")
      .map((call) => (call.payload as { request: string }).request),
  );
  expect(cancellations.sort()).toEqual(requests.sort());
  await page.evaluate(() =>
    window.__launcherTest.heldIcons.forEach((icon) => icon.release()),
  );
  await expect(page.locator(".app-avatar img")).toHaveCount(0);
  await page.evaluate(() => {
    window.__launcherTest.holdIcons = false;
    return window.__launcherTest.emit("launcher-opened", false);
  });
  await expect(page.getByRole("option").locator("img")).toBeVisible();
  await expect(page.locator(".preview-icon img")).toBeVisible();
});

test("hidden launcher does not cancel icon requests rejected for capacity", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.busyIcons = true;
  });
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "app_icon",
          ).length,
      ),
    )
    .toBe(2);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-hidden", null),
  );
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "cancel_app_icon",
      ),
    ),
  ).toEqual([]);
});

test("capacity notification retries only visible icons", async ({ page }) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.busyIcons = true;
  });
  await selectCategory(page, "Apps");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "app_icon",
          ).length,
      ),
    )
    .toBeGreaterThan(0);
  await expect(page.locator(".app-avatar img")).toHaveCount(0);
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect(page.getByRole("option")).toHaveCount(1);
  await page.evaluate(async () => {
    window.__launcherTest.busyIcons = false;
    await window.__launcherTest.emit("app-icons-ready", null);
  });
  await expect(page.getByRole("option").locator("img")).toBeVisible();
});

test("a capacity notification before the busy reply still retries the icon", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.iconReadyBeforeBusy = true;
  });
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("sa");
  await expect(page.getByRole("option").locator("img")).toBeVisible();
  await expect(page.locator(".preview-icon img")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.filter(
          (call) => call.command === "app_icon",
        ).length,
    ),
  ).toBe(3);
});

test("a display scale change replaces pending icon sizes", async ({ page }) => {
  await page.addInitScript(() => {
    const media: MediaQueryList[] = [];
    const original = window.matchMedia.bind(window);
    window.matchMedia = (query) => {
      const value = original(query);
      if (query.startsWith("(resolution:")) media.push(value);
      return value;
    };
    Object.assign(window, {
      changeTestScale: () => {
        Object.defineProperty(window, "devicePixelRatio", {
          value: 2,
          configurable: true,
        });
        for (const value of [...media])
          value.dispatchEvent(new Event("change"));
      },
    });
  });
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.nativeIcons = true;
    window.__launcherTest.holdIcons = true;
  });
  await selectCategory(page, "Apps");
  await expect
    .poll(() => page.evaluate(() => window.__launcherTest.heldIcons.length))
    .toBeGreaterThan(0);
  const previous = await page.evaluate(() =>
    window.__launcherTest.heldIcons.map((icon) => icon.request),
  );
  await page.evaluate(() =>
    (window as unknown as { changeTestScale: () => void }).changeTestScale(),
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) =>
              call.command === "app_icon" &&
              (call.payload as { pixels: number }).pixels === 128,
          ).length,
      ),
    )
    .toBeGreaterThan(0);
  const cancelled = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "cancel_app_icon")
      .map((call) => (call.payload as { request: string }).request),
  );
  expect(previous.every((request) => cancelled.includes(request))).toBe(true);
  await page.evaluate(() =>
    window.__launcherTest.heldIcons.forEach((icon) => icon.release()),
  );
  await expect(page.getByRole("option").first().locator("img")).toBeVisible();
});
