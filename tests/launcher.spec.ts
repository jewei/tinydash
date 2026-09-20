import { expect, test, type Page } from "@playwright/test";
import { inflateSync } from "node:zlib";
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

async function actions(page: Page) {
  return page.evaluate(() =>
    window.__launcherTest.calls.filter(
      (call) => call.command === "execute_action",
    ),
  );
}

async function selectCategory(page: Page, name: string) {
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name, exact: true })
    .click();
}

test("installed app icons appear in the result list and detail panel", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const rowIcon = page
    .getByRole("listbox")
    .getByRole("option")
    .first()
    .locator("img");
  await expect(rowIcon).toBeVisible();
  await expect
    .poll(() =>
      rowIcon.evaluate((image: HTMLImageElement) => image.naturalWidth),
    )
    .toBeGreaterThan(0);
  await expect(page.locator(".preview-icon img")).toHaveAttribute(
    "src",
    (await rowIcon.getAttribute("src")) as string,
  );
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await expect(page.locator(".preview-icon img")).toHaveCount(0);
  await expect(page.locator(".preview-icon svg")).toBeVisible();
  await input.press("ArrowUp");
  await expect(page.locator(".preview-icon img")).toBeVisible();
});

test("rounded window corners remain transparent behind menus and dialogs", async ({
  page,
}) => {
  await openLauncher(page);
  async function expectClearCorner() {
    const png = await page.screenshot({ omitBackground: true });
    expect(png[24]).toBe(8); // Eight bits per channel.
    expect(png[25]).toBe(6); // RGBA, rather than an opaque RGB screenshot.
    const chunks: Buffer[] = [];
    for (let offset = 8; offset < png.length;) {
      const length = png.readUInt32BE(offset);
      if (png.toString("ascii", offset + 4, offset + 8) === "IDAT") {
        chunks.push(png.subarray(offset + 8, offset + 8 + length));
      }
      offset += length + 12;
    }
    // All PNG row filters leave the first pixel of the first row unchanged.
    // The first byte is the filter, followed by red, green, blue, and alpha.
    expect(inflateSync(Buffer.concat(chunks))[4]).toBe(0);
  }
  for (const appearance of ["Dark", "Compact", "Light"]) {
    await page.keyboard.press("Meta+k");
    await expectClearCorner();
    await page
      .getByRole("menuitemradio", { name: appearance, exact: true })
      .click();
    await expectClearCorner();
  }
  await selectCategory(page, "System");
  await page.getByRole("listbox").getByRole("option").first().click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expectClearCorner();
});

