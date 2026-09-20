# File index and clipboard allocation

**Decision: keep [PR #5](https://github.com/jewei/tinydash/pull/5) as a draft. Do not merge yet.** Provider allocation fell, and behavior checks passed. However, three of 56 native latency comparisons are inconclusive against the limits set before the test. The native memory results do not support a general claim of lower total application memory.

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

The next provider run used five alternating blocks without concurrent builds or Simulator load. Median index construction time fell from 43.741 to 4.000 ms for ASCII paths and from 55.078 to 19.954 ms for the mixed paths. These are isolated constructor times; they exclude file scanning. Search times were close. For `doc`, ASCII p50 was 4.069 versus 4.131 ms, and mixed-path p50 was 6.200 versus 6.247 ms. This test does not establish a general search speed increase.

## Native method set before testing

The native comparison uses two isolated release builds with the same diagnostic overlay and a separate app identifier. Product files differ only by this patch. The overlay controls the test app and supplies synthetic clipboard input. It is absent from the product build.

Before acceptance measurements, complete builds and local tests. Use five paired fresh launches for each profile. Randomize the order within each pair with a fixed seed and preserve every run. The controlled profile has no indexed files or clipboard history. The daily-use profile has 50,000 fixture files with 10% Unicode names, 100 clipboard records, file watching, and pins. These are repeatable test profiles, not observations of a person's normal use.

Record physical footprint for the host and every attributed helper. Keep RSS separate. Record four states: hidden after startup, hidden after first use, hidden after Settings, and hidden after sustained use. Use a ten-second settling period and five samples one second apart for each state. Record visible memory separately after first use.

Measure search through native IPC and through input events in the real webview. Measure warm reopen readiness separately. Record text results and errors. These timings exclude the time when pixels reach the display. Use launch-level p50 and p95 values. Compare paired launch differences with a 95% bootstrap interval. The maximum permitted latency increase is the greater of 2 ms or 5% for p50, and 5 ms or 10% for p95. The fixed launch count limits precision; an interval above a margin is inconclusive or a failure, not a pass.

The acceptance claim for this patch is less provider allocation with unchanged behavior and acceptable latency. A 10 MiB whole-application saving from the separate icon experiment is not a target for this change. Report native memory observations without adding them to the isolated allocation savings.

Native functional checks cover ASCII and Unicode file results, file-watch changes, clipboard preview, single copy, ordered and duplicate selection IDs, missing IDs, and oversized selections. Preserve and restore the OS clipboard during the copy check. Check clipboard contents without recording unrelated user data.

## Native results

All 20 planned launches completed. The application catalog stayed the same. Ordered IPC results, IDs, titles, and scores matched across each pair. UI result titles also matched. Both builds passed the separate clipboard and file-watch checks. Rejected clipboard selections returned the expected error and left the clipboard text unchanged. The test does not count clipboard writes. The helper restored the saved clipboard in both checks. No attributed helper process remained after app exit.

For each launch, the analysis excludes two warmup rounds. It then uses 25 samples per IPC query, 15 per UI query, and 20 per warm reopen test. It checks all expected cases and sample counts before analysis. The summary uses 10,000 paired bootstrap samples with seed `20260920`.

Of 56 latency comparisons, 53 passed and three were inconclusive. None established an increase above the permitted limit. An inconclusive result still prevents acceptance: its interval does not rule out an increase above that limit.

| Inconclusive comparison                    | Baseline mean | Candidate mean | 95% interval for candidate minus baseline | Permitted increase |
| ------------------------------------------ | ------------: | -------------: | ----------------------------------------: | -----------------: |
| Controlled profile, warm reopen p50        |     33.305 ms |      33.095 ms |                       -7.983 to +7.106 ms |               2 ms |
| Daily profile, Apps `saf` UI p95           |        9.0 ms |        10.8 ms |                           -1.0 to +5.8 ms |               5 ms |
| Daily profile, Files `document-001` UI p95 |       12.6 ms |        15.6 ms |                           -0.4 to +7.6 ms |               5 ms |

Each mean is the mean of five launch-level percentiles, not a percentile pooled across launches. These are individual comparison intervals. Five pairs give limited precision. The report does not treat the inconclusive results as proof of a regression or repeat the test until it passes.

The table below reports host plus attributed-helper physical footprint in MiB. Hidden states use the mean of five launch medians; each median contains five samples. The visible state uses the mean of one separate sample per launch. RSS is recorded separately in the raw data.

| Profile    | State                       | Baseline | Candidate | 95% interval for candidate minus baseline |
| ---------- | --------------------------- | -------: | --------: | ----------------------------------------: |
| Controlled | Startup, hidden             |   116.43 |    123.35 |                          -15.94 to +29.52 |
| Controlled | After first use, hidden     |   144.42 |    146.93 |                            -5.80 to +9.99 |
| Controlled | After Settings, hidden      |   184.37 |    187.25 |                            -0.27 to +6.21 |
| Controlled | After sustained use, hidden |   178.37 |    182.97 |                            +2.14 to +7.01 |
| Controlled | After first use, visible    |   243.08 |    239.71 |                          -19.53 to +15.03 |
| Daily      | Startup, hidden             |   152.55 |    162.30 |                           -6.93 to +21.44 |
| Daily      | After first use, hidden     |   314.13 |    241.36 |                         -135.87 to -12.87 |
| Daily      | After Settings, hidden      |   317.58 |    246.97 |                          -136.92 to -9.88 |
| Daily      | After sustained use, hidden |   306.45 |    234.94 |                          -148.17 to +5.13 |
| Daily      | After first use, visible    |   414.19 |    401.26 |                          -74.93 to +53.02 |

Native memory varied widely. The candidate used more memory in the controlled profile after sustained use. Some daily-profile states showed lower memory, but these results do not establish a consistent total application saving. The isolated provider savings remain valid within their stated scope.

## Review and platform checks

The Standards review and Spec review found no product code defect. The evidence links requested by the Standards review are now included below. The Spec review found missing completeness checks in the analysis script; these checks were added before the script analyzed the completed runs.

The macOS tests listed above passed on product commit `7784a47861a8db68ab5cfded9c431452111f8fc9`. The [push CI run](https://github.com/jewei/tinydash/actions/runs/35506332647) passed all macOS, Windows, and Linux build checks, plus Windows and Linux native checks. The [PR CI run](https://github.com/jewei/tinydash/actions/runs/35506349578) passed after one Linux native job retry. Its first attempt timed out while looking for the rocket emoji after reopening the launcher. The same test passed in the concurrent push run and on retry without a code change. The cause is unconfirmed. The first failure log and artifacts remain in the evidence archive.

The live PR checks show validation of any later documentation and evidence commit. A green CI result alone does not resolve the native latency acceptance gap.

## Evidence and next step

The [evidence index](benchmarks/2026-09-20-file-index/README.md) links the raw runs, exact source and executable hashes, diagnostic source overlays, analysis scripts, and reproduction steps. It preserves the supplied Linux measurements and both local provider runs. The [provider summary](benchmarks/2026-09-20-file-index/provider-summary.json) and [native summary](benchmarks/2026-09-20-file-index/native-summary.json) contain the full numerical results.

Before merge, investigate the three uncertain latency cases and the controlled-profile memory increase. Set the sample count and acceptance method for a separate study before collecting new measurements. Preserve these results and the original limits. Do not claim that the current native study passed.

Use [the provider benchmark](../scripts/perf/file-index/README.md) to repeat the isolated measurements. The repository's [performance plan](performance-resume.md) describes the broader acceptance requirements.
