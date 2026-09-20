# File index and clipboard allocation

The file index now borrows the existing ASCII filename and path bytes for matching. Unicode text and paths that need separator conversion retain a normalized cache. Clipboard lookups borrow history entries. Copy operations allocate their output after size validation.

The patch preserves result IDs, paths, scores, order, action validation, and clipboard text. It changes five Rust files. It does not include the icon or window experiments in draft PR #3.

## Verified provider results

The isolated benchmark compiles the production providers. The results below were reproduced on an Apple M2 with macOS 27.0 and Rust 1.98.1. The baseline is `06974f411483f5d254d80b617d9fcbe4dc114a71`.

| Workload                                                          | Baseline requested heap | Candidate requested heap | Reduction |
| ----------------------------------------------------------------- | ----------------------: | -----------------------: | --------: |
| Retained index, 50,000 ASCII paths                                |        16,540,000 bytes |         13,645,000 bytes |     17.5% |
| Retained index, 50,000 paths with 10% Unicode names               |        18,203,500 bytes |         15,598,000 bytes |     14.3% |
| Peak extra heap, valid selection of 100 entries                   |            39,267 bytes |             17,123 bytes |     56.4% |
| Peak extra heap, rejected selection of 100 entries of 16 KiB each |         1,645,668 bytes |              1,124 bytes |     99.9% |

These measurements exclude allocator overhead, filesystem scanning, IPC, native clipboard access, and the webview. They do not establish total application memory savings. Windows paths that need separator conversion retain a path cache and save less memory.

All measured ordered IDs and scores matched. The separate 128-case comparison also matched. Both clipboard cases returned the same text or error. The full macOS Rust suite passed with 161 tests and three ignored tests. All 132 UI tests passed. Clippy, formatting, and the frontend build passed.

The first local timing run had large variation and coincided with heavy Simulator activity observed immediately after the run. Its timing results are inconclusive. All observations remain in the review evidence.

## Follow-up method

The native comparison uses two isolated release builds with the same diagnostic overlay and a separate app identifier. Product files differ only by this patch. The overlay controls the test app and supplies synthetic clipboard input. It is absent from the product build.

Before acceptance measurements, complete builds and local tests. Use five paired fresh launches for each profile. Randomize the order within each pair with a fixed seed and preserve every run. The controlled profile has no indexed files or clipboard history. The daily-use profile has 50,000 fixture files with 10% Unicode names, 100 clipboard records, file watching, and pins. These are repeatable test profiles, not observations of a person's normal use.

Record physical footprint for the host and every attributed helper. Keep RSS separate. Record four states: hidden after startup, hidden after first use, hidden after Settings, and hidden after sustained use. Use a ten-second settling period and five samples one second apart for each state. Record visible memory separately after first use.

Measure search through native IPC and through input events in the real webview. Measure warm reopen readiness separately. Record text results and errors. These timings exclude the time when pixels reach the display. Use launch-level p50 and p95 values. Compare paired launch differences with a 95% bootstrap interval. The maximum permitted latency increase is the greater of 2 ms or 5% for p50, and 5 ms or 10% for p95. The fixed launch count limits precision; an interval above a margin is inconclusive or a failure, not a pass.

The acceptance claim for this patch is less provider allocation with unchanged behavior and acceptable latency. A 10 MiB whole-application saving from the separate icon experiment is not a target for this change. Report native memory observations without adding them to the isolated allocation savings.

Native functional checks cover ASCII and Unicode file results, file-watch changes, clipboard preview, single copy, ordered and duplicate selection IDs, missing IDs, and oversized selections. Preserve and restore the OS clipboard during the copy check. Check clipboard contents without recording unrelated user data.

## Follow-up status

The patch is under native validation. Windows and Linux CI results and the final native report will be recorded here before the merge decision.

Use [the provider benchmark](../scripts/perf/file-index/README.md) to repeat the isolated measurements. The repository's [performance plan](performance-resume.md) describes the broader acceptance requirements.
