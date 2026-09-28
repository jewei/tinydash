# Performance samples

Measurements describe the named test, not every search or desktop. Product targets are not measurements. In particular, a proposed warm ordinary-search p95 below 50 ms from input to paint has **not** been established by the samples below.

## Backend baseline

Measured source: `2ec68a45a52160db7963a8699905c585c096d381`, clean worktree. Configuration: Rust 1.98.1, Cargo release profile from that source, Apple M2 MacBook Air, eight CPU cores, 16 GiB RAM, macOS 27.0 (26A428), arm64. This test does not use a webview.

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml --locked profile_search_50k_files -- --ignored --nocapture
```

The existing test builds 50,000 synthetic paths in 100 synthetic project folders and repeats four name/path/no-match queries 25 times. One run on 28 September 2026 reported:

| Stage              | Result   |
| ------------------ | -------- |
| Index construction | 8 ms     |
| Search p50         | 4.550 ms |
| Search p95         | 4.754 ms |
| Search maximum     | 5.002 ms |

The test uses its existing sorted sample indices (50, 95, 99), not the nearest-rank convention used by the native summary below. These results exclude queueing, IPC, serialization, frontend updates and paint. They do not measure allocations, peak memory, idle CPU, clipboard histories, calculation pins or filesystem churn. Repeat the command on the source being evaluated; do not attribute this baseline to later changes.

## Native evidence

The [native verification procedure](../how-to/verify.md#drive-the-real-desktop-app) records the executable/package hash, build source and test source. Its `native/performance.json` contains only synthetic scenario names, durations, counts and non-identifying platform configuration. Keep it with the wrapper's build and result records; a detached timing file does not establish build identity.

The suite warms up once, then takes 25 samples each for application search, calculation, unit conversion and emoji. It measures inside the webview, excluding WebDriver request transport:

1. Input event to settled result DOM, including queueing, IPC and backend search.
2. Input event to a double-`requestAnimationFrame` rendering opportunity after the DOM update.

A rendering opportunity is **not proof of screen paint**, compositor presentation or physical input latency. The report labels both separately. Per-scenario p50/p95/p99 use nearest rank; with 25 observations, p99 is the maximum, not a reliable tail estimate. CI runner timings are descriptive and have no performance pass/fail threshold. Startup and reopen observations include readiness polling and WebDriver overhead.

Run the aggregation regression separately:

```sh
bun run verify:browser tests/performance-evidence.spec.ts
```

That test checks arithmetic and evidence validation, not native performance. The native suite needs its documented Windows or Linux X11 session. It does not establish macOS or Wayland latency. Never substitute browser tests with mocked IPC for native results.

For larger investigations, separately record search-lock wait, provider and pin work, serialized bytes, allocation counts, index-replacement peak memory and background CPU. Use synthetic datasets at configured limits and retain source/configuration alongside percentiles. Do not log queries, clipboard contents, generated secrets or personal paths.
