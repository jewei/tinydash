import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { renderPage } from "../render";
import { detectPlatform, validateRelease, type Release } from "../downloads";

const version = "0.1.0";
const base = "https://github.com/jewei/tinydash/releases";
const release: Release = {
  version,
  releaseUrl: `${base}/tag/v${version}`,
  publishedAt: "2026-09-19T00:00:00Z",
  downloads: Object.fromEntries(
    Object.entries({
      macos: "TinyDash_0.1.0_aarch64.dmg",
      windows: "TinyDash_0.1.0_x64-setup.exe",
      linux: "tinydash_0.1.0_amd64.deb",
    }).map(([platform, filename]) => [
      platform,
      {
        url: `${base}/download/v${version}/${filename}`,
        bytes: 12_582_912,
        sha256: "a".repeat(64),
        requirement:
          platform === "macos"
            ? "macOS 26"
            : platform === "windows"
              ? "Windows 11"
              : "Ubuntu 24.04",
      },
    ]),
  ),
};

test("release data rejects partial, unsafe, and mismatched downloads", () => {
  expect(validateRelease(release)).toEqual(release);
  for (const edit of [
    (value: Release) => {
      delete value.downloads.linux;
    },
    (value: Release) => {
      value.downloads.macos!.url = "https://example.com/installer.dmg";
    },
    (value: Release) => {
      value.downloads.windows!.url = `${base}/download/v0.1.0/TinyDash_0.2.0_x64-setup.exe`;
    },
    (value: Release) => {
      value.downloads.linux!.sha256 = "";
    },
    (value: Release) => {
      value.version = "0.1.0-rc.1";
    },
    (value: Release) => {
      value.downloads.macos!.bytes = 0;
    },
    (value: Release) => {
      value.downloads.macos!.requirement = "";
    },
    (value: Release) => {
      value.version = null;
    },
  ]) {
    const changed = structuredClone(release);
    edit(changed);
    expect(() => validateRelease(changed)).toThrow();
  }
});

test("mobile, ChromeOS, and unknown systems keep manual selection", () => {
  for (const agent of [
    "Android Linux",
    "iPhone Mac OS X",
    "iPad",
    "CrOS Linux",
    "Unknown",
  ])
    expect(detectPlatform(agent)).toBeNull();
  expect(detectPlatform("Macintosh", 5)).toBeNull();
});

test("published downloads work with JavaScript off", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  const template = await readFile(
    new URL("../index.html", import.meta.url),
    "utf8",
  );
  await page.route("**/tinydash/", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: renderPage(template, release),
    }),
  );
  await page.goto("http://127.0.0.1:4174/tinydash/");
  await expect(page.locator("#release-status")).toContainText("Version 0.1.0");
  await expect(page.locator("[data-download][href]")).toHaveCount(3);
  for (const platform of ["macos", "windows", "linux"] as const) {
    await expect(
      page.locator(`[data-platform="${platform}"] [data-download]`),
    ).toHaveAttribute("href", release.downloads[platform]!.url);
    await expect(
      page.locator(`[data-platform="${platform}"] [data-detail]`),
    ).toContainText(release.downloads[platform]!.requirement);
  }
  await expect(page.locator("body")).not.toContainText("{{");
  await context.close();
});

test("a failed refresh keeps the verified static downloads", async ({
  page,
}) => {
  const template = await readFile(
    new URL("../index.html", import.meta.url),
    "utf8",
  );
  await page.route("**/tinydash/", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: renderPage(template, release),
    }),
  );
  await page.route("**/release.json", (route) => route.abort());
  await page.goto("./");
  await expect(page.locator("#release-status")).toContainText(
    "listed downloads are still available",
  );
  await expect(page.locator("[data-download][href]")).toHaveCount(3);
});

test("release requirements render as text", async () => {
  const template = await readFile(
    new URL("../index.html", import.meta.url),
    "utf8",
  );
  const changed = structuredClone(release);
  changed.downloads.macos!.requirement = '<img src="x" onerror="alert(1)">';
  const page = renderPage(template, changed);
  expect(page).toContain("&lt;img");
  expect(page).not.toContain('onerror="alert(1)"');
});

test("the laptop view shows the title and download action without scrolling", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("./");
  await page.evaluate(() => document.fonts.ready);
  for (const selector of ["h1", ".intro-copy", "#primary-download"]) {
    const box = await page.locator(selector).boundingBox();
    expect(box!.y + box!.height).toBeLessThan(800);
  }
  await page.screenshot({
    path: testInfo.outputPath("download-page-1280.png"),
  });
});