test("Canvas detail actions follow the selection and keep system confirmation", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toHaveAttribute(
    "placeholder",
    "What are you looking for?",
  );
  await selectCategory(page, "Apps");
  await expect(page.locator(".result-subtitle").first()).toHaveText(
    "Files and folders",
  );
  await expect(page.locator(".result-subtitle").nth(1)).toHaveText(
    "Web browser",
  );
  await input.press("ArrowDown");
  const details = page.getByRole("complementary", {
    name: "Selected item details",
  });
  await expect(
    details.getByRole("heading", { name: "Safari", exact: true }),
  ).toBeVisible();
  await expect(
    details.getByText("/Applications", { exact: true }),
  ).toBeVisible();
  await expect(details.getByText("Safari.app", { exact: true })).toBeVisible();
  await details.getByRole("button", { name: "Launch application" }).click();
  await details
    .getByRole("button", { name: "Show in enclosing folder" })
    .click();
  expect(await actions(page)).toEqual([
    { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    { command: "execute_action", payload: { id: "app-1", action: "reveal" } },
  ]);
  await selectCategory(page, "System");
  await details.getByRole("button", { name: "Run selected command" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(
    page.getByRole("dialog").getByRole("button", { name: "Cancel" }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  expect(await actions(page)).toHaveLength(2);
  await expect(input).toBeFocused();
});

test("appearance choices persist and Compact keeps clipboard text available", async ({
  page,
}) => {
  await openLauncher(page);
  for (const [label, value] of [
    ["Dark", "dark"],
    ["Compact", "compact"],
    ["Light", "light"],
  ]) {
    await page.keyboard.press("Meta+k");
    await page.getByRole("menuitemradio", { name: label, exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute(
      "data-appearance",
      value,
    );
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute(
      "data-appearance",
      value,
    );
    await expect(
      page.getByRole("heading", { name: "What will you do next?" }),
    ).toBeVisible();
    if (value === "compact") {
      await expect(
        page.getByRole("complementary", { name: "Selected item details" }),
      ).toBeHidden();
      await selectCategory(page, "Clipboard");
      await expect(page.getByLabel("Saved clipboard text")).toContainText(
        "Meeting notes",
      );
      await page.setViewportSize({ width: 360, height: 550 });
      await expect(page.getByLabel("Saved clipboard text")).toBeVisible();
      await page.setViewportSize({ width: 720, height: 550 });
    }
  }
});

test("previous appearance names migrate to the Canvas choices", async ({
  page,
}) => {
  await openLauncher(page);
  for (const [previous, current] of [
    ["mint", "dark"],
    ["paper", "light"],
    ["graphite", "compact"],
  ]) {
    await page.evaluate(
      (value) => localStorage.setItem("tinydash.appearance", value),
      previous,
    );
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute(
      "data-appearance",
      current,
    );
  }
});

test("emoji arrows follow grid columns and wrap incomplete rows after resize", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await selectCategory(page, "Emoji");
  await input.fill("grid");
  const options = page.getByRole("listbox").getByRole("option");
  await expect(options).toHaveCount(20);
  for (const width of [980, 360]) {
    await page.setViewportSize({ width, height: 620 });
    const columns = await options.evaluateAll((elements) => {
      const top = elements[0].getBoundingClientRect().top;
      return elements.filter(
        (element) => element.getBoundingClientRect().top === top,
      ).length;
    });
    const last = Math.floor(19 / columns) * columns;
    await input.press("ArrowUp");
    await expect(input).toHaveAttribute(
      "aria-activedescendant",
      `result-${last}`,
    );
    await input.press("ArrowDown");
    await expect(input).toHaveAttribute("aria-activedescendant", "result-0");
    await input.press("ArrowDown");
    await expect(input).toHaveAttribute(
      "aria-activedescendant",
      `result-${columns}`,
    );
    await input.press("ArrowUp");
    await input.press("ArrowLeft");
    await expect(input).toHaveAttribute("aria-activedescendant", "result-19");
    await input.press("ArrowRight");
    await expect(input).toHaveAttribute("aria-activedescendant", "result-0");
  }
  await input.press("Enter");
  expect(await actions(page)).toEqual([
    { command: "execute_action", payload: { id: "emoji:0", action: "copy" } },
  ]);
});

test("currency refresh keeps cached results usable and shows rate dates and failures", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Calculator");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("100 USD to MYR");
  await page.evaluate(() => {
    window.__launcherTest.currencyDate = "2026-09-16";
    window.__launcherTest.currencyRefreshing = true;
    return window.__launcherTest.emit("currency-changed", null);
  });
  await expect(page.locator(".list-count")).toHaveText("Updating rates...");
  await expect(page.locator(".result-title")).toHaveText("400 MYR");
  await expect(page.locator(".result-subtitle")).toContainText(
    "ECB 2026-09-16 · cached rates",
  );
  await page.keyboard.press("Meta+r");
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.some(
        (call) => call.command === "refresh_currency",
      ),
    ),
  ).toBe(true);
  await page.evaluate(() => {
    window.__launcherTest.currencyRefreshing = false;
    window.__launcherTest.currencyWarning =
      "Could not refresh currency rates. Saved rates remain available offline.";
    return window.__launcherTest.emit("currency-changed", null);
  });
  await expect(page.locator(".list-count")).toHaveText("Rates 2026-09-16");
  await expect(page.getByRole("alert")).toContainText(
    "Saved rates remain available offline",
  );
  await input.press("Enter");
  expect(await actions(page)).toContainEqual({
    command: "execute_action",
    payload: { id: "calculation:currency", action: "copy" },
  });
  await page.setViewportSize({ width: 360, height: 460 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    360,
  );
});

test("missing currency help is readable and its refresh action is available in All mode", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.currencyMissing = true;
  });
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .fill("100 USD to MYR");
  const alert = page.getByRole("alert");
  await expect(alert).toHaveText(
    "Currency rates are unavailable. Connect to the internet, then open Actions and choose Refresh currency rates.",
  );
  for (const width of [720, 360]) {
    await page.setViewportSize({ width, height: 460 });
    const size = await alert.evaluate((element) => ({
      width: element.clientWidth,
      contentWidth: element.scrollWidth,
      height: element.clientHeight,
      contentHeight: element.scrollHeight,
    }));
    expect(size.contentWidth).toBeLessThanOrEqual(size.width);
    expect(size.contentHeight).toBeLessThanOrEqual(size.height);
    await expect(
      page.getByRole("button", { name: "Actions", exact: false }),
    ).toBeInViewport();
  }
  await page.getByRole("button", { name: "Actions", exact: false }).click();
  await page.getByRole("menuitem", { name: "Refresh currency rates" }).click();
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.some(
        (call) => call.command === "refresh_currency",
      ),
    ),
  ).toBe(true);
});

