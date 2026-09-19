# TinyDash memory optimization plan

Reduce memory while preserving search results, unsaved settings, keyboard control, and fast normal reopening. Keep the current Tauri UI for this work. Release each change separately so that its effect can be measured and reversed.

Implementation status on 2026-09-19: result-copy reduction and on-demand data URLs are implemented as separate review patches. The final comparison shows lower controlled host memory, but no demonstrated total-memory reduction. All latency acceptance results are inconclusive. The combined build remains a review candidate. Binary delivery is deferred because its memory benefit was not established. Settings release failed two native close tests and is also deferred; the original hide behavior is retained. The experimental Tao patch is absent from the working product. The [implementation report](memory-optimization-results.md) records the failures and validation.

This plan includes the acceptance details from both reviews, checks of the local code, and the [recorded measurements](benchmarks/2026-09-19/memory/results/summary.json). No new performance measurements were made for this review. The recorded experiment used commit `bccaa24`. The local code reviewed on 2026-09-19 is at `568972bbc92df37f2bbfd2951e8ab478b061e205` with further uncommitted changes. A new baseline must include the exact source selected for implementation. Source observations below refer to this local revision, not an assumed match with the public repository.

The second opinion is correct about the main limits. Memory in WebKit processes is not all proven to be unavoidable engine memory. The Settings, icon, and window-destruction savings cannot be added together. They came from different workloads and process groups. A native rewrite is not justified by these measurements.

| Evidence                                                                                                                 | Planning consequence                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Destroying Settings saved 29.8 MiB in one process.                                                                       | Address Settings first among product changes. Preserve the complete editing session before destruction. This saving applies after a Settings visit. |
| Removing all icons reduced host memory by 6.2 MiB in one comparison pair. Total memory differed by 3.4 MiB.              | Treat icons as a smaller opportunity. Measure the finished implementation instead of promising the removal-test saving.                             |
| Destroying the main webview saved 75.4 MiB, but recreation failed.                                                       | Keep main-window release experimental. Diagnose readiness and activation before adding an idle timer.                                               |
| The blank-page experiment did not record its completed URL.                                                              | Use a verified minimal local page in a fresh process. Do not use the blank-page result to decide the UI architecture.                               |
| The repeated-use test grew by 7.7 MiB overall. WebKit content grew by about 9.0 MiB, while other processes also changed. | Investigate the content process first. Its footprint is not the same as its JavaScript heap.                                                        |

The work order is fixed below. P0 through P3 form the first optimization cycle. P4 follows the measurements. P5 is conditional. P4 and P5 must not delay the first release.

| Stage | Deliverable                                          | Required evidence before proceeding                                                                      |
| ----- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| P0    | Repeatable baseline and minimal-page comparison      | Verified page identity, matched backend work, complete process accounting, and a measured noise range    |
| P1    | Settings session preservation and window release     | Drafts survive actual destruction; shortcuts work; repeated visits do not retain more windows            |
| P2    | Selection before icon payload construction           | Identical results, order, pins, and actions; direct evidence that discarded candidates do not copy icons |
| P3    | Icons loaded on demand through a bounded cache       | Text results never wait for icons; cache limits hold; memory and first-use latency are measured          |
| P4    | Targeted reduction of retained frontend allocations  | A measured allocation category falls without changing behavior, or a recorded decision to make no change |
| P5    | Optional release of the main webview after idle time | Reliable recreation, preserved background work, and acceptable interactive latency                       |

P0 establishes the comparison method before product code changes.

Freeze the candidate source in an isolated checkout. Include the selected uncommitted changes in that snapshot. Record the full base commit, working-tree changes, and a content hash for every included untracked source file. Preserve those files with the snapshot. Save its tree identity, patch, lockfiles, build configuration, and executable hash. Give the test app a separate identifier and data directory. Compare each change with its immediate parent build. Rerun the final combined build against the original frozen baseline.

