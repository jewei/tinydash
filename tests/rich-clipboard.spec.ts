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
  await expect(page.getByRole("option", { name: /Next file/ })).toBeVisible();
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(page.getByRole("option", { name: /Next file/ })).toHaveAttribute(
    "aria-selected",
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
  await page.getByRole("option", { name: /Other file/ }).click();
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await expect(page.getByRole("status")).toContainText("Pinned.");
  const list = page.getByRole("listbox", { name: "Saved images and files" });
  await expect(list.getByRole("option").first()).toContainText(
    "Pinned · Other file",
  );
  await expect(list.getByRole("option").first()).toHaveAttribute(
    "aria-selected",
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
  await page.getByRole("option", { name: /Missing reference/ }).click();
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
    page.getByRole("option", { name: /Missing reference/ }),
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
  await page.getByRole("option", { name: /Saved PNG/ }).click();
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

test("rich history combines filename, source app, and type filters with a clear empty state", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({
      ...state.entries[0],
      id: 2,
      kind: "image",
      title: "Saved PNG",
      sourceApp: "com.example.Editor",
    });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  const type = page.getByRole("combobox", { name: "Clipboard content type" });
  const source = page.getByRole("combobox", { name: "Clipboard source app" });
  const list = page.getByRole("listbox", { name: "Saved images and files" });
  await search.fill("REPORT.PDF finder");
  await expect(list.getByRole("option")).toHaveCount(1);
  await expect(page.getByText("1 of 2 entries", { exact: true })).toBeVisible();
  await type.selectOption("files");
  await source.selectOption("com.apple.finder");
  await expect(list.getByRole("option")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await type.selectOption("image");
  await expect(page.getByText("No entries match these filters.")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Delete saved entry" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(list.getByRole("option")).toHaveCount(2);
  await expect(search).toHaveValue("");
  await source.selectOption("com.example.Editor");
  await expect(list.getByRole("option")).toContainText("Saved PNG");
  await expect(
    page.getByRole("img", { name: "Saved clipboard image" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Clear filters" }).click();
  await expect(page.getByRole("option", { name: /Saved PNG/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("rich history rejects stale replies and keeps only the newest queued search", async ({
  page,
}) => {
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.holdHistory = true;
    window.__richClipboardTest.historyQueries = [];
  });
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await search.fill("report");
  await expect
    .poll(() =>
      page.evaluate(() => !!window.__richClipboardTest.releaseHistory),
    )
    .toBe(true);
  await search.fill("design");
  await search.fill("missing");
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  expect(
    await page.evaluate(() => window.__richClipboardTest.historyQueries),
  ).toEqual(["report"]);
  await page.evaluate(() => {
    window.__richClipboardTest.holdHistory = false;
    window.__richClipboardTest.releaseHistory?.();
  });
  await expect(page.getByText("No entries match these filters.")).toBeVisible();
  expect(
    await page.evaluate(() => window.__richClipboardTest.historyQueries),
  ).toEqual(["report", "missing"]);
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toHaveCount(0);
});

test("rich search errors keep filters for retry and selection remains stable on refresh", async ({
  page,
}) => {
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await page.evaluate(() => {
    window.__richClipboardTest.historyError = "Clipboard storage is busy.";
  });
  await search.fill("report");
  await expect(page.getByRole("alert")).toContainText(
    "Clipboard storage is busy",
  );
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(search).toHaveValue("report");
  await page.evaluate(() => {
    window.__richClipboardTest.historyError = null;
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await expect(
    page
      .getByRole("listbox", { name: "Saved images and files" })
      .getByRole("option"),
  ).toHaveAttribute("aria-selected", "true");
  await expect(search).toHaveValue("report");
  await page.getByRole("button", { name: "Delete saved entry" }).click();
  await expect(
    page.getByText("No saved images or file references."),
  ).toBeVisible();
  await expect(page.getByText("0 of 0 entries", { exact: true })).toBeVisible();
});

test("rich keyboard navigation keeps search focus and copies the selected filtered entry", async ({
  page,
}) => {
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({ ...state.entries[0], id: 2, title: "Second file" });
    state.filesById[2] = ["/fixtures/report-second.pdf"];
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await search.fill("report");
  await expect(page.getByText("2 of 2 entries", { exact: true })).toBeVisible();
  await search.press("ArrowDown");
  const second = page.getByRole("option", { name: /Second file/ });
  await expect(second).toHaveAttribute("aria-selected", "true");
  await expect(search).toBeFocused();
  await expect(search).toHaveAttribute(
    "aria-activedescendant",
    "rich-clipboard-entry-2",
  );
  await search.press("ArrowDown");
  await expect(second).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await search.press("Enter");
  await expect
    .poll(() => page.evaluate(() => window.__richClipboardTest.copiedIds))
    .toEqual([2]);
  await expect(search).toBeFocused();
  await expect(search).toHaveValue("report");
  await second.focus();
  await page.keyboard.press("ArrowUp");
  const first = page.getByRole("option", { name: /2 file references/ });
  await expect(first).toBeFocused();
  await expect(first).toHaveAttribute("tabindex", "0");
  await expect(second).toHaveAttribute("tabindex", "-1");
  await page.keyboard.press("ArrowUp");
  await expect(first).toBeFocused();
  await page.keyboard.press("Space");
  expect(
    await page.evaluate(() => window.__richClipboardTest.copiedIds),
  ).toEqual([2]);
  // Refresh without moving focus, as a background clipboard event would.
  await page
    .getByRole("button", { name: "Refresh", exact: true })
    .evaluate((button) => (button as HTMLButtonElement).click());
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await expect(first).toBeFocused();
});

test("rich keyboard paste uses the platform modifier and restores keyboard focus after failure", async ({
  page,
}) => {
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.pasteError =
      "Direct paste needs Accessibility access.";
  });
  await search.press("Control+Shift+Enter");
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([]);
  await search.press("Meta+Shift+Enter");
  await expect(page.getByRole("status")).toContainText("Accessibility");
  await expect(search).toBeFocused();
  await search.press("Enter");
  await expect
    .poll(() => page.evaluate(() => window.__richClipboardTest.copiedIds))
    .toEqual([1]);
  await page.goto("/?platform=windows");
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await search.press("Meta+Shift+Enter");
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([]);
  await search.press("Control+Shift+Enter");
  await expect
    .poll(() => page.evaluate(() => window.__richClipboardTest.pastedIds))
    .toEqual([1]);
});

test("rich shortcuts ignore composition, repeated activation, and incomplete previews", async ({
  page,
}) => {
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await search.dispatchEvent("keydown", { key: "Enter", repeat: true });
  await search.dispatchEvent("keydown", { key: "Enter", isComposing: true });
  await search.evaluate((input) => {
    input.dispatchEvent(
      new CompositionEvent("compositionstart", { bubbles: true }),
    );
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        bubbles: true,
        cancelable: true,
      }),
    );
    input.dispatchEvent(
      new CompositionEvent("compositionend", { bubbles: true }),
    );
    input.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        bubbles: true,
        cancelable: true,
      }),
    );
  });
  expect(
    await page.evaluate(() => window.__richClipboardTest.copiedIds),
  ).toEqual([]);
  await page.evaluate(() => {
    const state = window.__richClipboardTest;
    state.entries.push({ ...state.entries[0], id: 2, title: "Slow file" });
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.holdPreview = true;
  });
  await search.press("ArrowDown");
  await expect(page.getByRole("option", { name: /Slow file/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await search.press("Enter");
  await search.press("Meta+Shift+Enter");
  expect(
    await page.evaluate(() => [
      window.__richClipboardTest.copiedIds,
      window.__richClipboardTest.pastedIds,
    ]),
  ).toEqual([[], []]);
  await expect
    .poll(() =>
      page.evaluate(() => !!window.__richClipboardTest.releasePreview),
    )
    .toBe(true);
  await page.evaluate(() => {
    window.__richClipboardTest.holdPreview = false;
    window.__richClipboardTest.releasePreview?.();
  });
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await search.press("Enter");
  await expect
    .poll(() => page.evaluate(() => window.__richClipboardTest.copiedIds))
    .toEqual([2]);
});

test("rich shortcuts leave filter and button behavior intact and cannot duplicate busy paste", async ({
  page,
}) => {
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  const filter = page.getByRole("combobox", { name: "Clipboard content type" });
  await filter.focus();
  await filter.dispatchEvent("keydown", {
    key: "Enter",
    metaKey: true,
    shiftKey: true,
  });
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([]);
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("button", { name: "Unpin saved entry", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__richClipboardTest.copiedIds),
  ).toEqual([]);
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__richClipboardTest.holdPaste = true;
  });
  const row = page
    .getByRole("listbox", { name: "Saved images and files" })
    .getByRole("option")
    .first();
  await row.focus();
  await page.keyboard.press("Meta+Shift+Enter");
  await expect
    .poll(() => page.evaluate(() => !!window.__richClipboardTest.releasePaste))
    .toBe(true);
  await row.dispatchEvent("keydown", {
    key: "Enter",
    metaKey: true,
    shiftKey: true,
  });
  await row.dispatchEvent("keydown", { key: "Enter" });
  expect(
    await page.evaluate(() => [
      window.__richClipboardTest.copiedIds,
      window.__richClipboardTest.pastedIds,
    ]),
  ).toEqual([[], [1]]);
  await page.evaluate(() => {
    window.__richClipboardTest.holdPaste = false;
    window.__richClipboardTest.releasePaste?.();
  });
  await expect(row).toBeFocused();
});

test("pinned names save, search, keep original paths, and restore", async ({
  page,
}) => {
  const nameButton = page.getByRole("button", {
    name: "Name pinned entry",
    exact: true,
  });
  await expect(nameButton).toBeDisabled();
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await nameButton.click();
  await page
    .getByRole("textbox", { name: "Custom name" })
    .fill("Release files");
  await page.getByRole("button", { name: "Save name", exact: true }).click();
  await expect(
    page.getByRole("option", { name: /Release files/ }),
  ).toBeVisible();
  await expect(
    page.getByText("Original title: 2 file references", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toContainText("/fixtures/report.pdf");
  const search = page.getByRole("combobox", {
    name: "Search images and files",
  });
  await search.fill("release");
  await expect(page.getByRole("option")).toHaveCount(1);
  await page
    .getByRole("button", { name: "Unpin saved entry", exact: true })
    .click();
  await expect(page.getByRole("option")).toContainText("Release files");
  await expect(
    page.getByRole("button", { name: "Rename pinned entry", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Rename pinned entry", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Restore original title", exact: true })
    .click();
  await expect(
    page.getByText("No entries match these filters.", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Clear filters", exact: true })
    .click();
  await expect(page.getByRole("option")).toContainText("2 file references");
});

test("name editor keeps its draft on write error and can name an unavailable preview", async ({
  page,
}) => {
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await page.evaluate(() => {
    window.__richClipboardTest.previewError = true;
    window.__richClipboardTest.nameError = true;
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Preview unavailable");
  await page
    .getByRole("button", { name: "Name pinned entry", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Custom name" });
  await input.fill("Find these files");
  await input.press("Enter");
  await expect(page.getByRole("status")).toContainText(
    "Clipboard storage is busy",
  );
  await expect(input).toHaveValue("Find these files");
  await expect(input).toBeFocused();
  await expect(page.getByRole("option")).toContainText("2 file references");
  await page.evaluate(() => {
    window.__richClipboardTest.nameError = false;
  });
  await input.press("Enter");
  await expect(page.getByRole("option")).toContainText("Find these files");
  expect(
    await page.evaluate(() => window.__richClipboardTest.namedEntries),
  ).toEqual([
    { id: 1, name: "Find these files" },
    { id: 1, name: "Find these files" },
  ]);
});

test("name editor cancels without writes and blocks composition and result actions", async ({
  page,
}) => {
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Name pinned entry", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Custom name" });
  await input.fill("Draft name");
  await expect(
    page.getByRole("combobox", { name: "Search images and files" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Back", exact: true }),
  ).toBeDisabled();
  await input.dispatchEvent("compositionstart");
  await input.press("Enter");
  await input.dispatchEvent("compositionend");
  await input.dispatchEvent("keydown", { key: "Enter", repeat: true });
  await expect
    .poll(() =>
      page.evaluate(() => window.__richClipboardTest.namedEntries.length),
    )
    .toBe(0);
  await page
    .getByRole("button", { name: "Cancel naming", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Name pinned entry", exact: true }),
  ).toBeFocused();
  await page
    .getByRole("button", { name: "Name pinned entry", exact: true })
    .click();
  await expect(input).toHaveValue("");
  await input.press("Escape");
  await expect(input).toHaveCount(0);
  expect(
    await page.evaluate(() => window.__richClipboardTest.namedEntries),
  ).toEqual([]);
});

test("reveal sends the saved entry and exact file index without copying", async ({
  page,
}) => {
  const first = page.getByRole("button", {
    name: "Reveal in Finder: /fixtures/report.pdf",
    exact: true,
  });
  const second = page.getByRole("button", {
    name: "Reveal in Finder: /fixtures/design.png",
    exact: true,
  });
  await first.click();
  await expect(page.getByRole("status")).toHaveText(
    "Reveal request sent to Finder.",
  );
  await second.focus();
  await second.press("Enter");
  await expect
    .poll(() => page.evaluate(() => window.__richClipboardTest.revealedFiles))
    .toEqual([
      { id: 1, fileIndex: 0 },
      { id: 1, fileIndex: 1 },
    ]);
  expect(
    await page.evaluate(() => window.__richClipboardTest.copiedIds),
  ).toEqual([]);
  expect(
    await page.evaluate(() => window.__richClipboardTest.pastedIds),
  ).toEqual([]);
  await expect(page.getByRole("option")).toContainText("2 file references");
});

test("reveal blocks concurrent actions and keeps an error available for retry", async ({
  page,
}) => {
  const reveal = page.getByRole("button", {
    name: "Reveal in Finder: /fixtures/design.png",
    exact: true,
  });
  await page.evaluate(() => {
    window.__richClipboardTest.holdReveal = true;
  });
  await reveal.click();
  await expect(
    page.getByRole("button", { name: "Copy original format" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Delete saved entry" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Back", exact: true }),
  ).toBeDisabled();
  await expect(reveal).toBeDisabled();
  await page.evaluate(() => {
    window.__richClipboardTest.revealError =
      "Finder could not reveal the file.";
    window.__richClipboardTest.holdReveal = false;
    window.__richClipboardTest.releaseReveal?.();
  });
  await expect(page.getByRole("status")).toHaveText(
    "Finder could not reveal the file.",
  );
  await expect(reveal).toBeFocused();
  await page.evaluate(() => {
    window.__richClipboardTest.revealError = null;
    window.__richClipboardTest.missing = true;
  });
  await reveal.click();
  await expect(page.getByRole("status")).toContainText("no longer available");
  await expect(page.getByRole("option")).toHaveCount(1);
  await page.evaluate(() => {
    window.__richClipboardTest.missing = false;
  });
  await reveal.click();
  await expect(page.getByRole("status")).toHaveText(
    "Reveal request sent to Finder.",
  );
});

test("reveal is unavailable while naming and absent for images and unsupported platforms", async ({
  page,
}) => {
  const reveals = page.getByRole("button", { name: /^Reveal in Finder:/ });
  await expect(reveals).toHaveCount(2);
  await page
    .getByRole("button", { name: "Pin saved entry", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Name pinned entry", exact: true })
    .click();
  await expect(reveals.first()).toBeDisabled();
  await page
    .getByRole("button", { name: "Cancel naming", exact: true })
    .click();
  await page.evaluate(() => {
    window.__richClipboardTest.entries[0].kind = "image";
  });
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    page.getByRole("img", { name: "Saved clipboard image" }),
  ).toBeVisible();
  await expect(reveals).toHaveCount(0);
  await page.goto("/?platform=windows");
  await expect(
    page.getByRole("list", { name: "Saved file references" }),
  ).toBeVisible();
  await expect(reveals).toHaveCount(0);
});

test.describe("saved image export", () => {
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => {
      window.__richClipboardTest.entries[0].kind = "image";
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(
      page.getByRole("img", { name: "Saved clipboard image" }),
    ).toBeVisible();
  });

  test("save and cancel send only the entry ID and keep history and clipboard", async ({
    page,
  }) => {
    const save = page.getByRole("button", {
      name: "Save image as…",
      exact: true,
    });
    await save.click();
    await expect(page.getByRole("status")).toHaveText("PNG image saved.");
    await expect(save).toBeFocused();
    await page.evaluate(() => {
      window.__richClipboardTest.saveImageResult = false;
    });
    await save.press("Enter");
    await expect(page.getByRole("status")).toHaveText("Save canceled.");
    await expect(save).toBeFocused();
    await expect(page.getByRole("option")).toHaveCount(1);
    expect(
      await page.evaluate(() => window.__richClipboardTest.savedImageIds),
    ).toEqual([1, 1]);
    expect(
      await page.evaluate(() => window.__richClipboardTest.copiedIds),
    ).toEqual([]);
    expect(
      await page.evaluate(() => window.__richClipboardTest.pastedIds),
    ).toEqual([]);
  });

  test("save blocks other controls, retains errors, and permits retry", async ({
    page,
  }) => {
    const save = page.getByRole("button", {
      name: "Save image as…",
      exact: true,
    });
    await page.evaluate(() => {
      window.__richClipboardTest.holdSaveImage = true;
    });
    await save.click();
    await expect(save).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Back", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Delete saved entry" }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "Copy original format" }),
    ).toBeDisabled();
    await page.evaluate(() => {
      window.__richClipboardTest.saveImageError =
        "Could not save the output file.";
      window.__richClipboardTest.holdSaveImage = false;
      window.__richClipboardTest.releaseSaveImage?.();
    });
    await expect(page.getByRole("status")).toHaveText(
      "Could not save the output file.",
    );
    await expect(save).toBeFocused();
    await expect(
      page.getByRole("img", { name: "Saved clipboard image" }),
    ).toBeVisible();
    await page.evaluate(() => {
      window.__richClipboardTest.saveImageError = null;
    });
    await save.click();
    await expect(page.getByRole("status")).toHaveText("PNG image saved.");
  });

  test("save requires a current preview and is absent for file lists and other platforms", async ({
    page,
  }) => {
    const save = page.getByRole("button", {
      name: "Save image as…",
      exact: true,
    });
    await page
      .getByRole("button", { name: "Pin saved entry", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Name pinned entry", exact: true })
      .click();
    await expect(save).toBeDisabled();
    await page
      .getByRole("button", { name: "Cancel naming", exact: true })
      .click();
    await page.evaluate(() => {
      window.__richClipboardTest.previewError = true;
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Preview unavailable");
    await expect(save).toBeDisabled();
    await page.evaluate(() => {
      window.__richClipboardTest.previewError = false;
      window.__richClipboardTest.entries[0].kind = "files";
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(save).toHaveCount(0);
    await page.goto("/?platform=windows");
    await page.evaluate(() => {
      window.__richClipboardTest.entries[0].kind = "image";
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(
      page.getByRole("img", { name: "Saved clipboard image" }),
    ).toBeVisible();
    await expect(save).toHaveCount(0);
  });
});

test.describe("pinned-only rich history", () => {
  test.beforeEach(async ({ page }) => {
    await page.evaluate(() => {
      const state = window.__richClipboardTest;
      const original = state.entries[0];
      state.entries = [
        original,
        { ...original, id: 2, pinned: true, customName: "Release files" },
        {
          ...original,
          id: 3,
          kind: "image",
          pinned: true,
          title: "PNG image",
          customName: "Company logo",
          sourceApp: "com.apple.preview",
        },
      ];
      state.filesById[2] = ["/fixtures/release.zip"];
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("option")).toHaveCount(3);
  });

  test("combines pinned, query, type and source filters and clears all", async ({
    page,
  }) => {
    const pinned = page.getByRole("button", {
      name: "Pinned only",
      exact: true,
    });
    await expect(pinned).toHaveAttribute("aria-pressed", "false");
    await pinned.click();
    await expect(pinned).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByText("2 of 3 entries", { exact: true }),
    ).toBeVisible();
    await page
      .getByRole("combobox", { name: "Search images and files" })
      .fill("release");
    await page
      .getByRole("combobox", { name: "Clipboard content type" })
      .selectOption("files");
    await page
      .getByRole("combobox", { name: "Clipboard source app" })
      .selectOption("com.apple.finder");
    await expect(
      page.getByRole("option", { name: /Release files/ }),
    ).toHaveAttribute("aria-selected", "true");
    await expect(
      page.getByText("1 of 3 entries", { exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(pinned).toHaveAttribute("aria-pressed", "true");
    await page
      .getByRole("button", { name: "Clear filters", exact: true })
      .click();
    await expect(pinned).toHaveAttribute("aria-pressed", "false");
    await expect(page.getByRole("option")).toHaveCount(3);
    await expect(
      page.getByRole("combobox", { name: "Search images and files" }),
    ).toHaveValue("");
    await expect(
      page.getByRole("combobox", { name: "Clipboard content type" }),
    ).toHaveValue("");
    await expect(
      page.getByRole("combobox", { name: "Clipboard source app" }),
    ).toHaveValue("");
  });

  test("unpin removes a match and preserves the active filter through empty results", async ({
    page,
  }) => {
    const pinned = page.getByRole("button", {
      name: "Pinned only",
      exact: true,
    });
    await pinned.focus();
    await pinned.press("Space");
    await expect(page.getByRole("option")).toHaveCount(2);
    await expect(
      page.getByRole("option", { name: /Release files/ }),
    ).toHaveAttribute("aria-selected", "true");
    await page
      .getByRole("button", { name: "Unpin saved entry", exact: true })
      .click();
    await expect(page.getByRole("option")).toHaveCount(1);
    await expect(
      page.getByRole("option", { name: /Company logo/ }),
    ).toHaveAttribute("aria-selected", "true");
    await page
      .getByRole("button", { name: "Unpin saved entry", exact: true })
      .click();
    await expect(
      page.getByText("No entries match these filters.", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText("0 of 3 entries", { exact: true }),
    ).toBeVisible();
    await expect(pinned).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.getByRole("button", { name: "Copy original format" }),
    ).toBeDisabled();
    await page
      .getByRole("button", { name: "Clear filters", exact: true })
      .click();
    await expect(page.getByRole("option")).toHaveCount(3);
  });

  test("keeps the latest toggle during pending replies and retains it after an error", async ({
    page,
  }) => {
    const pinned = page.getByRole("button", {
      name: "Pinned only",
      exact: true,
    });
    await page.evaluate(() => {
      window.__richClipboardTest.holdHistory = true;
    });
    await pinned.click();
    await expect
      .poll(() =>
        page.evaluate(() =>
          window.__richClipboardTest.historyPinnedOnly.at(-1),
        ),
      )
      .toBe(true);
    await pinned.click();
    await page.evaluate(() => {
      window.__richClipboardTest.holdHistory = false;
      window.__richClipboardTest.releaseHistory?.();
    });
    await expect(
      page.getByText("3 of 3 entries", { exact: true }),
    ).toBeVisible();
    await expect(pinned).toHaveAttribute("aria-pressed", "false");
    await page.evaluate(() => {
      window.__richClipboardTest.historyError = "Storage is busy.";
    });
    await pinned.click();
    await expect(page.getByRole("alert")).toContainText("History unavailable");
    await expect(pinned).toHaveAttribute("aria-pressed", "true");
    await page.evaluate(() => {
      window.__richClipboardTest.historyError = null;
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(
      page.getByText("2 of 3 entries", { exact: true }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "Rename pinned entry", exact: true })
      .click();
    await expect(pinned).toBeDisabled();
    await page
      .getByRole("button", { name: "Cancel naming", exact: true })
      .click();
    await expect(pinned).toBeEnabled();
  });
});
