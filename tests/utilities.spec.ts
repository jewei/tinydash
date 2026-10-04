import { expect, test, type Page } from "@playwright/test";

async function open(page: Page) {
  await page.route(
    (url) => url.pathname === "/src/index.tsx",
    (route) =>
      route.fulfill({
        contentType: "application/javascript",
        body: 'import "/tests/utilities-harness.tsx";',
      }),
  );
  await page.goto("/");
  await expect(
    page.getByRole("region", { name: "Native utilities" }),
  ).toBeVisible();
}
async function calls(page: Page, command: string) {
  return page.evaluate(
    (name) =>
      (
        window as unknown as {
          utilityCalls: { command: string; args: Record<string, unknown> }[];
        }
      ).utilityCalls.filter((call) => call.command === name),
    command,
  );
}

test("process confirmation defaults to cancel, binds identity and requires explicit consent", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("textbox", { name: "Filter processes" }).fill("editor");
  await expect(page.locator(".utility-processes li")).toHaveCount(1);
  await page.getByRole("button", { name: "Force kill", exact: true }).click();
  await expect(page.getByRole("alertdialog")).toContainText("PID 400");
  await expect(
    page.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  expect(await calls(page, "utility_confirm_process")).toHaveLength(0);
  await page.getByRole("button", { name: "Quit", exact: true }).click();
  await page.getByRole("button", { name: "Confirm quit", exact: true }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  expect((await calls(page, "utility_prepare_process"))[1].args).toEqual({
    process: { pid: 400, identity: "start-one", name: "Fixture editor" },
    force: false,
  });
  expect((await calls(page, "utility_confirm_process"))[0].args).toEqual({
    token: "one-use-ticket",
    confirmed: true,
  });
});

test("color conversion errors recover and keep awake remains visible after close without idle polling", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Colors", exact: true }).click();
  await page.getByRole("textbox", { name: "Color", exact: true }).fill("bad");
  await page.getByRole("button", { name: "Convert color" }).click();
  await expect(page.getByRole("alert")).toContainText("Invalid color");
  await page.getByRole("textbox", { name: "Color", exact: true }).fill("#f00");
  await page.getByRole("button", { name: "Convert color" }).click();
  await expect(
    page.getByRole("textbox", { name: "Converted color" }).first(),
  ).toHaveValue("#FF0000");
  await page.getByRole("button", { name: "Keep awake", exact: true }).click();
  await page
    .getByRole("spinbutton", { name: "Duration in minutes" })
    .fill("481");
  await expect(
    page.getByRole("button", { name: "Start keep awake" }),
  ).toBeDisabled();
  await page
    .getByRole("spinbutton", { name: "Duration in minutes" })
    .fill("15");
  await page.getByRole("button", { name: "Start keep awake" }).click();
  await expect(page.locator(".utility-awake-state")).toContainText(
    "Keeping awake",
  );
  await page.getByRole("button", { name: "Close utilities" }).click();
  await expect(
    page.getByRole("region", { name: "Native utilities" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: /Awake until/ }).click();
  await expect(page.getByRole("button", { name: /Awake until/ })).toHaveCount(
    0,
  );
  const count = (await calls(page, "utility_awake_status")).length;
  await page.clock.install();
  await page.clock.fastForward(60000);
  expect(await calls(page, "utility_awake_status")).toHaveLength(count);
  expect(
    (await calls(page, "utility_set_awake")).map((call) => call.args.minutes),
  ).toEqual([15, 0]);
});

test("native sampler capability dispatches IPC and cancellation is not an error", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Colors", exact: true }).click();
  const picker = page.getByRole("button", { name: "Pick screen color" });
  await picker.click();
  await expect(picker).toBeEnabled();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await picker.click();
  await expect(
    page.getByRole("textbox", { name: "Converted color" }).first(),
  ).toHaveValue("#00FF00");
  expect(await calls(page, "utility_eyedropper")).toHaveLength(2);
  await page
    .getByRole("textbox", { name: "Color", exact: true })
    .fill("an unconverted draft");
  for (const format of ["HEX", "RGB", "HSL"]) {
    await page
      .getByRole("button", { name: `Copy ${format}`, exact: true })
      .click();
    await expect(page.getByRole("status")).toHaveText(
      `${format} copied to clipboard.`,
    );
  }
  expect(
    (await calls(page, "utility_copy_color")).map((call) => call.args),
  ).toEqual([
    { input: "#00FF00", format: "hex" },
    { input: "#00FF00", format: "rgb" },
    { input: "#00FF00", format: "hsl" },
  ]);
});

test("media dispatch and captured window placement report native errors", async ({
  page,
}) => {
  await open(page);
  await page.getByRole("button", { name: "Media", exact: true }).click();
  await page.getByRole("button", { name: "Play / Pause" }).click();
  expect((await calls(page, "utility_media"))[0].args.action).toBe("playPause");
  await page.getByRole("button", { name: "Windows", exact: true }).click();
  await page
    .getByRole("button", { name: "Capture window in 3 seconds" })
    .click();
  await expect(
    page.getByText("Target: Fixture editor (PID 400)"),
  ).toBeVisible();
  expect(
    (await calls(page, "utility_capture_window")).map(
      (call) => call.args.delayed,
    ),
  ).toEqual([false, true]);
  await page.getByRole("button", { name: "Left half" }).click();
  expect((await calls(page, "utility_window"))[0].args.action).toBe("left");
  await page.getByRole("button", { name: "Maximize", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Accessibility permission denied",
  );
  await page.getByRole("button", { name: "Restore captured bounds" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
});
