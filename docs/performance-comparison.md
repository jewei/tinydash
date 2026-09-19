# Launcher performance comparison

Measured on 19 September 2026 on an Apple M2 Mac with 16 GiB of memory and macOS 27.0.
All three apps used arm64 release builds.

TinyDash had the shortest median window-open time in this test.
Its hidden-window memory footprint was 3.3 times Bopop's and 3.2 times Tinycast's.
Bopop had the smallest executable and app bundle.
Application search times were close enough that background work and the measurement method matter.

| Measurement                      |  TinyDash |     Bopop |  Tinycast |
| -------------------------------- | --------: | --------: | --------: |
| Hidden window, memory footprint  | 163.1 MiB |  49.3 MiB |  51.0 MiB |
| Open window, memory footprint    | 174.7 MiB |  49.8 MiB |  51.7 MiB |
| Hidden window, RSS               | 218.9 MiB | 130.5 MiB | 107.0 MiB |
| Hidden window, CPU use           |    0.003% |    0.001% |    0.001% |
| Hidden window, interrupt wakeups |    14.7/s |     2.1/s |     0.1/s |
| Main executable                  | 13.26 MiB |  2.99 MiB | 11.28 MiB |
| App bundle, regular files        | 13.30 MiB |  6.47 MiB | 13.28 MiB |

Memory values include each app and the helper processes that macOS attributed to it.
This includes TinyDash's WebKit processes. It also includes attributed system helpers for the native apps.
Memory values are the median of three run medians. One MiB is 1,048,576 bytes.
TinyDash run medians ranged from 157.4 to 166.6 MiB.
Bopop run medians ranged from 48.9 to 52.9 MiB. Tinycast run medians ranged from 51.0 to 51.5 MiB.

CPU use is the median of three 30-second intervals. A value of 100% means one CPU core.
These small values do not establish a useful CPU ranking. Wakeup counts do not measure battery use.
TinyDash wakeups varied from 0.7 to 15.7 per second across runs. A longer test is needed to establish a stable rate.
RSS and memory footprint are different macOS measures. The footprint can include compressed memory.
The bundle sizes exclude shared macOS frameworks. Bopop's bundle includes Sparkle.

The next table gives the median and the 95th percentile, in that order.
The 95th percentile is the value at or below which 95% of samples finished.
Each app has 60 application-search samples, 60 arithmetic samples, and 30 window-open samples.

| Operation             |       TinyDash |          Bopop |        Tinycast |
| --------------------- | -------------: | -------------: | --------------: |
| Application search    | 31.1 / 50.6 ms | 42.6 / 58.0 ms |  37.2 / 99.8 ms |
| Arithmetic            | 21.7 / 47.2 ms | 44.9 / 65.5 ms |  36.0 / 68.3 ms |
| Open the window again | 35.3 / 52.7 ms | 67.1 / 86.2 ms | 91.8 / 107.6 ms |

The queries were `saf`, `term`, `12 * 8`, and `123 + 456`.
The required results were `Safari`, `Terminal`, `96`, and `579`.
All 360 measured query checks and all 90 measured window-open checks passed.
Each search started after a no-match query. The test checked that the expected result was absent before it sent new input.

The query timer starts when the test posts a complete query as a keyboard event.
It stops when the expected result appears in the accessibility tree, outside the input field.
This includes input handling, search, UI updates, accessibility delivery, and polling overhead.
It does not measure the first painted screen pixel.
The window-open timer starts at the global shortcut. It stops when an on-screen window and an accessible input field exist.
It does not test keyboard focus or the end of the window animation.
The polling loop sleeps for 2 ms between checks. Tree traversal adds further time.

The apps used separate test settings with empty histories. Clipboard capture was off in all three apps.
TinyDash had no file-search roots, and file watching and currency updates were off.
Bopop currency updates and automatic update checks were off.
Tinycast's optional features kept their default off settings. Its first-run window was marked as complete.
The tests used recurring queries, so query caches could be warm. No test launched a search result or changed the system clipboard.
These are minimal launcher profiles. They do not describe full daily use with file indexes, clipboard histories, or extensions.

