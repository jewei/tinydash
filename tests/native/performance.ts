export type TimingScenario = "app" | "calculation" | "conversion" | "emoji";

export interface QueryTiming {
  scenario: TimingScenario;
  domMs: number;
  frameOpportunityMs: number;
}

export function percentiles(samples: readonly number[]) {
  if (
    samples.length === 0 ||
    samples.some((sample) => !Number.isFinite(sample) || sample < 0)
  ) {
    throw new Error(
      "Timing samples must be nonempty, finite, and nonnegative.",
    );
  }
  const sorted = [...samples].sort((left, right) => left - right);
  const percentile = (fraction: number) =>
    sorted[Math.ceil(sorted.length * fraction) - 1];
  return {
    count: sorted.length,
    p50: percentile(0.5),
    p95: percentile(0.95),
    p99: percentile(0.99),
    max: sorted[sorted.length - 1],
  };
}

export function summarizeTimings(samples: readonly QueryTiming[]) {
  const scenarios: TimingScenario[] = [
    "app",
    "calculation",
    "conversion",
    "emoji",
  ];
  return Object.fromEntries(
    scenarios.map((scenario) => {
      const selected = samples.filter((sample) => sample.scenario === scenario);
      if (selected.some((sample) => sample.frameOpportunityMs < sample.domMs)) {
        throw new Error("A frame opportunity cannot precede the DOM update.");
      }
      return [
        scenario,
        {
          domMs: percentiles(selected.map((sample) => sample.domMs)),
          frameOpportunityMs: percentiles(
            selected.map((sample) => sample.frameOpportunityMs),
          ),
        },
      ];
    }),
  );
}
