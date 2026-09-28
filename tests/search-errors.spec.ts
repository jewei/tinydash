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
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
}

async function expectSettled(page: Page) {
  await expect(
    page.getByRole("listbox", { includeHidden: true }),
  ).toHaveAttribute("aria-busy", "false");
}

test("a current background retry clears a search failure without editing the query", async ({
  page,
}) => {
  await openLauncher(page);
  await expect(page.locator(".list-count")).toHaveText("0 results");
  await page.evaluate(() => {
    window.__launcherTest.rejectSearch = "Search temporarily unavailable";
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("alert")).toContainText(
    "Search temporarily unavailable",
  );
  await expect(page.getByRole("option")).toHaveCount(0);
  await expectSettled(page);
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = null;
    await window.__launcherTest.emit("apps-changed");
  });
  await expect(page.getByRole("option", { selected: true })).toContainText(
    "Safari",
  );
  await expectSettled(page);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(input).toHaveValue("sa");
  await expect(input).toBeFocused();
  const searches = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "search")
      .map((call) => call.payload),
  );
  expect(searches).toMatchObject([
    { query: "", mode: "all" },
    { query: "sa", mode: "all" },
    { query: "sa", mode: "all" },
  ]);
});

test("background success clears only search errors and preserves action failures", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("option", { selected: true })).toContainText(
    "Safari",
  );
  await expectSettled(page);
  await page.evaluate(() => {
    window.__launcherTest.rejectActions = "Application launch failed";
  });
  await input.press("Enter");
  await expect(page.getByRole("alert")).toHaveText("Application launch failed");
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = "Background search failed";
    await window.__launcherTest.emit("apps-changed");
  });
  await expect(page.getByRole("option")).toHaveCount(0);
  await expectSettled(page);
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = null;
    await window.__launcherTest.emit("apps-changed");
  });
  await expect(page.getByRole("option", { selected: true })).toContainText(
    "Safari",
  );
  await expectSettled(page);
  await expect(page.getByRole("alert")).toHaveText("Application launch failed");
  await expect(input).toHaveValue("sa");
  // An explicit input edit still dismisses the action failure.
  await input.fill("saf");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a successful background search does not dismiss a startup failure", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      "tinydash.test.rejectReady",
      "Startup fixture failure",
    ),
  );
  await openLauncher(page);
  await expect(page.getByRole("alert")).toContainText(
    "Could not connect to TinyDash. Error: Startup fixture failure",
  );
  await page.evaluate(async () => {
    await window.__launcherTest.emit("apps-changed");
  });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "search",
          ).length,
      ),
    )
    .toBe(1);
  await expectSettled(page);
  await expect(page.getByRole("alert")).toContainText(
    "Startup fixture failure",
  );
});

test("an obsolete background success leaves search errors intact while the latest retry is pending", async ({
  page,
}) => {
  await openLauncher(page);
  await expect(page.locator(".list-count")).toHaveText("0 results");
  await page.evaluate(() => {
    window.__launcherTest.rejectSearch = "Initial search failure";
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("alert")).toContainText("Initial search failure");
  await expectSettled(page);
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = null;
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("apps-changed");
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = "Latest search failure";
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("apps-changed");
    const release = window.__launcherTest.releaseSearch!;
    window.__launcherTest.releaseSearch = undefined;
    release();
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "true");
  await expect(page.getByRole("alert")).toContainText("Initial search failure");
  await expect(page.getByRole("option")).toHaveCount(0);
  await page.evaluate(() => window.__launcherTest.releaseSearch!());
  await expectSettled(page);
  await expect(page.getByRole("alert")).toContainText("Latest search failure");
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = null;
    await window.__launcherTest.emit("apps-changed");
  });
  await expect(page.getByRole("option", { selected: true })).toContainText(
    "Safari",
  );
  await expectSettled(page);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(input).toHaveValue("sa");
});

test("an obsolete success cannot dismiss a newer action failure or replace the latest search failure", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("option")).toContainText("Safari");
  await expectSettled(page);
  await page.evaluate(async () => {
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("apps-changed");
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  // This action is available during a pending search, unlike result actions.
  await page.evaluate(() => {
    window.__launcherTest.rejectResetPosition = "Newer reset failure";
  });
  await page.keyboard.press("Meta+k");
  await page.getByRole("menuitem", { name: "Reset window position" }).click();
  await expect(page.getByRole("alert")).toContainText("Newer reset failure");
  await page.evaluate(async () => {
    window.__launcherTest.rejectSearch = "Latest search failed";
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("apps-changed");
    const release = window.__launcherTest.releaseSearch!;
    window.__launcherTest.releaseSearch = undefined;
    release();
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "true");
  await expect(page.getByRole("alert")).toContainText("Newer reset failure");
  await page.evaluate(() => window.__launcherTest.releaseSearch!());
  await expectSettled(page);
  await expect(page.getByRole("option")).toHaveCount(0);
  await expect(page.getByRole("alert")).toContainText("Newer reset failure");
  // Reopening keeps the query and dismisses the action error. Its new search
  // still fails, so the latest search failure becomes visible.
  await input.press("Escape");
  await page.evaluate(async () => {
    await window.__launcherTest.emit("launcher-opened", false);
  });
  await expect(page.getByRole("alert")).toContainText("Latest search failed");
  await expectSettled(page);
  await expect(input).toHaveValue("sa");
  const calls = await page.evaluate(() => window.__launcherTest.calls);
  const held = calls.filter((call) => call.command === "search")[2].payload as {
    requestId: number;
  };
  expect(
    calls.filter((call) => call.command === "cancel_search"),
  ).toContainEqual({
    command: "cancel_search",
    payload: { requestId: held.requestId },
  });
});
