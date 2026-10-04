import { expect, test } from "@playwright/test";

test("an icon replaces its SVG shape when its name changes and keeps size reactive", async ({
  page,
}) => {
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    (route) =>
      route.fulfill({
        contentType: "application/javascript",
        body: 'import "/tests/fixtures/icons.tsx";',
      }),
  );
  await page.goto("/");
  const icon = page.locator("svg");
  await expect(icon.locator("rect")).toHaveCount(1);
  await expect(icon).toHaveAttribute("width", "20");
  await icon.evaluate((element) =>
    element.setAttribute("data-owned", "stable"),
  );
  await page.getByRole("button", { name: "Change glyph" }).click();
  await expect(icon.locator("rect")).toHaveCount(0);
  await expect(icon.locator("circle")).toHaveCount(1);
  await expect(icon).toHaveAttribute("data-owned", "stable");
  await page.getByRole("button", { name: "Change size" }).click();
  await expect(icon).toHaveAttribute("width", "32");
  await expect(icon).toHaveAttribute("height", "32");
  await page.getByRole("button", { name: "Change glyph" }).click();
  await expect(icon.locator("circle")).toHaveCount(0);
  await expect(icon.locator("rect")).toHaveCount(1);
  expect(
    await icon.evaluate((element) =>
      [element, ...element.children].every(
        (node) => node.namespaceURI === "http://www.w3.org/2000/svg",
      ),
    ),
  ).toBe(true);
});
