# TinyDash memory optimization results

This iteration did not demonstrate a reduction in total application memory. The working implementation removes unnecessary result copies and loads small icons on demand. It remains a review candidate: the final comparison did not pass the memory and latency acceptance criteria.

In the controlled profile, median hidden host memory fell from 33.4 to 27.5 MiB. Total hidden memory rose from 127.1 to 146.2 MiB. In the daily-use profile, total hidden memory rose from 186.1 to 276.3 MiB. Each total-memory change is smaller than the larger range within one build, so the memory verdict is inconclusive under the declared rule. All six latency verdicts are also inconclusive. These results do not support a claim of lower total memory or unchanged performance.

Settings window release is deferred. Two native tests crashed in WebKit during Settings closure, including a second close sequence that failed on cycle 942. The working code retains the original Settings window and contains no Tao dependency patch. The failed prototypes remain in the evidence. The final comparison and both diagnostics are complete.

The work follows the [optimization plan](memory-optimization-plan.md). The base commit is `568972bbc92df37f2bbfd2951e8ab478b061e205`. The baseline also includes the uncommitted files that were present before this work. This baseline differs from the build in the original 163.1 MiB screenshot.

| Change                  | Selection                                      | Evidence                                                                                       |
| ----------------------- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| P1, Settings release    | Deferred; retain the current window.           | Editing-state tests passed, but later native close tests crashed.                              |
| P2, result construction | Implemented; separate review patch.            | Fixed search results match; direct counters show fewer icon copies.                            |
| P3a, on-demand icons    | Implemented; release acceptance is unresolved. | Cache limits and functional tests pass. Total-memory and latency verdicts remain inconclusive. |
| P3b, binary icons       | Deferred.                                      | No demonstrated memory benefit over small data URLs.                                           |

P2 keeps ranking, usage bonuses, pins, stable ties, and actions. The fixed corpus has 100 applications, tied scores, aliases, Unicode, a hidden application, usage data, and more than 30 pins. Seven query cases match results recorded from the original implementation. The fixture omits icon bytes but compares all other serialized result fields.

| Synthetic payload check        |  Original |      P2 |            Difference |
| ------------------------------ | --------: | ------: | --------------------: |
| Broad search, icon copies      |        99 |      30 |       69 fewer copies |
| Broad search, copied bytes     | 3,244,032 | 983,040 | 2,260,992 fewer bytes |
| Settings catalog, icon copies  |       100 |       0 |      100 fewer copies |
| Settings catalog, copied bytes | 3,276,800 |       0 | 3,276,800 fewer bytes |

Each synthetic icon contains 32 KiB. These counts measure copy work per operation. They do not measure retained physical footprint. The counter is test-only and is absent from performance runs.

| Icon resource                      |     Limit | Behavior when full                                                                   |
| ---------------------------------- | --------: | ------------------------------------------------------------------------------------ |
| Stored icon data URLs              |     2 MiB | Evict the least recently used entries. Return an oversized image without caching it. |
| Cached entries, including misses   |       256 | Evict old entries.                                                                   |
| Active native loads                |         2 | Queue work within the queue limit.                                                   |
| Queued unique loads                |        64 | Cancel obsolete work. Keep the placeholder if capacity is still unavailable.         |
| Pending consumers                  |       128 | Return a capacity response.                                                          |
| Frontend entries for visible icons |        64 | Retain placeholders until the row mounts again.                                      |
| Requested pixel size               | 16 to 256 | Reject unsupported sizes.                                                            |

Requests for one image share work. A query change removes consumers that no longer need the image. A late response must match the current entry. Native extraction runs outside the search lock. Search completion does not await icon loading. At display scale 2, a 36-pixel row requests a 72-pixel image, and a 64-pixel preview requests a 128-pixel image.

Catalog generation, icon source revision, and requested size define the cache identity. Catalog refresh invalidates old images and requests. Unit tests check the byte, entry, active-job, queue, and consumer limits. They also check missing images, oversized images, cancellation, invalidation, and panic cleanup. Native tests check PNG dimensions, distinct application images, Finder custom icons, and custom icon replacement on a temporary directory.