test("the drag handle moves the window without taking input focus", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const handle = page.getByTitle("Drag to move window");
  await expect(input).toBeFocused();
  await handle.click();
  await expect(input).toBeFocused();
  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__launcherTest.calls.filter(
          (call) => call.command === "plugin:window|start_dragging",
        ),
      ),
    )
    .toEqual([
      { command: "plugin:window|start_dragging", payload: { label: "main" } },
    ]);
  await handle.click({ button: "right" });
  await input.fill("Safari");
  await input.dblclick();
  expect(
    await input.evaluate((element: HTMLInputElement) =>
      element.value.slice(
        element.selectionStart ?? 0,
        element.selectionEnd ?? 0,
      ),
    ),
  ).toBe("Safari");
  await selectCategory(page, "Apps");
  await page.getByRole("button", { name: "Hide launcher" }).click();
  const calls = await page.evaluate(() => window.__launcherTest.calls);
  expect(
    calls.filter((call) => call.command === "plugin:window|start_dragging"),
  ).toHaveLength(1);
  expect(calls.some((call) => call.command === "hide_launcher")).toBe(true);
});

test("system commands ask before running and extra Enter cancels", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "System");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toHaveAttribute(
    "placeholder",
    "Search system commands...",
  );
  await expect(page.locator(".list-count")).toHaveText("4 shown");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Restart this computer?" });
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  expect(await actions(page)).toEqual([]);
  await page.keyboard.press("Meta+2");
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(input).toBeFocused();
  expect(await actions(page)).toEqual([]);
  await page.keyboard.press("Meta+2");
  await expect(
    page.getByRole("dialog", { name: "Shut down this computer?" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.keyboard.press("Meta+k");
  await page.getByRole("menuitem", { name: "Run command" }).click();
  await expect(dialog).toBeVisible();
  await page.screenshot({
    path: test.info().outputPath("system-confirmation.png"),
  });
  await page.keyboard.press("Escape");
  expect(await actions(page)).toEqual([]);
});

test("confirmation keeps the selected command through a ranking update and prevents duplicate requests", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("reboot");
  await expect(page.locator(".result-title").first()).toHaveText("Restart");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Restart this computer?" });
  await page.evaluate(() => {
    window.__launcherTest.reverseSystem = true;
    window.__launcherTest.holdAction = true;
    return window.__launcherTest.emit("usage-changed", null);
  });
  await expect(page.locator(".result-title").first()).toHaveText(
    "Open system settings",
  );
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Restart", exact: true }).click();
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "system:restart", action: "run", confirmed: true },
      },
    ]);
  await expect(
    dialog.getByRole("button", { name: "Running..." }),
  ).toBeDisabled();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeDisabled();
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(await actions(page)).toHaveLength(1);
  await page.evaluate(() => window.__launcherTest.releaseAction?.());
  await expect(dialog).toHaveCount(0);
});

