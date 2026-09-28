import { expect, test } from "@playwright/test";
import {
  percentiles,
  summarizeTimings,
  type QueryTiming,
} from "./native/performance";

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
    { scenario: "app", domMs: 2, frameOpportunityMs: 10 },
    { scenario: "calculation", domMs: 3, frameOpportunityMs: 11 },
    { scenario: "conversion", domMs: 4, frameOpportunityMs: 12 },
    { scenario: "emoji", domMs: 5, frameOpportunityMs: 13 },
  ];
  const summary = summarizeTimings(samples);
  expect(Object.keys(summary)).toEqual([
    "app",
    "calculation",
    "conversion",
    "emoji",
  ]);
  expect(summary.app.domMs.p95).toBe(2);
  expect(summary.app.frameOpportunityMs.p95).toBe(10);
  expect(() => summarizeTimings(samples.slice(1))).toThrow(/nonempty/);
  expect(() =>
    summarizeTimings([
      { scenario: "app", domMs: 12, frameOpportunityMs: 10 },
      ...samples.slice(1),
    ]),
  ).toThrow(/precede/);
});