The selected implementation returns small PNG data URLs through a separate icon command. Search responses contain stable icon keys. Icon commands accept only identifiers from the application catalog. Platforms without native icon support keep their existing fallback. Native desktop validation in this report is limited to macOS. The binary trial used Tauri binary responses and Blob URLs; those changes are absent from the selected build.

The Rust cache limit includes the stored base64 representation and data-URL prefix. It does not cap decoded WebKit images or macOS framework caches. The frontend keeps no persistent image cache. It drops each image reference when the last visible consumer leaves or the launcher hides.

The selected build passed 160 Rust tests and all 130 browser tests. Three environment-specific Rust tests are excluded from the standard suite; the native icon test was run separately and passed. Clippy, Rust formatting, type checks, and the production frontend build passed. The final native comparison is complete. The retained Settings window passed 1,000 open and close cycles with its draft preserved. Earlier checks of the same icon implementation passed delayed-image, keyboard, and warm-reopen tests. Those results do not establish reliability of the deferred Settings release.

The release benchmark uses an isolated app identifier and data directory. It does not replace the installed TinyDash app. Clipboard capture reads synthetic text through a diagnostic hook. It does not read or change the system clipboard.

Physical footprint is the primary metric. Each sample includes the host and every helper assigned to that app by macOS process responsibility. RSS is recorded separately. Each state has a 30-second settling period and 31 samples at one-second intervals. A background sampler records workload peaks. Short allocations can occur between samples, so these are sampled peaks.

The final comparison starts five fresh processes per build and profile. It alternates the original baseline and the selected icon build. The controlled profile disables file roots, clipboard capture, and currency updates. The daily profile uses 50,000 files, file watching, 100 clipboard entries, pins, and cached currency data. Individual results, executable hashes, process membership, source records, environment readings, and failed runs are retained.

After build cleanup, about 29 GB of disk space was available. Cleanup overlapped daily trial 2 of the earlier Settings release prototype. That prototype later crashed and is not the selected build. All 20 final comparison runs started after cleanup. Available storage ranged from 28.84 to 29.39 GB. Earlier records remain separate. Compiler activity stopped before native measurements.

All final runs used macOS 27.0, build 26A428, a 980 by 620 page at display scale 2, and the same catalog of 124 applications. Each run completed 20 Settings visits. Every daily-use run confirmed file-watch updates and synthetic clipboard capture. The OS reported no recorded thermal or performance warning.

After the final checks, the remaining test app bundles, generated file trees, isolated clipboard profile, and completed editor cache were removed. Free storage was then 30.4 GB. Source archives, review patches, raw records, logs, and executable hashes are retained in the [evidence directory](benchmarks/2026-09-19/optimization/README.md).

Both builds contain the same diagnostic control overlay. Memory-workload traces retain request metadata and result titles; response-size serialization is disabled. Native query and warm-reopen measurements disable response tracing. Their result-ready checks use accessibility data. A separate screen recording confirms visible content and working input. These checks do not measure the exact time at which pixels reach the display.

The minimal-page comparison uses the same release binary, startup services, app catalog, native icon work, and backend requests. It verifies the completed page URL and marker. The minimal page receives and discards complete responses. The measured difference describes page-related cost under that workload. It does not establish removable memory or a universal WebKit minimum.

| Page, hidden footprint in MiB | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Median |
| ----------------------------- | ----: | ----: | ----: | ----: | ----: | -----: |
| Real launcher                 | 122.0 | 129.0 | 134.4 | 166.9 | 136.2 |  134.4 |
| Minimal page                  |  84.3 |  89.5 |  88.4 |  88.7 | 114.6 |   88.7 |

All ten runs completed the same 108 backend requests. Arguments, result titles, and serialized response lengths match between the pages. This first diagnostic includes an extra response-length trace. Its old field named `bytes` measures UTF-16 code units. The final comparison disables that serialization, so these page-control values are separate from its baseline. See the [minimal-page records](benchmarks/2026-09-19/optimization/minimal-summary.json).

The first sustained-use attempt stopped before its first memory group. Its trace included the automatic query from opening the launcher and the first explicit test query. The test now waits for that opening refresh and records it separately. The failed record is retained; this required no product change.

P4 adds no further frontend change. No specific retained allocation was established as unnecessary in this iteration. Main-window release remains off. This implementation does not change the UI toolkit or establish a 50 MiB product target.