test("confirmation keeps an immutable command when the same result ID changes", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("reboot");
  await expect(page.locator(".result-title").first()).toHaveText("Restart");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Restart this computer?" });
  await expect(dialog).toBeVisible();
  await page.evaluate(() => {
    window.__launcherTest.resultOverrides["system:restart"] = {
      title: "Updated command",
      primaryAction: "copy",
      confirmation: {
        title: "Updated confirmation",
        description: "This must not replace the open dialog.",
        confirmLabel: "Updated action",
      },
    };
    return window.__launcherTest.emit("usage-changed", null);
  });
  await expect(page.locator(".result-title").first()).toHaveText(
    "Updated command",
  );
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("Save your work before you continue.");
  await dialog.getByRole("button", { name: "Restart", exact: true }).click();
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "system:restart", action: "run", confirmed: true },
      },
    ]);
});

test("reused rows move and update metadata, pins, icons, and actions", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.resultOverrides["app-1"] = {
      icon: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jC1sAAAAASUVORK5CYII=",
    };
  });
  await selectCategory(page, "Apps");
  const safari = page.getByRole("option").filter({ hasText: "Safari" });
  await expect(safari.locator("img")).toBeVisible();
  const row = await safari.elementHandle();
  await page.evaluate(() => {
    window.__launcherTest.usedAppFirst = true;
    window.__launcherTest.pins.apps = ["app-1"];
    window.__launcherTest.resultOverrides["app-1"] = {
      title: "Safari Preview",
      subtitle: "Updated browser",
      icon: null,
      primaryAction: "reveal",
      secondaryActions: [],
    };
    return window.__launcherTest.emit("apps-changed", null);
  });
  const first = page.getByRole("option").first();
  await expect(first).toContainText("Safari Preview");
  expect(await first.evaluate((node, previous) => node === previous, row)).toBe(
    true,
  );
  await expect(first).toContainText("Updated browser");
  await expect(first.getByLabel("Pinned to Apps")).toBeVisible();
  await expect(first.locator(".result-shortcut")).toHaveText("⌘1");
  await expect(first.locator("img")).toHaveCount(0);
  await first.click();
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "app-1", action: "reveal" },
      },
    ]);
  await row?.dispose();
});

test("clipboard copy choices keep their snapshot during a result update", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  await page.keyboard.press("Meta+k");
  await page.getByRole("menuitem", { name: "Copy selected entries" }).click();
  const dialog = page.getByRole("dialog", { name: "Copy selected entries" });
  await expect(dialog.getByLabel("Meeting notes")).toBeVisible();
  await dialog.getByLabel("Meeting notes").check();
  await page.evaluate(() => {
    window.__launcherTest.resultOverrides["clipboard:1"] = {
      title: "Updated notes",
    };
    window.__launcherTest.clipboardDeleted = ["clipboard:2"];
    return window.__launcherTest.emit("clipboard-changed", null);
  });
  await expect(page.locator(".result-title")).toHaveText("Updated notes");
  await expect(dialog.getByLabel("Meeting notes")).toBeChecked();
  await expect(dialog.getByLabel("Project link")).toBeVisible();
  await dialog.getByRole("button", { name: "Copy 1 entries" }).click();
  await expect
    .poll(() => page.evaluate(() => window.__launcherTest.copiedSelection))
    .toEqual({ ids: ["clipboard:1"], separator: "\n" });
});

