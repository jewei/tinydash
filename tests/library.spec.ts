import { expect, test, type Page } from "@playwright/test";

/** Standalone panel harness: these tests do not claim native storage/OS proof. */
async function openLibrary(
  page: Page,
  options: { rejectList?: boolean; holdList?: boolean } = {},
) {
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    async (route) => {
      const response = await route.fetch();
      const source = await response.text();
      const imports = source.slice(0, source.indexOf("const root ="));
      await route.fulfill({
        response,
        body: `${imports}
import LibraryPanel from "/src/components/LibraryPanel.tsx";
import { mockIPC } from "/node_modules/@tauri-apps/api/mocks.js";
let entries = JSON.parse(localStorage.getItem("library-test") || "[]");
window.__libraryTest = { calls: [], rejectSave: false, rejectExecute: false, rejectDelete: false, ...${JSON.stringify(options)} };
const persist = () => localStorage.setItem("library-test", JSON.stringify(entries));
const metadata = (entry) => ({
  id: entry.id, name: entry.name, kind: entry.kind, keywords: entry.keywords,
  arguments: entry.kind === "quicklink"
    ? [...new Set([...entry.content.matchAll(/\\{(query|argument:([A-Za-z0-9_-]+))\\}/g)].map(match => match[2] || "query"))] : [],
  usesClipboard: entry.kind === "snippet" && entry.content.includes("{clipboard}")
});
mockIPC((command, payload) => {
  window.__libraryTest.calls.push({ command, payload });
  if (command === "library_list") {
    if (window.__libraryTest.rejectList) throw new Error("Could not load library");
    const list = () => entries.filter(entry => (entry.name + " " + entry.keywords).toLowerCase().includes(payload.query.toLowerCase())).map(metadata);
    if (window.__libraryTest.holdList) return new Promise(resolve => {
      window.__libraryTest.releaseList = () => {
        window.__libraryTest.holdList = false;
        resolve(list());
      };
    });
    return list();
  }
  if (command === "library_get") {
    const entry = entries.find(entry => entry.id === payload.id);
    if (!entry) throw new Error("Library item no longer exists");
    return entry;
  }
  if (command === "library_save") {
    if (window.__libraryTest.rejectSave) throw new Error("Could not save library");
    const entry = { id: payload.id || "library:" + crypto.randomUUID(), ...payload.draft };
    entries = [...entries.filter(value => value.id !== entry.id), entry]; persist(); return entry;
  }
  if (command === "library_delete") {
    if (window.__libraryTest.rejectDelete) throw new Error("Could not delete item");
    if (!payload.confirmed) throw new Error("Confirm deletion first");
    entries = entries.filter(entry => entry.id !== payload.id); persist(); return;
  }
  if (command === "library_execute") {
    if (window.__libraryTest.rejectExecute) throw new Error("Could not open quicklink");
    const entry = entries.find(entry => entry.id === payload.id);
    if (!entry) throw new Error("Library item no longer exists");
    if (entry.content.includes("{clipboard}") && !payload.allowClipboard) throw new Error("Clipboard permission required");
    return;
  }
  throw new Error("Unexpected command " + command);
});
render(() => LibraryPanel({ onClose: () => { window.__libraryTest.closed = true; } }), document.getElementById("root"));`,
      });
    },
  );
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Quicklinks & snippets" }),
  ).toBeVisible();
}