Use five fresh application launches per configuration and workload. A configuration includes the build, page, and feature settings. Use the same release-build settings, installed-app catalog, fixture data, warmup, and settling periods for each matched comparison. Alternate baseline and candidate order across pairs. Keep the OS, display scale, theme, and power conditions constant. Stop unrelated builds. Record memory pressure, swap use, thermal state, and test order. Preserve every run. If a predeclared setup check fails, record it and repeat the complete affected pair.

Use macOS physical footprint as the primary memory metric. Keep the existing `proc_pid_rusage` reader and process-responsibility attribution. Sum the host and every helper attributed to that app. Save PID, executable, footprint, and RSS separately. Include helpers such as Metal compiler processes. Record process arrivals and exits. Do not silently remove them to improve a result. Treat host-only RSS as a different metric.

Record visible-idle and hidden footprint for each workload in a fixed order. Use the same visible content in each matched comparison. For each state, wait 30 seconds, then take 31 samples at one-second intervals. Also sample throughout the workload to record its peak. Report all five run results, their median, and their range. Samples within one process are not independent launches. If a change is within the measured same-build variation, mark its memory effect inconclusive. Attribute transient allocation costs in a separate profiler run. Label sampled footprint peaks as sampled peaks, because sampling can miss short allocations.

Use these workloads:

| Workload                 | Contents                                                                                                                                                |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Minimal launcher profile | Empty history, no file roots, clipboard capture and currency updates off; application search and arithmetic                                             |
| Settings lifecycle       | Twenty varied open/edit/close/reopen cycles; include the failure and race conditions specified in P1                                                    |
| Repeated launcher use    | Ten groups of 100 measured queries and 20 reopen operations, plus fixed warmup; take a hidden-memory sample group after each group                      |
| Daily-use profile        | File indexing, file watching, and clipboard capture enabled; the same generated 50,000-file tree, 100 clipboard entries, pins, and cached currency data |
| Capacity check           | Separate maximum-configured file and clipboard fixtures; measure refresh peaks and pinned-history growth without mixing these results with daily use    |

Use short and 16 KiB clipboard entries in the daily-use fixture. Complete the initial file scan before steady-state measurements. Exercise file updates and clipboard capture against test data. Record refresh peaks separately from idle states.

For the minimal-page comparison, bundle both pages in the same diagnostic release build. Select the page before creating the first webview. Keep the Rust initialization, app scan, native icon work, window dimensions, transparency, and display scale the same. Both pages must initialize the same services through the real startup handshake. A page that never starts the normal backend work is not a valid control.

Measure each page after startup and after the same backend request sequence. The minimal page must receive and discard complete responses. The real launcher must display the corresponding results. Include any icon requests in the matched backend work. Do not bypass response serialization, extraction, or delivery in the minimal page. Record request counts, response sizes, and service readiness to verify comparable work. Also measure the real launcher's normal keyboard workload separately. Record the actual URL and a page-specific DOM marker after load. A successful navigation request alone is insufficient.

The difference estimates additional memory associated with the real page under this workload. It does not show that the whole difference can be removed while preserving the interface. It also does not establish a universal WebKit minimum.

