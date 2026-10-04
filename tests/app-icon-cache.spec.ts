import { expect, test } from "@playwright/test";

// Exercise ownership and repeat work in the real cache module. These counts
// do not measure WebKit memory or native image latency.
test("warm icons reuse payloads within count and byte limits", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const cachePath = "/src/app-icons.ts";
    const bridgePath = "/src/bridge.ts";
    const cache = (await import(
      cachePath
    )) as typeof import("../src/app-icons");
    const { backend } = (await import(
      bridgePath
    )) as typeof import("../src/bridge");
    const calls: string[] = [];
    let payload = "data:image/png;base64,fixture";
    backend.appIcon = async (key) => {
      calls.push(key);
      return payload;
    };
    backend.cancelAppIcon = async () => {};
    async function use(key: string, pixels = 72) {
      let release!: () => void;
      await new Promise<void>((resolve) => {
        release = cache.loadAppIcon(key, pixels, (url) => {
          if (url !== undefined) resolve();
        });
      });
      release();
      release(); // A duplicate release must not alter the next owner's entry.
    }
    await use("app-icon:1:0");
    await use("app-icon:1:0");
    const warmCalls = calls.length;
    await use("app-icon:1:0", 128);
    await use("app-icon:2:0");
    const identityCalls = calls.length;
    for (let i = 1; i <= 64; i++) await use(`app-icon:2:${i}`);
    const beforeEvicted = calls.length;
    await use("app-icon:1:0");
    const evictedCalls = calls.length - beforeEvicted;
    // Two 300 KiB UTF-16 payloads cannot both fit in the 512 KiB idle budget.
    payload = "x".repeat(150 * 1024);
    await use("app-icon:large:0");
    await use("app-icon:large:1");
    const beforeLarge = calls.length;
    await use("app-icon:large:0");
    const byteEvictions = calls.length - beforeLarge;
    payload = "x".repeat(300 * 1024);
    await use("app-icon:oversized:0");
    const beforeOversized = calls.length;
    await use("app-icon:oversized:0");
    const oversizedCalls = calls.length - beforeOversized;
    cache.disposeAppIcons();
    const beforeDisposed = calls.length;
    cache.loadAppIcon("app-icon:after-disposal", 72, () => {});
    await Promise.resolve();
    return {
      warmCalls,
      identityCalls,
      evictedCalls,
      byteEvictions,
      oversizedCalls,
      disposedCalls: calls.length - beforeDisposed,
    };
  });
  expect(result).toEqual({
    warmCalls: 1,
    identityCalls: 3,
    evictedCalls: 1,
    byteEvictions: 1,
    oversizedCalls: 1,
    disposedCalls: 0,
  });
});

test("shared consumers cancel only the final pending owner and reject stale replies", async ({
  page,
}) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    const cachePath = "/src/app-icons.ts";
    const bridgePath = "/src/bridge.ts";
    const cache = (await import(
      cachePath
    )) as typeof import("../src/app-icons");
    const { backend } = (await import(
      bridgePath
    )) as typeof import("../src/bridge");
    const pending: { request: string; resolve: (url: string) => void }[] = [];
    const canceled: string[] = [];
    backend.appIcon = (_key, _pixels, request) =>
      new Promise((resolve) => pending.push({ request, resolve }));
    backend.cancelAppIcon = async (request) => {
      canceled.push(request);
    };
    const seen: string[] = [];
    const consume = (url: string | undefined) => {
      if (url) seen.push(url);
    };
    const first = cache.loadAppIcon("app-icon:pending", 72, consume);
    const second = cache.loadAppIcon("app-icon:pending", 72, (url) =>
      consume(url),
    );
    for (let i = 0; !pending.length && i < 100; i++)
      await new Promise((resolve) => setTimeout(resolve, 1));
    if (pending.length !== 1) throw new Error("Expected one shared request");
    first();
    const canceledWhileOwned = canceled.length;
    second();
    const third = cache.loadAppIcon("app-icon:pending", 72, consume);
    first();
    second();
    for (let i = 0; pending.length < 2 && i < 100; i++)
      await new Promise((resolve) => setTimeout(resolve, 1));
    if (Number(pending.length) !== 2)
      throw new Error("Expected a new request after cancellation");
    pending[0].resolve("stale");
    pending[1].resolve("current");
    await new Promise((resolve) => setTimeout(resolve, 0));
    third();
    cache.disposeAppIcons();
    return {
      seen,
      canceledWhileOwned,
      canceled,
      firstRequest: pending[0].request,
      distinctRequests: pending[0].request !== pending[1].request,
    };
  });
  expect(result.seen).toEqual(["current"]);
  expect(result.canceledWhileOwned).toBe(0);
  expect(result.canceled).toEqual([result.firstRequest]);
  expect(result.distinctRequests).toBe(true);
});