test("system failures remain in the dialog and reopening discards pending confirmation", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "System");
  await expect(page.locator(".result-title")).toHaveCount(4);
  await page.keyboard.press("Meta+3");
  const dialog = page.getByRole("dialog", {
    name: "Put this computer to sleep?",
  });
  await page.evaluate(() => {
    window.__launcherTest.rejectActions = true;
  });
  await dialog.getByRole("button", { name: "Sleep", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "The OS denied this system command.",
  );
  await expect(
    dialog.getByRole("button", { name: "Sleep", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
  await expect(
    page
      .getByRole("navigation", { name: "Search categories" })
      .locator("[aria-pressed=true]"),
  ).toHaveAccessibleName("All");
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(await actions(page)).toHaveLength(1);
});

test("settings runs directly and System mode has its own empty state", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "System");
  await expect(page.locator(".result-title")).toHaveCount(4);
  await page.keyboard.press("Meta+4");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "system:settings", action: "run" },
      },
    ]);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("missing");
  await expect(
    page.getByRole("heading", { name: "No system commands found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Run command" }),
  ).toBeDisabled();
});

test("keeps only the latest waiting query when input and index events overlap", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.holdSearch = true;
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("slow");
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await input.fill("intermediate");
  await input.fill("sa");
  await page.evaluate(() => window.__launcherTest.emit("files-changed", null));
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => window.__launcherTest.releaseSearch?.());
  await expect(page.locator(".result-title")).toHaveText("Safari");
  await expect(page.getByRole("alert")).toHaveCount(0);
  const queries = await page.evaluate(() =>
    window.__launcherTest.calls
      .filter((call) => call.command === "search")
      .map((call) => (call.payload as { query: string }).query)
      .filter(Boolean),
  );
  expect(queries).toEqual(["slow", "sa"]);
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    ]);
});

test("hidden windows skip background searches and refresh when reopened", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("any");
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
  await input.press("Escape");
  const before = await page.evaluate(
    () =>
      window.__launcherTest.calls.filter((call) => call.command === "search")
        .length,
  );
  await page.evaluate(async () => {
    window.__launcherTest.usedAppFirst = true;
    for (const event of [
      "apps-changed",
      "files-changed",
      "currency-changed",
      "usage-changed",
      "clipboard-changed",
    ]) {
      await window.__launcherTest.emit(event);
    }
  });
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.filter((call) => call.command === "search")
          .length,
    ),
  ).toBe(before);
  await expect(page.locator(".result-title").first()).toHaveText("Finder");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(input).toHaveValue("any");
  await expect(input).toBeFocused();
  await expect(page.locator(".result-title").first()).toHaveText("Safari");
});

test("reopening requests the selected clipboard preview only once", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "Meeting notes",
  );
  await page.getByRole("combobox", { name: "Search TinyDash" }).press("Escape");
  await expect(page.getByLabel("Clipboard preview")).toHaveCount(0);
  const before = await page.evaluate(
    () =>
      window.__launcherTest.calls.filter(
        (call) => call.command === "clipboard_preview",
      ).length,
  );
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "Meeting notes",
  );
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.filter(
          (call) => call.command === "clipboard_preview",
        ).length,
    ),
  ).toBe(before + 1);
});

test("hiding cancels waiting input and ignores an in-flight search result", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await page.evaluate(() => {
    window.__launcherTest.holdSearch = true;
  });
  await input.fill("slow");
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await input.fill("sa");
  await input.press("Escape");
  await page.evaluate(() => window.__launcherTest.releaseSearch?.());
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls
        .filter((call) => call.command === "search")
        .map((call) => (call.payload as { query: string }).query)
        .filter(Boolean),
    ),
  ).toEqual(["slow"]);
  await expect(page.locator(".result-title")).toHaveCount(8);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(page.locator(".result-title")).toHaveText("Safari");
});