Keep Web Inspector and allocation profilers closed during release memory and latency measurements. Use separate diagnostic sessions to inspect JavaScript objects, decoded images, render layers, and other page allocations. These categories are distinct in [WebKit's memory tools](https://webkit.org/blog/6425/memory-debugging-with-web-inspector/). Replace the old probe's 100 ms command-file polling before measuring idle CPU or wakeups.

Measure normal search and reopen latency with at least 100 samples per operation across the five processes. Save p50, p95, and failures. Retain the existing accessibility-result timing for comparison. Add a separate check that the window is visible, the input has focus, and a test query produces the expected result. Use a short screen recording to check blank frames and delayed painting. An accessibility-ready signal or animation-frame callback does not prove that pixels have reached the display.

Use these initial engineering gates for ordinary search and warm reopening: p50 must not worsen by more than the greater of 2 ms or 5%; p95 must not worsen by more than the greater of 5 ms or 10%. These are proposed regression budgets, not measured improvements or user-specified requirements. Establish whether the harness can resolve them in P0. If noise exceeds a budget, the performance verdict is inconclusive. Keep Settings reconstruction, first-use icon loading, and main-window reconstruction as separate timing groups.

P1 releases Settings only after its editing session is safe.

Add a `SettingsSession` module in Rust. Its interface owns opening, close requests, and acknowledged session capture. Keep one session in memory for the app's lifetime. Do not write an unfinished draft to `settings.json` or treat closure as Save. Do not add persistence across app restart in this change.

The session must include more than `SettingsValues`. Preserve the saved-settings snapshot used for merging, the editable draft, raw folder and exclusion text, folder mode, appearance draft, selected section, and scroll position. Preserve relevant child-editor state, including the selected app, app filter, and text not yet committed by an `onChange` handler. Audit import-preview and other editing state before defining the snapshot type. Do not serialize DOM objects or the loaded application catalog.

Accept structurally valid draft state even when the form contains invalid or incomplete settings. Apply normal settings validation only on Save. Bound the number and size of stored snapshots. Derive the size limit from accepted settings and editor limits. If capture fails or exceeds that limit, preserve the existing webview and input.

Use this close sequence:

1. Intercept the native close request and assign it a generation number. Start an asynchronous close attempt with a finite deadline.
2. Stop new edits and Save requests. Read active editor input into the editing state. Stop shortcut recording.
3. Let any active Save finish, including its frontend continuation. Apply the same rule to imports and other operations that still need this webview. A failed Save must leave its draft and error available for capture.
4. Capture one consistent snapshot after those operations settle. Transfer it to Rust with the close generation and editing-state revision. Rust must reject stale snapshots. Continue only after Rust acknowledges successful storage outside the webview.
5. Restore committed global and category shortcuts through the existing Rust path. Complete other native cleanup. Recheck the close generation, then destroy the window.
6. A new open request cancels pending destruction and restores editing in the existing window. If destruction has finished, create one window. Load current committed settings and merge the stored session with them. Restore the selected section, child editor, and scroll position after layout.

Start with a five-second deadline for the close attempt. If capture fails, exceeds its limit, or times out, cancel closure and keep the existing webview and session. Restore editing and shortcut operation on this failure path too. A timeout cancels closure, not an active Save. Keep the webview alive for that Save to finish. Ignore late acknowledgements from cancelled attempts. Test that an old snapshot cannot replace newer input after cancellation and reopening.

Use an asynchronous close coordinator. Do not block the native event loop while waiting for a frontend reply. Do not rely on frontend `onCleanup` to restore native shortcuts after destruction. If shortcut registration fails, keep the window usable and display the error so the user can choose another shortcut. Successful native cleanup is required before destruction. Tauri's `close()` sends `CloseRequested`; `destroy()` bypasses that close-request path. See the [Tauri window interface](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.destroy).

The local [Settings code](../src/Settings.tsx) saves appearance immediately in the radio control's `onChange` handler. Its `save()` function also saves appearance after `saveSettings()` returns if the appearance values differ. The earlier claim that appearance saves only after Save was incorrect. Preserve the immediate-save behavior. Test closure during both successful and failed Save operations. Keep the frontend alive until all required continuations finish, and do not repeat completed operations after reopening.

Acceptance requires native destruction, not only a mocked close event. Use at least 20 varied native cycles. Include partial and invalid input, an open child editor, imports, appearance, Save, Discard, and external settings updates while closed. Include closure during successful Save, failed Save, and shortcut recording. Inject capture failure, capture timeout, shortcut restoration failure, and an immediate reopen during closure. Verify exact editing-state restoration and global and category shortcuts after each applicable cycle. For each failed close attempt, verify that the original webview remains and the user can recover.

Verify that only one Settings session and at most one Settings window exist. Check that repeated successful visits do not accumulate content processes. Measure process count and footprint after 20 cycles. Confirm a repeatable reduction against the build that retains Settings. The previous 29.8 MiB is evidence of an opportunity, not the acceptance threshold.

Primary files are [Settings](../src/Settings.tsx), [child editors](../src/components/SearchPreferences.tsx), [preferences](../src-tauri/src/launcher/preferences.rs), [window events](../src-tauri/src/lib.rs), and [the bridge](../src/bridge.ts). Extend [Settings tests](../tests/settings.spec.ts) and [native checks](../tests/native/smoke.ts). Rust tests should exercise the session interface, including stale close acknowledgements.

P2 removes unnecessary icon copying while retaining the ranking policy.

Keep the existing search interface and selection rules. Application candidates should carry identity, score, and the metadata required for selection. Attach the large icon payload only after usage scoring, category ordering, pin handling, and the final response limit. Start with deferred icon attachment; a general rewrite of all providers is unnecessary unless allocation measurements justify it.

Cover all application-result paths. In [apps.rs](../src-tauri/src/providers/apps.rs), `AppEntry::result()` clones the icon string. `AppProvider::catalog()` calls that function, then immediately sets `result.icon` to `None`. This confirms the local Settings catalog issue. Build catalog metadata directly; do not create full results and strip icons afterward. Pinned applications must also avoid icon copies before final selection. Keep non-application icon semantics, including emoji, unchanged.

Do not take the first 30 raw matches. Preserve name and alias scores, usage bonuses, hidden-app behavior, category priority, empty-query pins, missing-pin resolution, and stable ties. Preserve the newest clipboard entry rule when the response contains pins. The optimization must not change selected results or their actions.

Compare the old and new implementations through `SearchManager::search()` on fixed fixtures. Require identical result identifiers, order, pin information, and actions. Cover empty queries, broad matches, ties, aliases, Unicode, hidden apps, usage, and more than 30 pins. Fix timestamps in comparisons. Compare values and actions for tools whose IDs intentionally change. Keep the existing ranking tests.

Instrument icon payload construction and copied bytes in a separate diagnostic test. Use many large synthetic icons. Require zero payload copies for discarded applications and for the Settings catalog. Count copies for returned results separately. A smaller response alone does not prove that intermediate copies were removed. Keep this instrumentation out of production timing runs.

Measure allocation work, peak memory, and steady footprint separately. Accept the change when it removes demonstrated copying and preserves behavior and latency. A large reduction in steady hidden footprint is not required for P2.

Primary files are [application results](../src-tauri/src/providers/apps.rs), [search orchestration](../src-tauri/src/launcher/search.rs), and [ranking](../src-tauri/src/ranking/mod.rs). Reuse the existing ranking tests. Keep the existing response shape for this stage so that frontend changes do not obscure its effect.

P3 makes application icon loading independent of text search.

Add an `IconStore` module. Its interface resolves a catalog-owned application identifier and requested pixel size to image bytes. It owns cache lookup, duplicate-request suppression, a bounded work queue, native extraction, and invalidation. Application discovery should publish searchable text without resolving every icon. Search must not wait for a cache miss.

Implement this in two small changes. First, load only icons requested by displayed results and the selected preview. Measure it while retaining the existing delivery format where practical. Second, use stable icon keys and binary image responses to remove repeated base64 data from search replies. Measure the second change independently. It must justify its added delivery mechanism.

Use these explicit limits for the first trial. Revise them only from measurements and record the chosen values with the build.

| Resource                           | Initial limit | Behavior at the limit                                                                                 |
| ---------------------------------- | ------------- | ----------------------------------------------------------------------------------------------------- |
| Stored encoded icon data           | 2 MiB         | Evict the least recently used entries before insertion. Return an oversized icon without caching it.  |
| Active extraction jobs             | 2             | Queue new work within the queue limit. Never run extra extraction work on the search path.            |
| Queued unique requests             | 64            | Remove obsolete requests first. If still full, return a capacity response and retain the placeholder. |
| Cached entries, including misses   | 256           | Evict old entries so small or missing images cannot cause unbounded metadata growth.                  |
| Pending consumers                  | 128           | Return a capacity response when no consumer slot is available.                                        |
| Frontend entries for visible icons | 64            | Keep the placeholder until the row mounts again.                                                      |

Count the stored representation, including base64 bytes until binary delivery replaces it. Count cache metadata, active-job memory, and data retained by responses separately. Reject unsupported image sizes. Search completion must not await icon work. Neither extraction nor decoding may hold the lock used by search.

Requests for the same cache identity must share one queued or active load. On query changes, remove queued requests that have no remaining consumers. Give current visible results priority. Cancel active work where supported; otherwise let it finish within the two-job limit. Never attach a late response to a row based only on its position. Check application identity, icon revision, pixel size, and the current view generation first. Retry a capacity response only when the result is still visible and capacity becomes available. Prevent repeated retries from creating another unbounded queue.

Choose the image size from rendered size and display scale. At scale 2, a 36-pixel row needs 72 by 72 pixels, and a 64-pixel preview needs 128 by 128 pixels. Include catalog generation, icon-source revision, and requested size in cache identity. Invalidate after app refresh or icon-source changes. Test custom icons, missing icons, application replacement, and mixed-scale displays.

Use placeholders with fixed dimensions while icons load. Missing or failed icons must not block selection, keyboard input, or launch. Avoid a second unbounded JavaScript image cache. Remove obsolete image references when result rows disappear. A Rust cache limit does not bound WebKit's decoded images or native framework caches; measure those separately.

For binary delivery, resolve only identifiers already known to the application catalog. The binary trial uses Tauri's binary command response and temporary Blob URLs. This uses the existing command transport and removes base64 image strings without adding a custom image route. Allow `blob:` only in the image content policy. Check the native transport on each supported platform before claiming equivalent desktop validation there. Keep platforms without native icon support on their existing fallback.

Acceptance requires search to complete and text results to appear while icon resolution is deliberately delayed. Assert the byte, active-job, and queue limits throughout the test. Test duplicate requests, cancellation, queue pressure, cache eviction, invalidation, scale changes, and failures. Change queries rapidly and force late completions; no result may receive another application's icon.

Measure three conditions: first use with an empty cache, repeated use with a warm cache, and sustained use that forces eviction. Include result scrolling. Report text-result latency and icon-arrival latency separately in each condition. Record peak and steady hidden footprint. Keep a change only when its measured memory or allocation benefit holds without exceeding the latency budgets. Report the new cache's own results; the earlier test with all icons disabled is not its expected saving.

Primary files are [app scanning](../src-tauri/src/launcher/mod.rs), [native icons](../src-tauri/src/platform/macos/icons.rs), [result types](../src-tauri/src/launcher/result.rs), [AppAvatar](../src/components/AppAvatar.tsx), [ResultIcon](../src/components/ResultIcon.tsx), [the bridge](../src/bridge.ts), and [the content policy](../src-tauri/tauri.conf.json). Add tests through the icon module interface and the real result UI. Update the existing [native icon checks](../src-tauri/src/platform/mod.rs). Check returned image bytes, dimensions, and distinct app icons instead of relying on the old data-URL prefix.

P4 uses the measured page cost to select further changes. It can proceed separately from the P1 through P3 release.

If JavaScript objects remain after repeated use, inspect retention paths and release the specific listeners, promises, strings, or result state responsible. If decoded images dominate, remove retained image references and reduce image dimensions. If render layers dominate, test layer release while hidden with the same visible layout and window configuration. Do not remove transparency or change the design without evidence that it addresses the measured cost.

Consider loading Settings JavaScript and CSS only when Settings opens. [The entry point](../src/index.tsx) currently imports both views. Measure the change; the current bundle size alone does not establish a large memory saving. Keep this separate from P1 so its effect remains visible.

For each proposed change, record the retained allocation, why the app no longer needs it, and its measured production effect. Do not add periodic forced garbage collection, reload, or webview destruction to conceal unexplained growth. Use before/after allocation snapshots in the process that grows. Repeat the release workload without an inspector to validate any resulting change.

Stop this iteration if no sufficiently valuable retained allocation is identified. Record that no frontend change is justified. Complete knowledge of WebKit's footprint is not required to release useful Settings and icon changes.

P5 remains a separate, optional experiment. Use the P0 comparison method and available P4 findings. It must not block the P1 through P3 release.

First diagnose the failed recreation with staged timestamps. Record native window creation, completed page URL, frontend listeners ready, restored state, focus, accepted input, correct results, and visible painting. The existing failure does not identify which stage failed. Do not add an idle timer until explicit destroy-and-recreate cycles pass.

Separate application startup from each window's readiness. Keep database, indexing, clipboard, and shortcut initialization independent of renderer creation. Reuse the existing once-only guards where suitable. A new frontend must receive its own state and activation request after its listeners are ready. It must not restart background workers or replay the original startup request.

Make a window-lifecycle module own absent, creating, ready, and releasing states. Route global shortcuts, category shortcuts, tray actions, single-instance activation, command-line modes, and macOS reopen events through its activation interface. Permit one creation at a time. Preserve the final requested visibility and category when requests arrive during creation. Recheck a generation number before a delayed release so an old timer cannot destroy a reopened window.

Keep the app alive when an intentional idle release removes the last window. Explicit Quit must still exit and stop workers. Preserve query, category, selection, position, and focus behavior according to the current settings. Keep background file and clipboard work active while the main webview is absent.

After these tests pass, trial a 60-second hidden delay behind a default-off option. This delay is a tuning value. Test at least 100 release/recreate cycles, rapid shortcut bursts, reopen racing the timer, activation through every route, monitor changes, and background updates. Use an injected clock in state tests and an explicit diagnostic release command in native cycle tests. Check the real delay separately, without waiting 60 seconds for every functional cycle. Measure warm reopening and reconstruction separately, including input and visible results.

Set the promotion conditions before running the experiment. The initial targets are at least 25% lower total hidden footprint and reconstruction p95 no greater than 250 ms. Compare the same combined P1 through P3 build with the option off and on, using the reference Mac. Measure from activation until the window is visible, accepts input, and displays the expected query result. Apply the P0 search and warm-reopen regression budgets as well. Require zero functional failures, no duplicate background workers, and successful background updates while the webview is absent.

These are proposed experimental targets, not current results or user-specified requirements. If any condition fails or the measured benefit is inconclusive, keep the window loaded by default. Retain the completed independent optimizations. Establish equivalent measurements on other operating systems before enabling the option there.

Primary files are [window activation](../src-tauri/src/launcher/window.rs), [startup requests](../src-tauri/src/launcher/startup.rs), [application readiness](../src-tauri/src/launcher/mod.rs), [native events](../src-tauri/src/lib.rs), and [frontend readiness](../src/App.tsx).

Each product change must pass its relevant Rust and UI checks, native lifecycle checks where applicable, and a release build. Use the existing `cargo test`, `bun run build`, and Playwright commands. Run the full affected suites once the focused checks pass. Use dedicated desktop measurements for performance budgets instead of timing thresholds on shared CI runners. Record the build, workload, per-process results, latency distributions, failures, and retained tradeoffs with each change.

The first release decision covers P1, P2, and measured parts of P3. Compare the combined build with the frozen baseline using five fresh launches per configuration in both controlled and daily-use profiles. Report each run, spread, process membership, hidden and visible footprint, sampled peak footprint, query latency, and warm-reopen latency. Include separate Settings-reopen and icon-arrival timings. Do not sum isolated savings.

Main-window destruction stays off until its own acceptance conditions pass. Reconsider a native UI only if the measured final result misses an explicit product memory budget. A separate native prototype must then demonstrate the required memory and latency together.
