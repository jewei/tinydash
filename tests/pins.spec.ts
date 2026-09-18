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
    page.getByRole("listbox", { name: "Search results" }).getByRole("option"),
  ).toHaveCount(8);
}

test("app pins persist, preserve selection, and can be removed in compact mode", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const rows = page
    .getByRole("listbox", { name: "Search results" })
    .getByRole("option");
  await input.press("ArrowDown");
  await page
    .getByRole("button", { name: "Pin application", exact: true })
    .click();
  await expect(rows.first()).toContainText("Safari");
  await expect(rows.first()).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("button", { name: "Unpin application", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".result-group").first()).toHaveText("Pinned1");
  await page.reload();
  await expect(rows.first()).toContainText("Safari");
  await input.fill("sa");
  await expect(rows).toHaveCount(1);
  await expect(page.locator(".result-group")).toHaveText("Applications1");
  await page.getByRole("button", { name: "Clear search", exact: true }).click();
  await expect(rows).toHaveCount(8);
  await input.press("Meta+k");
  await page
    .getByRole("menuitemradio", { name: "Compact", exact: true })
    .click();
  await input.press("Meta+k");
  await page
    .getByRole("menuitem", { name: "Unpin application", exact: true })
    .click();
  await expect(rows.first()).toContainText("Finder");
  await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
  await expect(input).toBeFocused();
  await page.reload();
  await expect(rows.first()).toContainText("Finder");
});

test("a failed pin write leaves the list unchanged and permits retry", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.press("ArrowDown");
  await page.evaluate(() => {
    window.__launcherTest.rejectPin = true;
  });
  const pin = page.getByRole("button", {
    name: "Pin application",
    exact: true,
  });
  await pin.click();
  await expect(page.getByRole("alert")).toContainText(
    "Could not save the app pin",
  );
  await expect(pin).toBeEnabled();
  await expect(pin).toHaveAttribute("aria-pressed", "false");
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Finder");
  await page.evaluate(() => {
    window.__launcherTest.rejectPin = false;
  });
  await pin.click();
  await expect(
    page.getByRole("listbox").getByRole("option").first(),
  ).toContainText("Safari");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("category buttons and option-arrow shortcuts select the same search scope", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const categories = page.getByRole("navigation", {
    name: "Search categories",
  });
  await categories.getByRole("button", { name: "Apps", exact: true }).click();
  await expect(input).toBeFocused();
  await expect(
    categories.getByRole("button", { name: "Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await input.press("Alt+ArrowRight");
  await expect(
    categories.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(input).toHaveAttribute(
    "placeholder",
    "Search filenames and paths...",
  );
  await input.press("Alt+ArrowLeft");
  await expect(
    categories.getByRole("button", { name: "Apps", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
});
