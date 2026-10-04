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

test("the search clear control stays available after an empty result or error", async ({
  page,
}) => {
  await openLauncher(page);
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  const clear = page.locator(".search-field").getByRole("button", {
    name: "Clear search",
    exact: true,
  });
  await input.fill("missing");
  await expect(
    page.getByRole("heading", { name: "No results found" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Try another search.", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Start typing.", exact: true }),
  ).toHaveCount(0);
  await clear.click();
  await expect(input).toHaveValue("");
  await expect(input).toBeFocused();
  await expect(clear).toHaveCount(0);

  await input.fill("error");
  await expect(page.getByRole("alert")).toContainText(
    "The application index is unavailable",
  );
  await expect(
    page.getByRole("heading", { name: "Search unavailable", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "No results found", exact: true }),
  ).toHaveCount(0);
  await clear.click();
  await expect(input).toHaveValue("");
  await expect(input).toBeFocused();
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("reduced motion keeps menus and confirmations immediate and usable", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openLauncher(page);
  await page.getByRole("button", { name: "Actions" }).click();
  await expect(page.locator(".actions-menu")).toBeVisible();
  await expect(page.locator(".actions-menu")).toHaveCSS(
    "animation-name",
    "none",
  );
  await expect(page.locator(".actions-filter")).toBeFocused();
  await page.keyboard.press("Escape");
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await expect(input).toBeFocused();
  await input.fill("reboot");
  await expect(page.getByRole("option").first()).toContainText("Restart");
  await input.press("Enter");
  const dialog = page.locator(".confirm-dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog).toHaveCSS("animation-name", "none");
  const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
  await expect(cancel).toBeFocused();
  await expect(cancel).toHaveCSS("transition-duration", "0s");
  await cancel.click();
  await expect(dialog).toHaveCount(0);
  await expect(input).toBeFocused();
  expect(
    await page.evaluate(() =>
      window.__launcherTest.calls.filter(
        ({ command }) => command === "execute_action",
      ),
    ),
  ).toEqual([]);
});

test("welcome actions omit unavailable result commands and remain searchable", async ({
  page,
}) => {
  await openLauncher(page);
  await page.getByRole("button", { name: "Actions" }).click();
  const menu = page.getByRole("menu", { name: "Launcher actions" });
  await expect(
    menu.getByRole("menuitem", { name: "Open application" }),
  ).toHaveCount(0);
  await expect(
    menu.getByRole("menuitemradio", { name: "Light", exact: true }),
  ).toBeVisible();
  const filter = menu.getByRole("searchbox", { name: "Search actions" });
  await filter.fill("settings");
  const settings = menu.getByRole("menuitem", { name: "Settings" });
  await expect(settings).toBeVisible();
  await filter.press("ArrowDown");
  await expect(settings).toBeFocused();
  await filter.fill("no such action");
  await expect(menu.getByRole("status")).toHaveText("No matching actions");
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("combobox", { name: "Search TinyDash" }),
  ).toBeFocused();
});

test("desktop previews keep saved dates and password guidance in view", async ({
  page,
}) => {
  await page.setViewportSize({ width: 980, height: 620 });
  await openLauncher(page);
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Clipboard", exact: true })
    .click();
  await expect(
    page.getByRole("complementary", { name: "Clipboard preview" }),
  ).toBeVisible();
  await expect(page.locator(".clipboard-date")).toBeInViewport({ ratio: 1 });
  await page
    .getByRole("navigation", { name: "Search categories" })
    .getByRole("button", { name: "Passwords", exact: true })
    .click();
  await expect(
    page.getByText("Copying here skips TinyDash's clipboard history.", {
      exact: true,
    }),
  ).toBeInViewport({ ratio: 1 });
});
