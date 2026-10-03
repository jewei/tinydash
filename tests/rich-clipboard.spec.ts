import { expect, test } from "@playwright/test";
import type {} from "./fixtures/rich-clipboard";

test.beforeEach(async ({ page }) => {
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    async (route) => {
      await route.fulfill({
        contentType: "text/javascript",
        body: 'import "/tests/fixtures/rich-clipboard.tsx";',
      });
    },
  );
  await page.goto("/");
});

test("rich clipboard previews file references and copies their original format", async ({
  page,
}) => {
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toContainText("/fixtures/report.pdf");
  await expect(page.getByText("Source: com.apple.finder")).toBeVisible();
  await page.getByRole("button", { name: "Copy original format" }).click();
  await expect(page.getByRole("status")).toHaveText(
    "Copied. Paste in the target application.",
  );
  expect(await page.evaluate(() => window.__richClipboardTest.calls)).toContain(
    "copy_rich_clipboard",
  );
  await page.getByRole("button", { name: "Back", exact: true }).click();
  expect(await page.evaluate(() => window.__richClipboardTest.closed)).toBe(
    true,
  );
});

test("missing rich clipboard file copy reports error without claiming success", async ({
  page,
}) => {
  await page.evaluate(() => {
    window.__richClipboardTest.missing = true;
  });
  await page.getByRole("button", { name: "Copy original format" }).click();
  await expect(page.getByRole("status")).toContainText(
    "A referenced file is no longer available.",
  );
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
});

test("deleting rich clipboard history removes preview without a copy command", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(
    page.getByText("No saved images or file references."),
  ).toBeVisible();
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(page.getByRole("status")).toHaveText(
    "Deleted from history. The system clipboard is unchanged.",
  );
  expect(
    await page.evaluate(() => window.__richClipboardTest.calls),
  ).not.toContain("copy_rich_clipboard");
});

test("deleting an entry keeps its confirmation when the next entry is selected", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({ ...state.entries[0], id: 2, title: "Next file" });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("button", { name: /Next file/ })).toBeVisible();
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(page.getByRole("button", { name: /Next file/ })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await expect(page.getByRole("status")).toHaveText(
    "Deleted from history. The system clipboard is unchanged.",
  );
});

test("a failed next preview preserves the deletion confirmation and its error", async ({
  page,
}) => {
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({
      ...state.entries[0],
      id: 2,
      title: "Unreadable file",
    });
    state.previewError = true;
  });
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "The saved preview cannot be read.",
  );
  await expect(page.getByRole("status")).toHaveText(
    "Deleted from history. The system clipboard is unchanged.",
  );
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Delete saved entry" }),
  ).toBeEnabled();
});
