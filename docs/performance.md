# Performance checks

Measured on 17 September 2026 with an Apple M2, macOS 27.0, and the release build. These are local samples, not performance guarantees for all computers.

## File search

The ignored Rust test builds an index from 50,000 synthetic file entries. It runs four queries 25 times and returns up to 30 results per query.

| Measurement                  | Result  |
| ---------------------------- | ------- |
| Normalize and sort the index | 53 ms   |
| Median search                | 4.10 ms |
| 95th percentile search       | 4.29 ms |
| Slowest search               | 4.33 ms |

This measures `SearchManager` in Files mode. It excludes filesystem traversal, IPC, and rendering. Run it with:

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml --locked profile_search_50k_files -- --ignored --nocapture
```

The resident application also detected file creation, renaming, and deletion in Downloads. Its index logs changed without a manual refresh. The observed updates took 0.52 to 1.65 seconds, including event delivery, the 300 ms quiet period, and a full scan of about 11,000 files. Compilation was active during this check. Windows and Linux native CI checks verify the displayed results as well as changes to subfolders and search roots.

## Idle process

Eight samples, one second apart, showed 65.0 to 65.8 MiB RSS for the Rust host process. Reported CPU use was 0.0% in seven samples and 0.6% in one sample. Its accumulated CPU time increased by 0.01 seconds. The application had loaded about 11,000 files and the currency cache. The window was hidden.

These numbers exclude WebKit processes. They do not describe total application memory. The existing clipboard counter check still runs once per second on macOS and Windows. The file worker waits for OS events. Currency refresh runs only on demand or when the user opens the launcher with missing or old rates.

## Window and build size

The release executable was 12,106,544 bytes. The previous Phase 7 executable was 11,228,016 bytes. These sizes exclude the app bundle metadata and OS webview libraries.

Set `RUST_LOG=tinydash_lib=debug` to record the duration of the Rust window-show function. This duration excludes the time needed for the webview to paint. Use a desktop recording to measure visible invocation time. The current automated timing test does not measure cold startup or visible invocation time.