test("background refresh keeps the latest keyboard selection", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await expect(
    page.getByRole("listbox").getByRole("option", { selected: true }),
  ).toContainText("Safari");
  await page.evaluate(async () => {
    window.__launcherTest.holdNextSearch = true;
    await window.__launcherTest.emit("files-changed");
  });
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await input.press("ArrowDown");
  await expect(
    page.getByRole("listbox").getByRole("option", { selected: true }),
  ).toContainText("Visual Studio Code");
  await page.evaluate(() => window.__launcherTest.releaseSearch?.());
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
  await expect(
    page.getByRole("listbox").getByRole("option", { selected: true }),
  ).toContainText("Visual Studio Code");
  for (const event of ["apps-changed", "usage-changed"]) {
    await page.evaluate((event) => window.__launcherTest.emit(event), event);
    await expect(page.getByRole("listbox")).toHaveAttribute(
      "aria-busy",
      "false",
    );
    await expect(
      page.getByRole("listbox").getByRole("option", { selected: true }),
    ).toContainText("Visual Studio Code");
  }
});

test("a background event does not preserve selection from a different query", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await page.evaluate(() => {
    window.__launcherTest.holdNextSearch = true;
  });
  await input.fill("new query");
  await page.waitForFunction(() => !!window.__launcherTest.releaseSearch);
  await page.evaluate(() => window.__launcherTest.emit("files-changed"));
  await page.evaluate(() => window.__launcherTest.releaseSearch?.());
  await expect(page.getByRole("listbox")).toHaveAttribute("aria-busy", "false");
  await expect(
    page.getByRole("listbox").getByRole("option", { selected: true }),
  ).toContainText("Finder");
});

test("opens and reveals files by ID, and refreshes files with the mode shortcut", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Files");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute(
    "placeholder",
    "Search filenames and paths...",
  );
  await expect(page.locator(".list-count")).toHaveText("1 file indexed");
  await expect(page.locator(".result-title")).toHaveText("Launch notes.md");
  await input.press("Enter");
  await input.press("Meta+Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "file:/Documents/Launch notes.md", action: "open" },
      },
      {
        command: "execute_action",
        payload: { id: "file:/Documents/Launch notes.md", action: "reveal" },
      },
    ]);
  await input.press("Meta+r");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "refresh_files",
          ).length,
      ),
    )
    .toBe(1);
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("searchbox", { name: "Search actions" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("menuitem", { name: "Open file" })).toBeFocused();
  await expect(
    page.getByRole("menuitem", { name: "Show in folder" }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", { name: "Refresh files" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.screenshot({ path: test.info().outputPath("files.png") });
});

test("keeps results usable during a file scan and shows scan warnings and empty results", async ({
  page,
}) => {
  await openLauncher(page);
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("any");
  await page.evaluate(() => {
    window.__launcherTest.fileIndexing = true;
    return window.__launcherTest.emit("files-changed", null);
  });
  await expect(page.locator(".query-hint")).toContainText("Scanning files...");
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("missing");
  await expect(
    page.getByRole("heading", { name: "No results yet" }),
  ).toBeVisible();
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("");
  await selectCategory(page, "Files");
  await expect(page.locator(".list-count")).toHaveText("Scanning files...");
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    window.__launcherTest.fileIndexing = false;
    window.__launcherTest.fileWarning =
      "File scan skipped 1 item. Permission denied.";
    return window.__launcherTest.emit("files-changed", null);
  });
  await expect(page.getByRole("alert")).toContainText("Permission denied");
  await page.getByRole("combobox", { name: "Search TinyDash" }).fill("missing");
  await expect(
    page.getByRole("heading", { name: "No files found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await selectCategory(page, "Apps");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("previews plain text, copies by ID, and deletes without hiding the launcher", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "<script>literal text</script>",
  );
  await expect(page.getByLabel("Saved clipboard text")).toContainText(
    "  Keep the original spacing. 🚀",
  );
  await page.screenshot({ path: test.info().outputPath("clipboard.png") });
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "clipboard:1", action: "copy" },
      },
    ]);
  await input.press("Meta+Backspace");
  await expect(page.locator(".result-title")).toHaveText("Project link");
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  await expect
    .poll(async () => (await actions(page)).at(-1))
    .toEqual({
      command: "execute_action",
      payload: { id: "clipboard:1", action: "delete" },
    });
  await expect(input).toBeFocused();
});

