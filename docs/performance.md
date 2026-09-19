# Performance checks

Measured on 17 September 2026 with an Apple M2, macOS 27.0, and the release build. These are local samples, not performance guarantees for all computers.

## File search

The ignored Rust test builds an index from 50,000 synthetic file entries. It runs four queries 25 times and returns up to 30 results per query. This sample uses Rust 1.98.1 after the window and selection fixes.

| Measurement                  | Result  |
| ---------------------------- | ------- |
| Normalize and sort the index | 65 ms   |
| Median search                | 4.09 ms |
| 95th percentile search       | 4.35 ms |
| Slowest search               | 4.97 ms |

This measures `SearchManager` in Files mode. It excludes filesystem traversal, IPC, and rendering. Run it with:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml --locked profile_search_50k_files -- --ignored --nocapture
```

The resident application also detected file creation, renaming, and deletion in Downloads. Its index logs changed without a manual refresh. The observed updates took 0.52 to 1.65 seconds, including event delivery, the 300 ms quiet period, and a full scan of about 11,000 files. Compilation was active during this check. Windows and Linux native CI checks verify the displayed results as well as changes to subfolders and search roots.

## Idle process

The baseline before the dependency update used eight samples, one second apart. They showed 65.0 to 65.8 MiB RSS for the Rust host process. Reported CPU use was 0.0% in seven samples and 0.6% in one sample. Its accumulated CPU time increased by 0.01 seconds. The application had loaded about 11,000 files and the currency cache. The window was hidden.

These numbers exclude WebKit processes. They do not describe total application memory. The existing clipboard counter check still runs once per second on macOS and Windows. The file worker waits for OS events. Currency refresh runs only on demand or when the user opens the launcher with missing or old rates.

The frontend now stops searches while its native window is hidden. A browser regression test sends application, file, currency, usage, and clipboard events after hiding. Before the change, those five events caused five search requests. After the change, they cause none. Reopening requests current results. A search that was already running can finish, but its reply is discarded. Clipboard capture and file indexing continue in Rust.

## Native query and startup checks

Windows and Linux native CI jobs save `performance.json` in their diagnostic artifacts. Each job measures 20 queries against its test profile. The samples cover an installed application, arithmetic, unit conversion, and emoji. A timer inside the webview starts before the input event and stops when the result list is ready. This includes frontend dispatch, Tauri IPC, Rust search, and the SolidJS DOM update. It excludes WebDriver transport and screen painting.

The report also records the initial startup check and subsequent reopen checks. These include WebDriver attachment or transport and 100 ms readiness polling. Reopen uses the single-instance launch path. Some reopen checks start with an already visible window. Neither measurement is the physical global-shortcut latency. The tests check the returned results, but apply no timing threshold to shared CI runners.

Native tests also change files while the window is hidden. They verify that Rust updates its index, that hidden UI results stay unchanged, and that reopening displays current data.

## Window and build size

Before the dependency update, the release executable was 12,106,544 bytes. The previous Phase 7 executable was 11,228,016 bytes. These sizes exclude the app bundle metadata and OS webview libraries.

Set `RUST_LOG=tinydash_lib=debug` to record the duration of the Rust window-show function. This duration excludes the time needed for the webview to paint. Use a desktop recording to measure visible invocation time. The automated startup check includes driver overhead and does not measure pure cold-start time or visible invocation time.

## Bounded release benchmark results

Measured on 19 September 2026 on arm64 macOS 27.0 with Rust 1.98.1. No other Cargo process was running when this check started. The existing ignored test built an index from 50,000 synthetic file entries. It ran four queries 25 times each, for 100 searches.

| Measurement                        |    Result |
| ---------------------------------- | --------: |
| Synthetic index build              |    130 ms |
| Search p50                         |  9,589 µs |
| Search p95                         | 12,932 µs |
| Search maximum                     | 18,156 µs |
| Full cold command                  |   1:57.22 |
| Release compilation inside command |      1:55 |
| Benchmark test body                |    0.94 s |

Command:

```sh
time cargo test --release --manifest-path src-tauri/Cargo.toml --locked profile_search_50k_files -- --ignored --nocapture
```

This is one local sample. It does not measure native OS shortcut or window latency, total webview memory, filesystem traversal, IPC, rendering, or clipboard capture cost. Native OS timing and total webview memory remain unverified.