test("library creates, edits, searches, explicitly runs and deletes quicklinks", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "New quicklink" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Search docs");
  await page.getByLabel("Keywords", { exact: true }).fill("reference manual");
  await page
    .getByLabel("URL template")
    .fill("https://example.com/?q={query}&lang={argument:lang}");
  await page.getByRole("button", { name: "Save item" }).click();
  await expect(page.getByRole("status")).toHaveText("Saved");
  await expect(
    page.getByRole("button", { name: "Open quicklink" }),
  ).toBeDisabled();
  await page.getByLabel("Argument: query", { exact: true }).fill("a & b");
  await page.getByLabel("Argument: lang", { exact: true }).fill("en");
  await page.getByRole("button", { name: "Open quicklink" }).click();
  await expect(page.getByRole("status")).toHaveText("Quicklink opened");
  const execute = await page.evaluate(() =>
    (
      window as unknown as {
        __libraryTest: { calls: { command: string; payload: unknown }[] };
      }
    ).__libraryTest.calls.filter((call) => call.command === "library_execute"),
  );
  expect(execute).toHaveLength(1);
  expect(execute[0].payload).toMatchObject({
    action: "open",
    arguments: { query: "a & b", lang: "en" },
    allowClipboard: false,
  });
  await page.getByRole("button", { name: "Edit item" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Documentation");
  await page.getByRole("button", { name: "Save item" }).click();
  await page.reload();
  await page.getByLabel("Search library").fill("manual");
  await page.getByRole("button", { name: "Documentation Quicklink" }).click();
  await page.getByRole("button", { name: "Delete item", exact: true }).click();
  await page.getByRole("button", { name: "Cancel deletion" }).click();
  await expect(
    page.getByRole("heading", { name: "Documentation" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Delete item", exact: true }).click();
  await page.getByRole("button", { name: "Confirm delete" }).click();
  await expect(page.getByRole("status")).toHaveText("Item deleted");
  await page.reload();
  await expect(
    page.getByText("Your library is empty.", { exact: false }),
  ).toBeVisible();
});

test("library editor and confirmations support keyboard focus and safe recovery", async ({
  page,
}) => {
  await openLibrary(page);
  const name = page.getByLabel("Name", { exact: true });
  const filter = page.getByLabel("Search library");
  const close = page.getByRole("button", { name: "Close library" });
  await page.getByRole("button", { name: "New snippet" }).click();
  await expect(name).toBeFocused();
  await name.fill("Keyboard note");
  await page.getByLabel("Snippet content").fill("Keep this text");
  await close.click();
  const discard = page.getByRole("alertdialog", {
    name: "Discard unsaved changes?",
  });
  await expect(
    discard.getByRole("button", { name: "Keep editing" }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    discard.getByRole("button", { name: "Discard changes" }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    discard.getByRole("button", { name: "Keep editing" }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(close).toBeFocused();
  await expect(page.getByLabel("Snippet content")).toHaveValue(
    "Keep this text",
  );
  await page.getByRole("button", { name: "Save item" }).click();
  await expect(filter).toBeFocused();
  await page.getByRole("button", { name: "Edit item" }).click();
  await expect(name).toBeFocused();
  await page.getByRole("button", { name: "Cancel editing" }).click();
  const deleteItem = page.getByRole("button", {
    name: "Delete item",
    exact: true,
  });
  await deleteItem.click();
  const deletion = page.getByRole("alertdialog", {
    name: "Delete library item?",
  });
  await expect(
    deletion.getByRole("button", { name: "Cancel deletion" }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(deleteItem).toBeFocused();
  await deleteItem.click();
  await page.evaluate(() => {
    (
      window as unknown as { __libraryTest: { rejectDelete: boolean } }
    ).__libraryTest.rejectDelete = true;
  });
  await deletion.getByRole("button", { name: "Confirm delete" }).click();
  await expect(deletion.getByRole("alert")).toHaveText(
    "Error: Could not delete item",
  );
  await expect(deletion).toBeVisible();
  await page.evaluate(() => {
    (
      window as unknown as { __libraryTest: { rejectDelete: boolean } }
    ).__libraryTest.rejectDelete = false;
  });
  await deletion.getByRole("button", { name: "Confirm delete" }).click();
  await expect(deletion).toHaveCount(0);
  await expect(filter).toBeFocused();
  await expect(page.getByRole("status")).toHaveText("Item deleted");
});

test("library distinguishes loading, an empty library and no search matches", async ({
  page,
}) => {
  await openLibrary(page, { holdList: true });
  await expect(page.getByText("Loading library...")).toBeVisible();
  await expect(
    page.getByRole("list", { name: "Library items" }),
  ).toHaveAttribute("aria-busy", "true");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          typeof (
            window as unknown as { __libraryTest: { releaseList?: () => void } }
          ).__libraryTest.releaseList,
      ),
    )
    .toBe("function");
  await page.evaluate(() =>
    (
      window as unknown as { __libraryTest: { releaseList: () => void } }
    ).__libraryTest.releaseList(),
  );
  await expect(
    page.getByText("Your library is empty.", { exact: false }),
  ).toBeVisible();
  await page.getByLabel("Search library").fill("missing");
  await expect(
    page.getByText("No matching items. Try another search."),
  ).toBeVisible();
  await expect(page.getByText("Loading library...")).toHaveCount(0);
});

test("library retries list failures without losing an editor draft", async ({
  page,
}) => {
  await openLibrary(page, { rejectList: true });
  await expect(page.getByRole("alert")).toHaveText(
    "Error: Could not load library",
  );
  await page.getByRole("button", { name: "New snippet" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Unsaved note");
  await page.getByLabel("Snippet content").fill("Draft survives retry");
  await expect(page.getByRole("alert")).toHaveText(
    "Error: Could not load library",
  );
  await page.evaluate(() => {
    (
      window as unknown as { __libraryTest: { rejectList: boolean } }
    ).__libraryTest.rejectList = false;
  });
  await page.getByRole("button", { name: "Retry loading library" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByLabel("Name", { exact: true })).toHaveValue(
    "Unsaved note",
  );
  await expect(page.getByLabel("Snippet content")).toHaveValue(
    "Draft survives retry",
  );
});

test("snippets require per-invocation clipboard consent and support copy and paste", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "New snippet" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Daily note");
  await page.getByLabel("Keywords", { exact: true }).fill(";daily");
  await page.getByLabel("Snippet content").fill("{date} {time}: {clipboard}");
  await page.getByRole("button", { name: "Save item" }).click();
  const consent = page.getByLabel(
    "Allow reading the clipboard for this invocation",
  );
  await expect(
    page.getByRole("button", { name: "Copy snippet" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Paste snippet" }),
  ).toBeDisabled();
  await consent.check();
  await page.getByRole("button", { name: "Copy snippet" }).click();
  await expect(page.getByRole("status")).toHaveText("Snippet copied");
  await expect(consent).not.toBeChecked();
  await consent.check();
  await page.getByRole("button", { name: "Paste snippet" }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { __libraryTest: { closed: boolean } })
            .__libraryTest.closed,
      ),
    )
    .toBe(true);
});

test("failed writes preserve drafts and navigation confirms discarded edits", async ({
  page,
}) => {
  await openLibrary(page);
  await page.getByRole("button", { name: "New snippet" }).click();
  await page.getByLabel("Name", { exact: true }).fill("Keep me");
  await page.getByLabel("Snippet content").fill("Unsaved work");
  await page.evaluate(() => {
    (
      window as unknown as { __libraryTest: { rejectSave: boolean } }
    ).__libraryTest.rejectSave = true;
  });
  await page.getByRole("button", { name: "Save item" }).click();
  await expect(page.getByRole("alert")).toContainText("Could not save library");
  await expect(page.getByLabel("Snippet content")).toHaveValue("Unsaved work");
  await page.getByRole("button", { name: "Close library" }).click();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.getByRole("button", { name: "Keep editing" }).click();
  await page.evaluate(() => {
    (
      window as unknown as { __libraryTest: { rejectSave: boolean } }
    ).__libraryTest.rejectSave = false;
  });
  await page.getByRole("button", { name: "Save item" }).click();
  await expect(page.getByRole("status")).toHaveText("Saved");
  await page.getByRole("button", { name: "Edit item" }).click();
  await page.getByLabel("Snippet content").fill("Discard this");
  await page.getByRole("button", { name: "Cancel editing" }).click();
  await page.getByRole("button", { name: "Discard changes" }).click();
  await expect(page.locator(".library-content")).toHaveText("Unsaved work");
});
