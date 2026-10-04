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

## Retained native baseline

[Checks run 36375876881](https://github.com/jewei/tinydash/actions/runs/36375876881) tested merge source `5fde5b949d630f294c6af9ea315730c644f8845b` on 28 September 2026. Its tree equals PR head `245c84e060881258832be2dc7b9669784a764323`; it predates the search-budget, storage and ACL follow-ups. Both installed release-package checks passed and recorded complete cleanup. [PR #10](https://github.com/jewei/tinydash/pull/10) retains the sanitized verification summary and package hashes even after CI artifacts expire.

Each scenario had one warm-up and 25 measured observations. Values below are milliseconds, rounded to one decimal. These are input-event-to-DOM and rendering-opportunity observations, **not actual paint measurements** or a performance guarantee.

| Scenario    | Windows DOM p95 | Windows frame opportunity p95 | Linux DOM p95 | Linux frame opportunity p95 |
| ----------- | --------------- | ----------------------------- | ------------- | --------------------------- |
| Application | 5.3             | 28.4                          | 8.0           | 22.0                        |
| Calculation | 5.0             | 27.9                          | 9.0           | 22.0                        |
| Conversion  | 5.4             | 28.0                          | 8.0           | 22.0                        |
| Emoji       | 5.1             | 27.9                          | 8.0           | 15.0                        |

Both virtualized CI runners reported AMD EPYC 7763 processors, four logical CPUs and approximately 16 GiB RAM. Windows reported build `10.0.26100`, x64 and WebView2 Edge 153. Linux X11 reported kernel `6.17.0-1022-azure`, x64 and WebKitGTK user-agent AppleWebKit `605.1.15`. User-agent versions identify the reported environment, not an independently audited webview package version.

Package SHA-256:

- `TinyDash_0.1.3_x64-setup.exe`: `d8f8d4ad9351965c4234ea6695339eb2e3899257fa81078dbb5e29ccc05f275c`
- `TinyDash_0.1.3_amd64.deb`: `ea52ac3d7fbe1ec1118346a3aeba87ad2ca9d88a34128df6fc8c7998bef0d3c6`

Small-sample p99 is the maximum here; the native artifact retains all three percentiles. CI hardware contention is uncontrolled. Do not compare these values as proof of an improvement over another machine or source.

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