The full icon trial used five fresh processes per format. Each run included cold use, 100 warm queries, and enough unique images to force cache eviction. Hidden total footprint was 136.4 MiB at the median with data URLs (133.7 to 177.1 MiB), and 182.6 MiB with binary responses and Blob URLs (169.0 to 246.7 MiB). The ranges overlap. The binary change has no demonstrated memory benefit and is deferred.

| Icon workload  | Data URL text, ms | Data URL decoded image, ms | Binary text, ms | Binary decoded image, ms |
| -------------- | ----------------: | -------------------------: | --------------: | -----------------------: |
| Empty cache    |              17.0 |                       54.5 |            16.0 |                     67.0 |
| Warm cache     |               5.0 |                       27.0 |             5.0 |                     28.0 |
| After eviction |               4.0 |                       39.0 |             4.5 |                     35.0 |

These values are medians of five run medians. Each run has two empty-cache queries, 100 warm queries, and two queries after eviction. Cold and post-eviction timings therefore have few samples. The eviction sequence requests 496 distinct image identities and exceeds both cache budgets. These scripted query and image-decode timings are separate from native keyboard latency. With an injected 500 ms image delay, selected-format text appears in 2 to 3 ms while decoded images arrive in 545 to 547 ms. Text does not wait for image extraction.

Settings release passed twenty varied editing-session cycles. These included partial and invalid input, successful and failed Save operations, shortcut recording, capture failure, timeout, late acknowledgement, immediate reopen, appearance changes, import previews, external updates, Discard, and section and scroll restoration. A separate build checked a native shortcut-registration failure. These tests were necessary, but were not sufficient to establish safe native destruction.

The first release prototype retained native Tao windows: 1 initially, 21 after 20 visits, and 41 after 40 visits. A local Tao 0.35.3 ownership patch changed one retain operation to adoption of the owned reference. Native window counts then remained 1, 1, and 1. The 100-visit diagnostic also became stable after its first group. These were single-process diagnostics, not five independent launches.

A later daily-use run crashed on its twentieth Settings close. A separate 300-cycle test passed. A second prototype released the webview before the native window and waited for native destruction before recreation. It passed the varied editing checks, but crashed during close 942 of a 1,000-cycle test. The failures occurred in WebKit layer-commit code on macOS 27.0, build 26A428. Their exact cause is not established. The close-order change did not resolve the failure.

Settings release failed the zero-functional-failure requirement. Its source, tests, native allocation records, Tao patch, and crash reports are retained for later investigation. None of its window-release code or dependency patch is in the selected product. The current Settings hide behavior preserves the live editing session. No Settings memory saving is claimed for this implementation.

The selected lockfile adds direct dependencies on already locked base64 and Tokio packages. It does not include the Windows dependency changes made by the deferred Tao trial. Native desktop validation is limited to macOS.

The selected comparison uses 20 fresh processes: five per build and profile. Both builds use the original Settings hide behavior. The following values are medians of five run medians. Full per-run values and ranges are in the [selected comparison tables](benchmarks/2026-09-19/optimization/selected-tables.md).

| Profile    | State            | Baseline, MiB | Selected, MiB | Change, MiB |
| ---------- | ---------------- | ------------: | ------------: | ----------: |
| controlled | Visible          |         136.6 |         158.6 |       +21.9 |
| controlled | Hidden           |         127.1 |         146.2 |       +19.2 |
| controlled | After Settings   |         189.4 |         185.5 |        -3.9 |
| controlled | Sampled peak     |         338.3 |         376.1 |       +37.8 |
| controlled | Hidden host only |          33.4 |          27.5 |        -5.9 |
| daily      | Visible          |         352.8 |         318.7 |       -34.1 |
| daily      | Hidden           |         186.1 |         276.3 |       +90.1 |
| daily      | After Settings   |         239.2 |         296.4 |       +57.2 |
| daily      | Sampled peak     |         545.5 |         507.3 |       -38.2 |
| daily      | Hidden host only |          67.2 |          70.4 |        +3.2 |

These are differences between complete builds. Do not add them to isolated Settings, minimal-page, or icon-trial savings. A sampled peak can miss a short allocation. Host-only values exclude WebKit and other helpers.

Selected-build hidden memory by process group:

| Profile    | Host, MiB | Web content, MiB | Web GPU, MiB | Web network, MiB | Other helpers, MiB |
| ---------- | --------: | ---------------: | -----------: | ---------------: | -----------------: |
| controlled |      27.5 |             81.4 |         12.7 |              6.4 |               17.9 |
| daily      |      70.4 |            158.9 |         12.7 |              6.2 |               15.5 |

These component medians need not add to the median total. A large WebKit process group identifies where memory is held. It does not establish that all that memory is required by the engine.

| Profile    | Operation   | Baseline p50 / p95, ms | Selected p50 / p95, ms | Performance gate |
| ---------- | ----------- | ---------------------: | ---------------------: | ---------------- |
| controlled | App search  |            29.5 / 53.7 |            30.5 / 57.1 | inconclusive     |
| controlled | Arithmetic  |            16.9 / 50.3 |            16.3 / 50.1 | inconclusive     |
| controlled | Warm reopen |            39.1 / 49.0 |            36.9 / 47.8 | inconclusive     |
| daily      | App search  |            49.7 / 64.1 |            52.2 / 65.7 | inconclusive     |
| daily      | Arithmetic  |            15.8 / 48.8 |            18.1 / 51.5 | inconclusive     |
| daily      | Warm reopen |            34.4 / 47.3 |            37.0 / 46.2 | inconclusive     |

The controlled hidden median change (+19.2 MiB) does not exceed the larger same-build range (34.5 MiB).

The controlled after settings median change (-3.9 MiB) does not exceed the larger same-build range (49.1 MiB).

The daily hidden median change (+90.1 MiB) does not exceed the larger same-build range (109.8 MiB).

The daily after settings median change (+57.2 MiB) does not exceed the larger same-build range (98.6 MiB).

An inconclusive performance gate is not a pass. The summary records the observed deltas, budgets, and independent-run spread. It retains valid outliers and every attributed helper.

The sustained-use check uses one process per build. Each process completes ten groups of 100 queries and 20 warm reopen operations. The table compares the hidden sample after group 1 with the sample after group 10. It is a growth diagnostic, not an independent-launch savings estimate.

| Build    | After 100 queries, MiB | After 1,000 queries, MiB | Change, MiB |
| -------- | ---------------------: | -----------------------: | ----------: |
| Baseline |                  162.2 |                    174.4 |       +12.2 |
| Selected |                  288.5 |                    128.1 |      -160.4 |

The selected build had a high first group. Groups 2 through 10 ranged from 127.4 to 131.1 MiB. This later range does not establish a savings estimate or prove that all unused allocations were released.

Selected-build process changes over the same interval:

| Process                                | After 100 queries, MiB | After 1,000 queries, MiB | Change, MiB |
| -------------------------------------- | ---------------------: | -----------------------: | ----------: |
| SetStoreUpdateService                  |                    2.3 |                      2.3 |        +0.0 |
| com.apple.SafariPlatformSupport.Helper |                   10.5 |                     10.5 |        +0.0 |
| com.apple.WebKit.GPU                   |                   13.0 |                     12.8 |        -0.2 |
| com.apple.WebKit.Networking            |                    6.4 |                      6.4 |        -0.0 |
| com.apple.WebKit.WebContent            |                  219.6 |                     63.3 |      -156.2 |
| com.apple.audio.SandboxHelper          |                    5.2 |                      2.9 |        -2.3 |
| tinydash                               |                   31.5 |                     29.8 |        -1.7 |

The raw records identify each process group and its retained footprint. They do not identify individual JavaScript objects or prove that the app is free of leaks. No further JavaScript or rendering change is justified by these records alone.

The capacity check uses one separate process per build, 100,000 files, 500 unpinned clipboard entries, and 101 pinned entries. Pinned entries remain outside the unpinned-history limit. Each added test entry contains 16 KiB of synthetic text.

| Build    | Hidden, MiB | After file refresh, MiB | Sampled peak, MiB |
| -------- | ----------: | ----------------------: | ----------------: |
| Baseline |       232.7 |                   199.9 |             263.4 |
| Selected |       195.5 |                   177.0 |             247.3 |

These maximum-setting values are separate from the controlled and daily-use comparisons. Detailed icon, repeated-use, and capacity results are in [diagnostics.json](benchmarks/2026-09-19/optimization/diagnostics.json).
