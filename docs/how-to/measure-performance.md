# Measure performance

Keep **targets**, **measurements**, and **coverage limits** separate. A passing correctness test or a successful release build is not a latency measurement. A Rust-provider benchmark is not launcher input-to-paint latency or native application memory use.

## Choose a repeatable comparison

Use the existing [synthetic file-provider harness](../../scripts/perf/file-index/README.md). It compiles the checkout's actual provider and ranking code; it does not scan personal folders or capture the clipboard. Follow its baseline/candidate commands for two identified commits, with the same Rust version, compiler options, corpus sizes, machine, and power settings.

For a single-checkout timing smoke run from the repository root:

```sh
mkdir -p .local/performance
CARGO_TARGET_DIR="$PWD/.local/performance/target" \
  cargo build --manifest-path scripts/perf/file-index/Cargo.toml --locked --release
.local/performance/target/release/tinydash-file-provider-profile ascii 100 50000 \
  > .local/performance/ascii.json
.local/performance/target/release/tinydash-file-provider-profile mixed 100 50000 \
  > .local/performance/mixed.json
```

Windows executable names have `.exe`. For a comparison, use at least five alternating baseline/candidate blocks as described in the harness README, rather than choosing the fastest run. Keep allocation-tracking and timing executables separate. Check ordered result equivalence before attributing timing changes to an optimization.

Record source commit and dirty state, compiler version, OS/architecture, hardware class, corpus count, warmup/sample counts, commands, and measurement boundaries. Keep raw records in ignored `.local/` or `test-results/` folders. Report per-scenario sample count and p50/p95/max durations, using the same percentile convention across runs; do not average percentiles or combine unrelated scenarios into one percentile. Record indexing separately from warm query time.

## Targets are not measured results

Before collecting results, write down the decision threshold, scenario, percentile, and required platform in the task's private acceptance record. For example, a team can choose a warm-query budget and an allowed candidate-versus-baseline regression, but those numbers are engineering targets until measured on the stated setup. This procedure introduces no universal performance guarantee or automatic wall-clock CI gate. Hosted-runner timings are noisy and are not interchangeable with a controlled desktop baseline.

The harness times provider search and result construction after warmup. It excludes IPC, Solid rendering, WebView startup, filesystem scanning, SQLite, real clipboard access, and window/focus behavior. Its allocation mode counts requested Rust heap bytes, not process RSS or physical footprint. A native result-to-paint or startup claim needs separately identified installed-build measurements on the required desktop platform. Keep those separate from provider measurements and from mocked browser tests.

## Native sample boundaries

The installed-build smoke test records 25 samples per synthetic scenario after a warmup cycle. Its version-2 `performance.json` reports `readyWaitMs` separately: a background refresh may already be pending before the synthetic input is dispatched. Readiness and dispatch are coordinated inside one webview script, not across separate WebDriver calls. Every dispatched sample is retained; there is no retry or fastest-sample selection. A watchdog still fails stalled readiness, response, or frame callbacks.

`domMs` starts at input dispatch; `readyWaitMs + domMs` includes preparation waiting. Double-rAF timing reports a rendering opportunity, not screen paint. Reopen timing includes WebDriver polling and now requires native window visibility as well as DOM readiness. Historical version-1 records used the earlier readiness procedure; do not treat cross-procedure differences as performance improvements.

## Privacy and evidence

Performance instrumentation and published summaries should emit only counts and durations (including byte counts), plus non-personal build/environment identity. Never log user query strings, indexed paths, app names, clipboard contents, passwords, or URLs. Use fixed synthetic scenarios in an isolated profile. The existing harness's raw records contain synthetic result IDs for equivalence checks; retain them privately and publish only numeric summaries. Do not point it or a native profiling run at personal data.

No measurements are asserted by this procedure. When publishing a measurement, identify the exact measured source and machine, include the repeatable command and sample counts, state what was excluded, and retain the underlying evidence. See [verification](verify.md) for build identity and platform-proof rules. [Recorded performance samples](../reference/performance-samples.md) retain separately identified backend and native baselines with their exclusions.
