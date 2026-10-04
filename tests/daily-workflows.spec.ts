import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function open(page: Page, settings = false) {
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
  await page.goto(settings ? "/?view=settings" : "/");
}

async function queue(page: Page) {
  await page.getByRole("button", { name: "Clipboard", exact: true }).click();
  await expect(
    page.getByRole("option", { name: /Meeting notes/ }),
  ).toBeVisible();
  await page.getByRole("button", { name: /^Actions/ }).click();
  await page.getByRole("menuitem", { name: "Create paste queue" }).click();
  const dialog = page.getByRole("dialog", { name: "Create paste queue" });
  await dialog.getByRole("checkbox", { name: /Project link/ }).check();
  await dialog.getByRole("checkbox", { name: /Meeting notes/ }).check();
  await dialog.getByRole("button", { name: "Queue 2 entries" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(
    page.getByRole("region", { name: "Paste queue", exact: true }),
  ).toBeVisible();
}

test("paste queue keeps the selected order, reports errors, skips, and finishes without wrapping", async ({
  page,
}) => {
  await open(page);
  await queue(page);
  const region = page.getByRole("region", { name: "Paste queue", exact: true });
  await expect(region).toContainText("Next: 1 of 2");
  await region.getByText("Preview next entry", { exact: true }).click();
  await expect(region.locator("pre")).toHaveText("Project link");
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.find(
          (call) =>
            call.command === "paste_queue" &&
            (call.payload as { action: string }).action === "start",
        )?.payload,
    ),
  ).toEqual({ action: "start", ids: ["clipboard:2", "clipboard:1"] });
  await page.evaluate(() => {
    window.__launcherTest.rejectPasteQueue =
      "Focus changed. Nothing was pasted.";
  });
  await region.getByRole("button", { name: "Paste next", exact: true }).click();
  await expect(region.getByRole("alert")).toContainText("Focus changed");
  await expect(region).toContainText("Next: 1 of 2");
  await page.evaluate(() => {
    window.__launcherTest.rejectPasteQueue = null;
  });
  await region.getByRole("button", { name: "Skip entry" }).focus();
  await page.keyboard.press("Enter");
  await expect(region).toContainText("Next: 2 of 2");
  await expect(region.getByRole("alert")).toHaveCount(0);
  await region.getByRole("button", { name: "Paste next", exact: true }).click();
  await expect(region).toContainText("Paste queue complete");
  await expect(
    region.getByRole("button", { name: "Paste next", exact: true }),
  ).toBeDisabled();
  await region.getByRole("button", { name: "Dismiss queue" }).click();
  await expect(region).not.toBeVisible();
});

test("paste queue updates after deletion and can be canceled without pasting", async ({
  page,
}) => {
  await open(page);
  await queue(page);
  const region = page.getByRole("region", { name: "Paste queue", exact: true });
  await page.evaluate(async () => {
    window.__launcherTest.clipboardDeleted.push("clipboard:2");
    await window.__launcherTest.emit("clipboard-changed");
  });
  await expect(region).toContainText("Next: 2 of 2");
  await region.getByRole("button", { name: "Cancel queue" }).click();
  await expect(region).not.toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) =>
          call.command === "paste_queue" &&
          (call.payload as { action: string }).action === "next",
      ),
    ),
  ).toEqual([]);
});

test("paste queue fits narrow and desktop layouts and keeps text inert", async ({
  page,
}) => {
  await open(page);
  await queue(page);
  const region = page.getByRole("region", { name: "Paste queue", exact: true });
  await page.evaluate(() =>
    window.__launcherTest.emit("paste-queue-changed", {
      total: 2,
      position: 0,
      next: {
        id: 2,
        content: "<script>literal text</script>\n" + "x".repeat(400),
        createdAt: 1,
        lastUsedAt: null,
      },
    }),
  );
  await region.getByText("Preview next entry", { exact: true }).click();
  for (const width of [360, 720]) {
    await page.setViewportSize({ width, height: 640 });
    await expect(region.locator("pre")).toContainText(
      "<script>literal text</script>",
    );
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await page.screenshot({
      path: test.info().outputPath(`paste-queue-${width}.png`),
    });
  }
});

test("usage suggestions appear below pins and support normal keyboard activation", async ({
  page,
}) => {
  await open(page);
  await page.evaluate(async () => {
    const fixture = {
      kind: "app" as const,
      title: "Used editor",
      subtitle: "Application",
      score: 525,
      icon: null,
      primaryAction: "launch" as const,
      secondaryActions: [],
    };
    window.__launcherTest.suggestions = [
      { ...fixture, id: "app:/fixture/editor" },
    ];
    await window.__launcherTest.emit("usage-changed");
  });
  await expect(
    page.locator(".result-group").filter({ hasText: "Suggestions" }),
  ).toBeVisible();
  await expect(page.getByRole("option", { name: /Used editor/ })).toBeVisible();
  await page.getByRole("button", { name: "Apps", exact: true }).click();
  await page.getByRole("button", { name: /^Actions/ }).click();
  await page.getByRole("menuitem", { name: "Pin to All", exact: true }).click();
  await page.getByRole("button", { name: "All", exact: true }).click();
  await expect(
    page.locator(".result-group").filter({ hasText: "Pinned" }),
  ).toBeVisible();
  const options = page.getByRole("option");
  await expect(options).toHaveCount(2);
  await expect(options.nth(1)).toContainText("Used editor");
  await page.screenshot({
    path: test.info().outputPath("usage-suggestions.png"),
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await input.press("Enter");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls
            .filter((call) => call.command === "execute_action")
            .at(-1)?.payload,
      ),
    )
    .toMatchObject({ id: "app:/fixture/editor", action: "launch" });
});

test("suggestions can be turned off and the choice persists", async ({
  page,
}) => {
  await open(page, true);
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("button", { name: "Search", exact: true })
    .click();
  const toggle = page.getByRole("switch", {
    name: "Show usage-based suggestions",
  });
  await expect(toggle).toBeChecked();
  await toggle.uncheck();
  await page.getByRole("button", { name: "Save changes" }).click();
  await page.reload();
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("button", { name: "Search", exact: true })
    .click();
  await expect(toggle).not.toBeChecked();
});

test("a snippet result requests insertion by ID and reports native errors", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Apps", exact: true }).click();
  await expect(page.getByRole("option").first()).toBeVisible();
  await page.evaluate(async () => {
    window.__launcherTest.suggestions = [
      {
        id: "library:fixture",
        kind: "systemCommand",
        title: "Signature",
        subtitle: "Snippet",
        score: 1900,
        icon: null,
        primaryAction: "run",
        secondaryActions: [],
      },
    ];
  });
  await page.getByRole("button", { name: "All", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Insert snippet", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__launcherTest.rejectActions =
      "Direct paste needs Accessibility access.";
  });
  await page
    .getByRole("button", { name: "Insert snippet", exact: true })
    .click();
  await expect(
    page.getByText("Direct paste needs Accessibility access.", { exact: true }),
  ).toBeVisible();
  const call = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "execute_action")
      .at(-1),
  );
  expect(call?.payload).toEqual({
    id: "library:fixture",
    action: "run",
  });
});
