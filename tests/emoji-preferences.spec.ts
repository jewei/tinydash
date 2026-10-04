import { expect, test, type Page } from "@playwright/test";
import type {} from "./mock-backend";

async function open(page: Page, settings = true) {
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
  if (settings)
    await page.getByRole("button", { name: "Search", exact: true }).click();
}

test("emoji settings save, reload, and discard tone and language choices", async ({
  page,
}) => {
  await open(page);
  const tone = page.getByRole("combobox", { name: "Preferred skin tone" });
  const chinese = page.getByRole("checkbox", { name: /Simplified Chinese/ });
  const malay = page.getByRole("checkbox", { name: /Malay/ });
  const spanish = page.getByRole("checkbox", { name: /Spanish/ });
  await expect(tone).toHaveValue("0");
  await expect(chinese).not.toBeChecked();
  await tone.selectOption("3");
  await chinese.check();
  await malay.check();
  await spanish.check();
  await expect(page.getByLabel("Skin tone preview")).toContainText("👍🏽");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByText("Changes saved.", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(() => ({
      tone: window.__launcherTest.settings.emojiSkinTone,
      languages: window.__launcherTest.settings.emojiLanguages,
    })),
  ).toEqual({ tone: 3, languages: ["zh", "ms", "es"] });
  await page.reload();
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(tone).toHaveValue("3");
  await expect(chinese).toBeChecked();
  await expect(malay).toBeChecked();
  await expect(spanish).toBeChecked();
  await tone.selectOption("5");
  await chinese.uncheck();
  await page.getByRole("button", { name: "Discard", exact: true }).click();
  await expect(tone).toHaveValue("3");
  await expect(chinese).toBeChecked();
});

test("settings search finds emoji controls and a failed save keeps the draft", async ({
  page,
}) => {
  await open(page);
  await page
    .getByRole("searchbox", { name: "Search settings" })
    .fill("skin tone");
  await expect(
    page.getByRole("combobox", { name: "Preferred skin tone" }),
  ).toBeVisible();
  await page
    .getByRole("combobox", { name: "Preferred skin tone" })
    .selectOption("2");
  await page.evaluate(() => {
    window.__launcherTest.rejectSettings = "Settings could not be saved.";
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Settings could not be saved.",
  );
  await expect(
    page.getByRole("combobox", { name: "Preferred skin tone" }),
  ).toHaveValue("2");
  expect(
    await page.evaluate(() => window.__launcherTest.settings.emojiSkinTone),
  ).toBe(0);
});

test("emoji actions send the exact displayed variant ID", async ({ page }) => {
  await open(page, false);
  // Synthetic result checks rendering and dispatch; the Rust provider tests search.
  await page.evaluate(() => {
    window.__launcherTest.suggestions = [
      {
        id: "emoji:👍🏽",
        kind: "emoji",
        title: "thumbs up: medium skin tone",
        subtitle: "People & body",
        icon: "👍🏽",
        primaryAction: "copy",
        secondaryActions: [],
        score: 1,
      },
    ];
  });
  const input = page.getByRole("combobox", { name: "Search TinyDash" });
  await input.fill("missing");
  await expect(page.getByRole("option")).toHaveCount(0);
  await input.fill("");
  const result = page.getByRole("option", {
    name: /thumbs up: medium skin tone/,
  });
  await expect(result.locator(".emoji-icon")).toHaveText("👍🏽");
  await input.press("Enter");
  expect(
    await page.evaluate(
      () =>
        window.__launcherTest.calls.find(
          (call) => call.command === "execute_action",
        )?.payload,
    ),
  ).toMatchObject({ id: "emoji:👍🏽", action: "copy" });
});
