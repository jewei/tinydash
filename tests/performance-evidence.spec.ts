import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import {
  measureQueryTiming,
  percentiles,
  summarizeTimings,
  type QueryTiming,
} from "./native/performance";

// The desktop smoke runs under Bun. Exercise Bun's actual serialized function
// rather than assuming Playwright's TypeScript transform emits identical JS.
const nativeTimingSource = execFileSync(
  "bun",
  [
    "-e",
    'import { measureQueryTiming } from "./tests/native/performance.ts"; process.stdout.write(measureQueryTiming.toString());',
  ],
  { cwd: resolve(import.meta.dirname, ".."), encoding: "utf8" },
);

test("native timing percentiles use nearest rank without changing samples", () => {
  const samples = [100, ...Array.from({ length: 99 }, (_, index) => index + 1)];
  expect(percentiles(samples)).toEqual({
    count: 100,
    p50: 50,
    p95: 95,
    p99: 99,
    max: 100,
  });
  expect(samples[0]).toBe(100);
  expect(percentiles([4])).toEqual({
    count: 1,
    p50: 4,
    p95: 4,
    p99: 4,
    max: 4,
  });
  for (const invalid of [[], [-1], [NaN], [Infinity]]) {
    expect(() => percentiles(invalid)).toThrow(/finite/);
  }
});

test("native performance evidence groups only named synthetic scenarios", () => {
  const samples: QueryTiming[] = [
    { scenario: "app", readyWaitMs: 7, domMs: 2, frameOpportunityMs: 10 },
    {
      scenario: "calculation",
      readyWaitMs: 0,
      domMs: 3,
      frameOpportunityMs: 11,
    },
    {
      scenario: "conversion",
      readyWaitMs: 0,
      domMs: 4,
      frameOpportunityMs: 12,
    },
    { scenario: "emoji", readyWaitMs: 0, domMs: 5, frameOpportunityMs: 13 },
  ];
  const summary = summarizeTimings(samples);
  expect(Object.keys(summary)).toEqual([
    "app",
    "calculation",
    "conversion",
    "emoji",
  ]);
  expect(summary.app.readyWaitMs.p95).toBe(7);
  expect(summary.app.domMs.p95).toBe(2);
  expect(summary.app.frameOpportunityMs.p95).toBe(10);
  expect(() => summarizeTimings(samples.slice(1))).toThrow(/nonempty/);
  expect(() =>
    summarizeTimings([
      { scenario: "app", readyWaitMs: 0, domMs: 12, frameOpportunityMs: 10 },
      ...samples.slice(1),
    ]),
  ).toThrow(/precede/);
});

test("serialized native timing waits for refresh readiness and dispatches exactly once", async ({
  page,
}) => {
  await page.setContent(
    '<input role="combobox"><div role="listbox" aria-busy="true"><span class="result-title"></span></div>',
  );
  const result = await page.evaluate(async (source) => {
    // Run the same serialized function used by WebDriver, not a separate mock.
    const measure = new Function(
      `return (${source})`,
    )() as typeof measureQueryTiming;
    const input = document.querySelector("input")!;
    const list = document.querySelector("[role=listbox]")!;
    let dispatched = 0;
    input.addEventListener("input", () => {
      dispatched += 1;
      list.setAttribute("aria-busy", "true");
      queueMicrotask(() => {
        document.querySelector(".result-title")!.textContent = input.value;
        list.setAttribute("aria-busy", "false");
      });
    });
    const pending = measure("synthetic result");
    await new Promise((resolve) => setTimeout(resolve, 25));
    const beforeReady = dispatched;
    list.setAttribute("aria-busy", "false");
    return { beforeReady, sample: await pending, dispatched };
  }, nativeTimingSource);
  expect(result.beforeReady).toBe(0);
  expect(result.dispatched).toBe(1);
  expect(result.sample).not.toHaveProperty("error");
  if ("error" in result.sample) throw new Error(result.sample.error);
  expect(result.sample.title).toBe("synthetic result");
  expect(result.sample.readyWaitMs).toBeGreaterThan(0);
  expect(result.sample.frameOpportunityMs).toBeGreaterThanOrEqual(
    result.sample.domMs,
  );
});

for (const busy of [true, false]) {
  test(`native timing bounds ${busy ? "readiness" : "response"} waits without retrying`, async ({
    page,
  }) => {
    await page.setContent(
      `<input role="combobox"><div role="listbox" aria-busy="${busy}"></div>`,
    );
    const result = await page.evaluate(
      async ({ source }) => {
        const measure = new Function(
          `return (${source})`,
        )() as typeof measureQueryTiming;
        const list = document.querySelector("[role=listbox]")!;
        let dispatched = 0;
        document.querySelector("input")!.addEventListener("input", () => {
          dispatched += 1;
          list.setAttribute("aria-busy", "true");
        });
        const sample = await measure("synthetic result", 20);
        list.setAttribute("aria-busy", "false");
        await new Promise((resolve) => setTimeout(resolve, 20));
        return { sample, dispatched };
      },
      { source: nativeTimingSource },
    );
    expect(result.dispatched).toBe(busy ? 0 : 1);
    expect(result.sample).toHaveProperty(
      "error",
      expect.stringContaining(busy ? "readiness" : "response/frame"),
    );
  });
}
