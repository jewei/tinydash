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

test("rich pins persist through refresh and retain selection when sorted first", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({ ...state.entries[0], id: 2, title: "Other file" });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await page.getByRole("button", { name: /Other file/ }).click();
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Pinned.");
  const list = page.getByRole("list", { name: "Saved images and files" });
  await expect(list.getByRole("button").first()).toContainText(
    "Pinned · Other file",
  );
  await expect(list.getByRole("button").first()).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Unpin saved entry", exact: true }),
  ).toBeEnabled();
  await page
    .getByRole("button", { name: "Unpin saved entry", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Unpinned.");
  await expect(
    page.getByRole("button", { name: "Pin saved entry", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() => window.__richClipboardTest.calls),
  ).not.toContain("copy_rich_clipboard");
});

test("failed pin update leaves the entry unpinned and supports retry", async ({
  page,
}) => {
  await page.evaluate(() => {
    window.__richClipboardTest.pinError = true;
  });
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("storage is busy");
  await expect(
    page.getByRole("button", { name: "Pin saved entry", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.pinError = false;
  });
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Unpin saved entry", exact: true }),
  ).toBeEnabled();
});

test("unreadable pinned entries can still be unpinned and deleted", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({
      ...state.entries[0],
      id: 2,
      title: "Missing reference",
      pinned: true,
    });
    state.previewError = true;
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await page.getByRole("button", { name: /Missing reference/ }).click();
  await expect(page.getByRole("alert")).toContainText("Preview unavailable");
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Unpin saved entry", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Unpinned.");
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(
    page.getByRole("button", { name: /Missing reference/ }),
  ).toHaveCount(0);
});

test("rich paste resolves the selected file entry and prevents repeated actions while busy", async ({
  page,
}) => {
  const paste = page.getByRole("button", {
    name: "Paste to previous app",
    exact: true,
  });
  await expect(paste).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.holdPaste = true;
  });
  await paste.click();
  await expect(paste).toBeDisabled();
  for (const name of [
    "Back",
    "Refresh",
    "Copy original format",
    "Delete saved entry",
    "Pin saved entry",
  ]) {
    await expect(
      page.getByRole("button", { name, exact: true }),
    ).toBeDisabled();
  }
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([1]);
  await page.evaluate(() => window.__richClipboardTest.releasePaste?.());
  await expect(page.getByRole("status")).toContainText(
    "Paste sent to the previous app",
  );
  await expect(paste).toBeEnabled();
});

test("rich paste failures retain the preview and offer Copy and retry", async ({
  page,
}) => {
  const paste = page.getByRole("button", {
    name: "Paste to previous app",
    exact: true,
  });
  await page.evaluate(() => {
    window.__richClipboardTest.pasteError =
      "Direct paste needs Accessibility access. Use Copy.";
  });
  await paste.click();
  await expect(page.getByRole("status")).toContainText(
    "Direct paste needs Accessibility access",
  );
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toContainText("/fixtures/report.pdf");
  await expect(paste).toBeFocused();
  await page.getByRole("button", { name: "Copy original format" }).click();
  await expect(page.getByRole("status")).toContainText("Copied.");
  await page.evaluate(() => {
    window.__richClipboardTest.pasteError = null;
    window.__richClipboardTest.missing = true;
  });
  await paste.click();
  await expect(page.getByRole("status")).toContainText(
    "A referenced file is no longer available",
  );
  await expect(paste).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.missing = false;
  });
  await paste.click();
  await expect(page.getByRole("status")).toContainText("Paste sent");
});

test("rich paste sends the selected image ID and is absent on unsupported platforms", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({
      ...state.entries[0],
      id: 2,
      kind: "image",
      title: "Saved PNG",
    });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await page.getByRole("button", { name: /Saved PNG/ }).click();
  await expect(
    page.getByRole("img", { name: "Saved clipboard image" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Paste to previous app", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Paste sent");
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([2]);
  await page.evaluate(() => {
    window.__richClipboardTest.supported = false;
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Paste to previous app", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
});
