import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

const fileId = "file:/Documents/Launch notes.md";

declare global {
  interface Window {
    __fileActionsTest: {
      calls: { command: string; payload: Record<string, unknown> }[];
      releasePreview?: () => void;
    };
  }
}

// Drive the real launcher, ResultPreview, and FilePreview. Only IPC is mocked;
// no native file, clipboard, application, or Trash effects occur in this suite.
async function openFiles(page: Page, mode = "text") {
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
  await page.evaluate(
    ({ mode, fileId }) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (
              command: string,
              payload: Record<string, unknown>,
            ) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;
      const original = internals.invoke;
      window.__fileActionsTest = { calls: [] };
      internals.invoke = async (command, payload) => {
        if (command === "file_preview") {
          window.__fileActionsTest.calls.push({ command, payload });
          if (mode === "stale" && payload.id === fileId) {
            await new Promise<void>((resolve) => {
              window.__fileActionsTest.releasePreview = resolve;
            });
          }
          if (mode === "error")
            throw "This file is no longer available. Refresh the file list.";
          return {
            name: payload.id === fileId ? "Launch notes.md" : "Projects",
            path:
              payload.id === fileId
                ? "/Documents/Launch notes.md"
                : "/Documents/Projects",
            folder: payload.id !== fileId,
            size: 70000,
            modifiedAt: 1700000000,
            readonly: true,
            content:
              mode === "image"
                ? {
                    type: "image",
                    dataUrl:
                      "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jC1sAAAAASUVORK5CYII=",
                  }
                : {
                    type: "text",
                    text:
                      payload.id === fileId
                        ? "<script>window.fileExecuted = true</script>"
                        : "Second file contents",
                    truncated: true,
                  },
          };
        }
        if (command === "execute_file_action") {
          window.__fileActionsTest.calls.push({ command, payload });
          if (payload.action === "quickLook")
            throw "Native Quick Look is available only on macOS. Use the inline preview instead.";
          if (payload.action === "trash" && payload.confirmed !== true)
            throw "Confirmation required";
          return;
        }
        return original(command, payload);
      };
    },
    { mode, fileId },
  );
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Files", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "File preview" }),
  ).toBeVisible();
}

async function calls(page: Page) {
  return page.evaluate(() =>
    window.__fileActionsTest.calls.filter(
      (call) => call.command === "execute_file_action",
    ),
  );
}

test("file preview renders text inertly, reports truncation, and copies only its backend ID", async ({
  page,
}) => {
  await openFiles(page);
  await expect(page.getByLabel("File text preview")).toHaveText(
    "<script>window.fileExecuted = true</script>",
  );
  await expect(page.getByText("Preview truncated to 64 KiB.")).toBeVisible();
  await expect(page.getByText("Read-only", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => "fileExecuted" in window)).toBe(false);
  const preview = page.getByRole("region", { name: "File preview" });
  await preview.getByRole("button", { name: "Copy Path", exact: true }).click();
  await expect(preview.getByRole("status")).toHaveText("Path copied.");
  expect(await calls(page)).toEqual([
    {
      command: "execute_file_action",
      payload: { id: fileId, action: "copyPath" },
    },
  ]);
  await preview
    .getByRole("button", { name: "Quick Look", exact: true })
    .click();
  await expect(preview.getByRole("alert")).toContainText(
    "available only on macOS",
  );
  await page.screenshot({
    path: test.info().outputPath("file-text-preview.png"),
  });
});

test("Open With requires an explicit catalog selection and Trash requires confirmation", async ({
  page,
}) => {
  await openFiles(page);
  await page.getByRole("button", { name: "Open With…", exact: true }).click();
  const open = page.getByRole("button", {
    name: "Open with selected application",
  });
  await expect(open).toBeDisabled();
  await page
    .getByRole("combobox", { name: "Open With application" })
    .selectOption({ label: "Visual Studio Code" });
  await open.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("region", { name: "File preview" }).getByRole("status"),
  ).toHaveText("Action completed.");
  expect(await calls(page)).toEqual([
    {
      command: "execute_file_action",
      payload: { id: fileId, action: "openWith", appId: "app-2" },
    },
  ]);
  await page
    .getByRole("button", { name: "Move to Trash…", exact: true })
    .click();
  await expect(
    page.getByRole("alertdialog", { name: "Move item to Trash?" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).focus();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "hide_launcher",
      ),
    ),
  ).toEqual([]);
  expect(
    (await calls(page)).some((call) => call.payload.action === "trash"),
  ).toBe(false);
  await page
    .getByRole("button", { name: "Move to Trash…", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Confirm Move to Trash", exact: true })
    .click();
  expect((await calls(page)).at(-1)?.payload).toEqual({
    id: fileId,
    action: "trash",
    confirmed: true,
  });
  await expect(
    page.getByRole("region", { name: "File preview" }).getByRole("status"),
  ).toHaveText("Moved to Trash.");
  // Enter on an inline control must never also open the search result.
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "execute_action",
      ),
    ),
  ).toEqual([]);
});

test("selection changes discard old previews and old confirmation state", async ({
  page,
}) => {
  await openFiles(page, "stale");
  await expect
    .poll(() => page.evaluate(() => !!window.__fileActionsTest.releasePreview))
    .toBe(true);
  await page
    .getByRole("button", { name: "Move to Trash…", exact: true })
    .click();
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .fill("Projects");
  await expect(
    page.getByRole("heading", { name: "Projects", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => window.__fileActionsTest.releasePreview?.());
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(page.getByLabel("File text preview")).toHaveText(
    "Second file contents",
  );
  expect(await calls(page)).toEqual([]);
});

test("image previews use a data URL", async ({ page }) => {
  await openFiles(page, "image");
  await expect(
    page.getByRole("img", { name: "Preview of Launch notes.md" }),
  ).toHaveAttribute("src", /^data:image\/png;base64,/);
  await page.screenshot({
    path: test.info().outputPath("file-image-preview.png"),
  });
});

test("preview failure reports a backend error", async ({ page }) => {
  await openFiles(page, "error");
  await expect(
    page.getByRole("region", { name: "File preview" }).getByRole("alert"),
  ).toContainText("This file is no longer available");
});