The test used three fresh processes per app. The app order changed between rounds.
Each process had 10 seconds to start before UI checks began.
Each run discarded two query cycles and two window-open cycles as warmup.
After the UI checks, the test hid the window and waited 30 seconds.
It then took 31 memory samples across 30 seconds and checked that the window remained hidden.
The test read CPU time and memory with `proc_pid_rusage` and used macOS process responsibility to find helpers.
The process groups stayed constant during each idle interval. No attributed helper remained two seconds after each app exited.
WindowServer and shared system work outside those process groups are excluded.

This was a running desktop session on AC power. Other apps and some unrelated Rust builds remained active.
The Mac had about 6 GiB of swap in use at the start. No thermal warning was reported.
These conditions limit conclusions about small timing differences. The results are local observations, not performance guarantees.
Cold startup, frame rate, battery use, large clipboard histories, and other operating systems were not measured.

TinyDash's process breakdown explains most of the memory difference.
The following values use the same hidden-window samples and the median of the three run medians.
Component medians need not add to the total median.

| TinyDash process group   | Memory footprint |
| ------------------------ | ---------------: |
| Rust host                |         36.5 MiB |
| WebKit content           |         85.6 MiB |
| WebKit GPU               |         14.9 MiB |
| WebKit network           |          7.0 MiB |
| Other attributed helpers |         19.1 MiB |

For a TinyDash memory investigation, start with the WebKit content process.
The Rust host alone would give an incomplete comparison with the native apps.

Live file search could not be compared. `mdutil -s /` reported `Indexing disabled`.
Bopop and Tinycast use Spotlight. TinyDash builds its own file index.
Bopop waits 250 ms after file-search input. Tinycast waits 120 ms.
TinyDash has no frontend debounce timer. These code settings are not measured end-to-end file-search times.
The relevant source is `QueryEngine.swift` in Bopop, `FileSearchSession.swift` in Tinycast, and `src/App.tsx` in TinyDash.

TinyDash's existing release test ran separately on 50,000 synthetic paths in five fresh test processes.
Each process ran four queries 25 times. The median of the five reported p50 values was 4.09 ms.
The per-run p50 range was 4.07 to 4.21 ms.
The median of the five reported p95 values was 4.26 ms.
This test excludes disk traversal, IPC, and UI work. It does not rank TinyDash against Spotlight.
Index normalization and sorting took a median of 62 ms.
The bundled test uses zero-based samples 50 and 95 for its reported percentiles. The raw output retains that definition.

```sh
cargo test --release --manifest-path src-tauri/Cargo.toml --locked \
  profile_search_50k_files -- --ignored --nocapture
```

| App      | Measured commit                            | Build                                 |
| -------- | ------------------------------------------ | ------------------------------------- |
| TinyDash | `bccaa245284cc22a7bc0c96d3083dace3a1950ca` | Tauri release, Rust 1.98.1, Bun 1.4.2 |
| Bopop    | `480d157255f3e65e6e45bf765d2c8804edfb7bfd` | Swift release, arm64                  |
| Tinycast | `0d7537bd1dc42ed60a5018090c1126983bbb4355` | Xcode release, arm64                  |

Swift was 6.4 and Xcode was 27.0, build 27A266a.
TinyDash used a saved copy of its committed source. Active working-tree edits were excluded, as requested.
All apps had separate bundle identifiers and test data folders.
Bopop needed one change in its temporary source copy: its dev data-folder name became `Bopop Perf 20260919`.
Its normal dev folder already contained user data. This change did not alter search or UI code.
The repository source files were not changed for this comparison.

The [raw samples and summary](benchmarks/2026-09-19/results/summary.json) contain per-run ranges and exact values.
The [method record](benchmarks/2026-09-19/results/method.json) contains build commands, settings, and timing rules.
Executable SHA-256 hashes are in the [size record](benchmarks/2026-09-19/results/sizes.json).
The [file-search record](benchmarks/2026-09-19/results/file-search.json) contains all five outputs.
The [test instructions](benchmarks/2026-09-19/README.md) describe how to use the saved measurement code.
