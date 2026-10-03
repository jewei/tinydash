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

test("settings search finds retention and preserves pending changes", async ({
  page,
}) => {
  await open(page, true);
  const search = page.getByRole("searchbox", { name: "Search settings" });
  await search.fill("retention");
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("button", { name: "Clipboard history" })
    .click();
  await page
    .getByRole("spinbutton", { name: "Clipboard retention days" })
    .fill("30");
  await page
    .getByRole("textbox", { name: "Excluded clipboard applications" })
    .fill("Passwords\ncom.example.vault");
  await page.getByRole("switch", { name: "Capture clipboard images" }).check();
  await search.fill("patterns");
  await search.press("Enter");
  await page
    .getByRole("textbox", { name: "File ignore patterns" })
    .fill("*.log\nbuild/**");
  await page.getByRole("switch", { name: "Include hidden files" }).check();
  await page.getByRole("button", { name: "Save changes" }).click();
  expect(
    await page.evaluate(() => ({
      days: window.__launcherTest.settings.clipboardRetentionDays,
      excluded: window.__launcherTest.settings.clipboardExcludedApps,
      images: window.__launcherTest.settings.clipboardCaptureImages,
      patterns: window.__launcherTest.settings.fileSearchIgnorePatterns,
      hidden: window.__launcherTest.settings.fileSearchIncludeHidden,
    })),
  ).toEqual({
    days: 30,
    excluded: ["Passwords", "com.example.vault"],
    images: true,
    patterns: ["*.log", "build/**"],
    hidden: true,
  });
  await page.screenshot({
    path: test.info().outputPath("searchable-settings.png"),
  });
});

test("item aliases hotkeys and hide versus disable persist", async ({
  page,
}) => {
  await open(page, true);
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("button", { name: "Search", exact: true })
    .click();
  const select = page.getByRole("combobox", { name: "Configure item" });
  await expect(select.locator("option").first()).toBeAttached();
  const id = await select.inputValue();
  await page.getByRole("textbox", { name: "Item aliases" }).fill("mybrowser");
  await page
    .getByRole("textbox", { name: "Item global shortcut" })
    .fill("Control+Alt+KeyB");
  await page.getByRole("checkbox", { name: "Hide item from search" }).check();
  await page.getByRole("button", { name: "Save changes" }).click();
  expect(
    await page.evaluate(
      (id) => window.__launcherTest.settings.itemPreferences[id],
      id,
    ),
  ).toEqual({
    aliases: ["mybrowser"],
    shortcut: "Control+Alt+KeyB",
    hidden: true,
    disabled: false,
  });
  await page.reload();
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("button", { name: "Search", exact: true })
    .click();
  await expect(
    page.getByRole("checkbox", { name: "Hide item from search" }),
  ).toBeChecked();
  await expect(
    page.getByRole("checkbox", { name: "Disable item and shortcut" }),
  ).not.toBeChecked();
});

test("launcher opens native views and restores search focus on close", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: /^Actions/ }).click();
  await page
    .getByRole("menuitem", { name: "Quicklinks and snippets", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Quicklinks and snippets" }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).not.toBeVisible();
  await page.getByRole("button", { name: "Close library" }).click();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
  await page.getByRole("button", { name: /^Actions/ }).click();
  await page.getByRole("menuitem", { name: "Utilities", exact: true }).click();
  await expect(
    page.getByRole("region", { name: "Native utilities" }),
  ).toBeVisible();
  await page.screenshot({
    path: test.info().outputPath("integrated-native-utilities.png"),
  });
  await page.getByRole("button", { name: "Close utilities" }).click();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
});

test("native shortcuts do not discard an unsaved library draft", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: /^Actions/ }).click();
  await page
    .getByRole("menuitem", { name: "Quicklinks and snippets", exact: true })
    .click();
  await page.getByRole("button", { name: "New snippet" }).click();
  await page
    .getByRole("textbox", { name: "Name", exact: true })
    .fill("Keep my draft");
  await page.evaluate(() => window.__launcherTest.emit("open-panel", "colors"));
  await expect(
    page.getByText("Close the current view before opening another command."),
  ).toBeVisible();
  await expect(
    page.getByRole("textbox", { name: "Name", exact: true }),
  ).toHaveValue("Keep my draft");
  await page.evaluate(() =>
    window.__launcherTest.emit("confirm-item", { id: "system:logout" }),
  );
  await expect(
    page.getByText(
      "Close the current view, then press the shortcut again to confirm this command.",
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("textbox", { name: "Name", exact: true }),
  ).toHaveValue("Keep my draft");
  await page.getByRole("button", { name: "Close library" }).click();
  await expect(
    page.getByRole("alertdialog", { name: "Discard unsaved changes?" }),
  ).toBeVisible();
});

test("native panels support keyboard return and narrow themed layouts", async ({
  page,
}) => {
  await open(page);
  for (const width of [320, 720]) {
    await page.setViewportSize({ width, height: 640 });
    for (const theme of ["Dark", "Light"]) {
      await page.getByRole("button", { name: /^Actions/ }).click();
      await page
        .getByRole("menuitemradio", { name: theme, exact: true })
        .click();
      for (const [name, region] of [
        ["Quicklinks and snippets", "Quicklinks and snippets"],
        ["Utilities", "Native utilities"],
        ["Images and files clipboard", "Image and file clipboard history"],
      ]) {
        await page.getByRole("button", { name: /^Actions/ }).click();
        await page.getByRole("menuitem", { name, exact: true }).click();
        await expect(
          page.getByRole("region", { name: region, exact: true }),
        ).toBeVisible();
        if (name === "Images and files clipboard")
          await expect(
            page.getByText("No saved images or file references."),
          ).toBeVisible();
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth,
          ),
        ).toBe(true);
        await page.screenshot({
          path: test
            .info()
            .outputPath(`${name.replaceAll(" ", "-")}-${width}-${theme}.png`),
        });
        await page.keyboard.press("Escape");
        await expect(
          page.getByRole("combobox", { name: "Search TinyDash" }),
        ).toBeFocused();
      }
    }
  }
});

test("paste is explicit and sends only the backend-owned result ID", async ({
  page,
}) => {
  await open(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("12 * 8");
  await expect(
    page.getByRole("button", { name: /Paste to previous app/ }),
  ).toBeEnabled();
  await page.getByRole("button", { name: /Paste to previous app/ }).click();
  const call = await page.evaluate(() =>
    window.__launcherTest.calls.find(
      (call) =>
        call.command === "execute_action" &&
        (call.payload as { action?: string })?.action === "paste",
    ),
  );
  expect(call?.payload).toEqual(expect.objectContaining({ action: "paste" }));
  expect(call?.payload).not.toHaveProperty("text");
});