test("ignores late clipboard previews and keeps the layout usable at narrow widths", async ({
  page,
}) => {
  await openLauncher(page);
  await page.evaluate(() => {
    window.__launcherTest.slowPreview = true;
  });
  await selectCategory(page, "Clipboard");
  await expect(page.locator(".result-row")).toHaveCount(2);
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .press("ArrowDown");
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  await page.waitForTimeout(300);
  await expect(page.getByLabel("Saved clipboard text")).toHaveText(
    "https://example.com/project",
  );
  for (const width of [320, 720]) {
    await page.setViewportSize({ width, height: 550 });
    await expect(page.getByLabel("Saved clipboard text")).toBeInViewport();
    await expect(
      page.getByRole("button", { name: "Copy text", exact: true }),
    ).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }
});

test("clear requires confirmation, keeps Cancel focused, and reports storage errors", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Clipboard");
  const clear = page.getByRole("button", {
    name: "Clear unpinned",
    exact: true,
  });
  await clear.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".result-row")).toHaveCount(2);
  await clear.click();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.evaluate(() => {
    window.__launcherTest.rejectClear = true;
  });
  await clear.click();
  await dialog.getByRole("button", { name: "Clear unpinned" }).click();
  await expect(dialog.getByRole("alert")).toHaveText(
    "Error: Could not delete clipboard history.",
  );
  await expect(page.locator(".result-row")).toHaveCount(2);
  await page.evaluate(() => {
    window.__launcherTest.rejectClear = false;
  });
  await dialog.getByRole("button", { name: "Clear unpinned" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "No saved clipboard text" }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
});

test("shows a storage warning while search and launch remain available", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  await page.evaluate(() => {
    window.__launcherTest.storageError =
      "Usage history could not be saved. Ranking changes will be lost when TinyDash quits.";
    return window.__launcherTest.emit("usage-changed", null);
  });
  await expect(page.getByRole("alert")).toContainText(
    "Usage history could not be saved",
  );
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    ]);
});

test("refreshes Rust ranking when the launcher preserves the query", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("app");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Finder");
  await page.evaluate(() => {
    window.__launcherTest.usedAppFirst = true;
    return window.__launcherTest.emit("launcher-opened", false);
  });
  await expect(input).toHaveValue("app");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Safari");
  await expect(input).toBeFocused();
});

test("focuses the query, wraps selection, and launches the selected app", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await input.press("ArrowUp");
  await expect(
    page.getByRole("listbox").getByRole("option").last(),
  ).toHaveAttribute("aria-selected", "true");
  await input.press("ArrowDown");
  await input.press("ArrowDown");
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-1", action: "launch" } },
    ]);
});

test("uses result shortcuts and the reveal action", async ({ page }) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  await page.keyboard.press("Meta+3");
  await expect.poll(async () => (await actions(page)).length).toBe(1);
  await page
    .getByRole("combobox", { name: "Search TinyDash" })
    .press("Meta+Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-2", action: "launch" } },
      { command: "execute_action", payload: { id: "app-0", action: "reveal" } },
    ]);
});

test("the actions menu keeps native button keyboard behavior and closes with Escape", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const button = page.getByRole("button", { name: "Actions" });
  await button.focus();
  await button.press("Enter");
  await expect(page.getByRole("menu")).toBeVisible();
  await expect(
    page.getByRole("searchbox", { name: "Search actions" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Open application" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      { command: "execute_action", payload: { id: "app-0", action: "reveal" } },
    ]);
  await page.keyboard.press("Meta+k");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          window.__launcherTest.calls.filter(
            (call) => call.command === "hide_launcher",
          ).length,
      ),
    )
    .toBe(1);
});

test("ignores late results and prevents launching stale results", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("slow");
  await input.press("Enter");
  expect(await actions(page)).toHaveLength(0);
  await input.fill("sa");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "Safari",
  );
  await page.waitForTimeout(300);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "Safari",
  );
});

