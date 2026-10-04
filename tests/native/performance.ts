export type TimingScenario = "app" | "calculation" | "conversion" | "emoji";

export interface QueryTiming {
  scenario: TimingScenario;
  readyWaitMs: number;
  domMs: number;
  frameOpportunityMs: number;
}

export interface QueryMeasurement {
  readyWaitMs: number;
  domMs: number;
  frameOpportunityMs: number;
  title: string;
}

// Self-contained: the native driver serializes this function into the webview.
// Wait before dispatch, not by discarding/retrying a measured input. Background
// refreshes can make the list busy between separate WebDriver round trips.
export function measureQueryTiming(
  query: string,
  timeoutMs = 4_000,
): Promise<QueryMeasurement | { error: string }> {
  return new Promise((resolve) => {
    const input = document.querySelector<HTMLInputElement>(
      "input[role=combobox]",
    );
    const list = document.querySelector("[role=listbox]");
    if (!input || !list) {
      resolve({ error: "Search elements were missing for the timing sample" });
      return;
    }
    const prepared = performance.now();
    let started: number | undefined;
    let completed = false;
    let frame = 0;
    const finish = (result: QueryMeasurement | { error: string }) => {
      if (completed) return;
      completed = true;
      observer.disconnect();
      cancelAnimationFrame(frame);
      clearTimeout(timer);
      resolve(result);
    };
    const begin = () => {
      if (started !== undefined || list.getAttribute("aria-busy") !== "false")
        return;
      started = performance.now();
      input.value = query;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    };
    const observer = new MutationObserver(() => {
      if (started === undefined) {
        begin();
        return;
      }
      if (list.getAttribute("aria-busy") !== "false") return;
      observer.disconnect();
      const readyWaitMs = started - prepared;
      const domMs = performance.now() - started;
      const title = list.querySelector(".result-title")?.textContent ?? "";
      frame = requestAnimationFrame(() => {
        frame = requestAnimationFrame(() =>
          finish({
            readyWaitMs,
            domMs,
            frameOpportunityMs: performance.now() - started!,
            title,
          }),
        );
      });
    });
    const timer = setTimeout(
      () =>
        finish({
          error: `Search timing timed out during ${started === undefined ? "readiness" : "response/frame"} (document visibility: ${document.visibilityState})`,
        }),
      timeoutMs,
    );
    observer.observe(list, {
      attributes: true,
      attributeFilter: ["aria-busy"],
    });
    begin();
  });
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
          readyWaitMs: percentiles(
            selected.map((sample) => sample.readyWaitMs),
          ),
          domMs: percentiles(selected.map((sample) => sample.domMs)),
          frameOpportunityMs: percentiles(
            selected.map((sample) => sample.frameOpportunityMs),
          ),
        },
      ];
    }),
  );
}