for (const [platform, agent, label] of [
  ["macos", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)", "macOS"],
  ["windows", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)", "Windows"],
  ["linux", "Mozilla/5.0 (X11; Linux x86_64)", "Ubuntu"],
  ["unknown", "Unknown", "Choose your download"],
]) {
  test(`download choice for ${platform} keeps all platforms available`, async ({
    browser,
  }) => {
    const context = await browser.newContext({ userAgent: agent });
    const page = await context.newPage();
    const requests: string[] = [];
    page.on("request", (request) => requests.push(request.url()));
    await page.route("**/release.json", (route) =>
      route.fulfill({ json: release }),
    );
    await page.goto("http://127.0.0.1:4174/tinydash/");
    await expect(page.locator("#release-status")).toContainText(
      "Version 0.1.0",
    );
    await expect(page.locator("#primary-download")).toContainText(label);
    await expect(page.locator("[data-download][href]")).toHaveCount(3);
    if (platform !== "unknown") {
      await expect(page.locator("#primary-download")).toHaveAttribute(
        "href",
        release.downloads[platform as keyof typeof release.downloads]!.url,
      );
      await expect(page.locator("#primary-detail")).toContainText(
        platform === "macos" ? "Apple silicon" : "x64",
      );
      if (platform === "windows") {
        await expect(page.locator("#primary-detail")).toContainText(
          "Unsigned preview. Windows may show a warning or block installation.",
        );
        await expect(page.locator('[data-platform="windows"]')).toContainText(
          "Unsigned preview.",
        );
      }
    }
    expect(requests.filter((url) => /\.(dmg|exe|deb)$/.test(url))).toEqual([]);
    await context.close();
  });
}

test("an unpublished or failed release list never creates installer links", async ({
  page,
}) => {
  await page.goto("./");
  await expect(page.locator("#release-status")).toContainText(
    "not available yet",
  );
  await expect(page.locator("[data-download][href]")).toHaveCount(0);
  await page.route("**/release.json", (route) => route.abort());
  await page.reload();
  await expect(page.locator("#release-status")).toContainText(
    "download list is unavailable",
  );
  await expect(page.locator("[data-download][href]")).toHaveCount(0);
});

for (const width of [320, 375, 414, 768, 1440]) {
  test(`page and enabled download controls fit at ${width}px`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await page.route("**/release.json", (route) =>
      route.fulfill({ json: release }),
    );
    await page.goto("./");
    await expect(page.locator("[data-download][href]")).toHaveCount(3);
    await page.evaluate(() => document.fonts.ready);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBe(width);
    for (const control of await page
      .locator(".button, .site-header a, .site-footer a, .download-links a")
      .all()) {
      const box = await control.boundingBox();
      expect(box!.x).toBeGreaterThanOrEqual(0);
      expect(box!.x + box!.width).toBeLessThanOrEqual(width + 1);
      expect(
        await control.evaluate((node) => node.scrollWidth <= node.clientWidth),
      ).toBe(true);
    }
    await expect(page.locator(".product-view img")).toHaveJSProperty(
      "naturalWidth",
      2052,
    );
    await page.screenshot({
      path: testInfo.outputPath(`download-page-${width}.png`),
      fullPage: true,
    });
  });
}

test("keyboard navigation has a visible focus indicator and opens checksum details", async ({
  page,
}) => {
  await page.route("**/release.json", (route) =>
    route.fulfill({ json: release }),
  );
  await page.goto("./");
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Skip to content" }),
  ).toBeFocused();
  expect(
    await page
      .locator(":focus")
      .evaluate((node) => getComputedStyle(node).outlineStyle),
  ).toBe("solid");
  await page.locator(".checksum summary").first().focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-checksum]").first()).toBeVisible();
});

test("text and focus colors meet contrast requirements", async ({ page }) => {
  await page.goto("./");
  const ratios = await page.evaluate(() => {
    const styles = getComputedStyle(document.documentElement);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 1;
    const context = canvas.getContext("2d")!;
    const luminance = (token: string) => {
      context.fillStyle = styles.getPropertyValue(token).trim();
      context.fillRect(0, 0, 1, 1);
      const values = [...context.getImageData(0, 0, 1, 1).data]
        .slice(0, 3)
        .map((n) => {
          const value = n / 255;
          return value <= 0.04045
            ? value / 12.92
            : ((value + 0.055) / 1.055) ** 2.4;
        });
      return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
    };
    return [
      ["--color-ink", "--color-paper", 4.5],
      ["--color-muted", "--color-field", 4.5],
      ["--color-accent", "--color-paper", 4.5],
      ["--color-accent-ink", "--color-accent", 4.5],
      ["--color-focus", "--color-paper", 3],
      ["--color-focus", "--color-field", 3],
    ].map(([fg, bg, minimum]) => {
      const a = luminance(fg as string),
        b = luminance(bg as string);
      return {
        ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05),
        minimum: Number(minimum),
      };
    });
  });
  for (const { ratio, minimum } of ratios)
    expect(ratio).toBeGreaterThanOrEqual(minimum);
});