test("shows action errors, handles empty results, and ignores IME confirmation", async ({
  page,
}) => {
  await openLauncher(page);
  await selectCategory(page, "Apps");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.dispatchEvent("keydown", {
    key: "Enter",
    isComposing: true,
    bubbles: true,
  });
  expect(await actions(page)).toHaveLength(0);
  await page.evaluate(() => {
    window.__launcherTest.rejectActions = true;
  });
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText(
    "Could not open the application",
  );
  await input.fill("missing");
  await expect(
    page.getByRole("heading", { name: "No results found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(8);
});

test("reopening clears or selects the prior query as configured", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("sa");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(input).toHaveValue("sa");
  await expect(input).toBeFocused();
  expect(
    await input.evaluate((element) => [
      (element as HTMLInputElement).selectionStart,
      (element as HTMLInputElement).selectionEnd,
    ]),
  ).toEqual([0, 2]);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(input).toHaveValue("");
  await expect(
    page.getByRole("heading", { name: "What will you do next?" }),
  ).toBeVisible();
  await expect(page.getByRole("option")).toHaveCount(0);
});

test("the layout fits narrow windows and the desktop window", async ({
  page,
}) => {
  await openLauncher(page);
  for (const width of [320, 375, 414, 720, 768]) {
    await page.setViewportSize({ width, height: 550 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await expect(
      page.getByRole("button", { name: "Actions" }),
    ).toBeInViewport();
  }
  await page.setViewportSize({ width: 720, height: 550 });
  await page.screenshot({ path: test.info().outputPath("launcher.png") });
});

test("copies calculation results and offers only their supported actions", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("12 * 8");
  await expect(page.getByRole("listbox").getByRole("option")).toHaveCount(1);
  await expect(page.getByRole("listbox").getByRole("option")).toContainText(
    "96",
  );
  await expect(
    page.getByRole("button", { name: "Copy result", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("searchbox", { name: "Search actions" }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Copy result" }),
  ).toBeFocused();
  await expect(
    page.getByRole("menuitem", { name: "Show in folder" }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await input.press("Enter");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "calculation:1", action: "copy" },
      },
    ]);
  await page.screenshot({ path: test.info().outputPath("calculator.png") });
});

test("changes search mode during a pending query and copies an emoji", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("slow");
  await selectCategory(page, "Emoji");
  await expect(input).toBeFocused();
  await expect(
    page.getByRole("listbox").getByRole("option").filter({ hasText: "rocket" }),
  ).toHaveCount(1);
  await expect(page.locator(".emoji-icon")).toHaveText("🚀");
  await page.waitForTimeout(300);
  await expect(page.locator(".result-title")).toHaveText("rocket");
  await page.keyboard.press("Meta+1");
  await expect
    .poll(() => actions(page))
    .toEqual([
      {
        command: "execute_action",
        payload: { id: "emoji:🚀", action: "copy" },
      },
    ]);
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", false),
  );
  await expect(
    page
      .getByRole("navigation", { name: "Search categories" })
      .locator("[aria-pressed=true]"),
  ).toHaveAccessibleName("Emoji");
  await page.evaluate(() =>
    window.__launcherTest.emit("launcher-opened", true),
  );
  await expect(
    page
      .getByRole("navigation", { name: "Search categories" })
      .locator("[aria-pressed=true]"),
  ).toHaveAccessibleName("All");
});

test("shows calculator errors and fits both new result types in a narrow window", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("=1 / 0");
  await expect(page.getByRole("alert")).toContainText("Division by zero");
  await input.press("Enter");
  expect(await actions(page)).toHaveLength(0);
  for (const query of ["12 * 8", ":rocket"]) {
    await input.fill(query);
    await expect(page.locator(".result-row")).toHaveCount(1);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await page.setViewportSize({ width: 320, height: 550 });
    await expect(
      page
        .getByRole("navigation", { name: "Search categories" })
        .locator("[aria-pressed=true]"),
    ).toBeInViewport();
    await expect(
      page.getByRole("button", { name: "Actions" }),
    ).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  }
  await page.setViewportSize({ width: 720, height: 550 });
  await page.screenshot({ path: test.info().outputPath("emoji.png") });
});
