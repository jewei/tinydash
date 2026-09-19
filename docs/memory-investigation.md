# TinyDash memory investigation

TinyDash's main memory cost is the webview that stays loaded while the launcher is hidden. In the saved comparison, the Rust process used 36.5 MiB. The WebKit content, GPU, and network processes used about 107.5 MiB together. These WebKit processes account for about 66% of the 163.1 MiB total. Component medians can differ slightly from the total median.

| Process group            | Hidden memory footprint |
| ------------------------ | ----------------------: |
| Rust host                |                36.5 MiB |
| WebKit content           |                85.6 MiB |
| WebKit GPU               |                14.9 MiB |
| WebKit network           |                 7.0 MiB |
| Other attributed helpers |                19.1 MiB |

The [comparison report](performance-comparison.md) measured three fresh release processes per app. Clipboard capture, file indexing, and currency updates were off. Thus, these features do not explain the reported gap. The 13.30 MiB app bundle is a disk measurement. It does not include the runtime allocations of WebKit and its helpers. WebKit uses separate processes for web content and other services. See the [WebKit architecture documentation](https://docs.webkit.org/Deep%20Dive/Architecture/WebKit2.html).

The screenshot describes commit `bccaa245284cc22a7bc0c96d3083dace3a1950ca`. The active checkout has further changes. The checks below use an isolated copy of that commit. The relevant window and icon behavior is also present in the active checkout. No product source was changed for this investigation.

The code explains why hidden memory remains high:

- [The main window](../src-tauri/src/launcher/window.rs) calls `hide()`. It keeps the window and webview alive. [The frontend](../src/App.tsx) stops new searches when hidden, but this does not release the webview.
- [The Settings close handler](../src-tauri/src/lib.rs) prevents closure and hides the window. This preserves an unfinished form. After the first Settings visit, [the window builder](../src-tauri/src/launcher/preferences.rs) reuses that webview.
- [Application scanning](../src-tauri/src/platform/macos.rs) loads an icon for every application. [Icon conversion](../src-tauri/src/platform/macos/icons.rs) makes a 192 by 192 PNG and stores it as a base64 data URL.
- [Application results](../src-tauri/src/providers/apps.rs) clone the icon string for each match. [Ranking](../src-tauri/src/ranking/mod.rs) limits the response later. Thus, a broad query can copy icons for entries that the UI never displays.
- [The frontend entry point](../src/index.tsx) imports both the launcher and Settings. The measured build has one JavaScript file of 93,182 bytes and one CSS file of 44,338 bytes. These file sizes alone do not explain an 85.6 MiB content process.

The isolated test found 125 application icons. Their encoded strings total 4,615,934 bytes, or 4.40 MiB. After the normal hide sequence, the page had 99 DOM nodes, zero result rows, and zero image elements. The high hidden memory therefore occurs even with an empty result list.

One release-process experiment measured these states in order. Each row uses five samples after a 30-second wait. All application windows were hidden during each sample group.

| State                                 | Total footprint | Change from previous state |
| ------------------------------------- | --------------: | -------------------------: |
| Normal hidden launcher                |       171.7 MiB |                   Baseline |
| Open Settings, then close it normally |       203.3 MiB |                  +31.6 MiB |
| Destroy the hidden Settings webview   |       173.5 MiB |                  -29.8 MiB |
| Request navigation to `about:blank`   |       172.9 MiB |      Less than 1 MiB saved |
| Destroy the main webview              |        97.6 MiB |                  -75.4 MiB |

Closing Settings normally retained a second WebKit content process of about 30 MiB. Destroying that window removed the second content process. The original comparison did not open Settings. Thus, this is an additional daily-use cost, not part of the screenshot's 163.1 MiB baseline.

Destroying the main window removed the remaining content process. The GPU, network, and other attributed helpers remained. This process also had an 18.2 MiB Metal compiler helper that the original comparison did not report. Thus, compare the changes within this experiment. Do not treat its absolute totals as new three-run benchmark results.

The recreation check failed. The test requested main-window recreation and display, but the accessibility check did not find a usable launcher within three seconds. The prototype therefore establishes a memory saving only. It does not establish a working idle-release feature or its reopen latency. The blank-page request produced little change. Its completion URL was not recorded, so that row needs a confirmed navigation check before a firm conclusion.

These measurements locate the main cost in the webview's lifetime. They do not separate fixed engine costs from caches and allocations retained after searches. A fresh minimal webview and Web Inspector memory categories are the next checks for that distinction. Do not treat the full 85.6 MiB content-process figure as unavoidable framework overhead.

Two further fresh-process checks put the icon result in context:

| Configuration                                      | Total footprint | Rust host | WebKit content |
| -------------------------------------------------- | --------------: | --------: | -------------: |
| All icon loading disabled                          |       151.2 MiB |  30.9 MiB |       83.5 MiB |
| Normal icon loading, repeated-search test baseline |       154.6 MiB |  37.0 MiB |       80.9 MiB |

Neither of these processes had the Metal compiler helper. Disabling all icons reduced host memory by about 6.2 MiB in this pair. Total memory differed by only 3.4 MiB because other components varied. This is one pair, and it removes all application icons. It is not a measured saving for a finished lazy-loading implementation. The data supports treating icons as a useful smaller target.

The repeated-search process then ran three more groups. Each group added 100 measured queries and 20 measured reopen operations, plus warmup operations. Hidden memory was 154.6 MiB initially, then 159.4, 161.5, and 162.4 MiB. Total growth was 7.7 MiB. Growth became smaller with each group. The test did not capture retained-object graphs or force garbage collection, so it cannot identify a leak or separate it from cache growth.

Across the three diagnostic processes, all 420 measured query checks and 90 normal reopen checks passed. The separate recreation check failed as described above. The [raw records and probe instructions](benchmarks/2026-09-19/memory/README.md) include that failure. The [summary](benchmarks/2026-09-19/memory/results/summary.json) contains process medians and check counts.

The [optimization plan](memory-optimization-plan.md) supersedes the initial implementation order after review of a second opinion. It starts with a new baseline and a verified minimal-page comparison. Product changes then cover Settings session preservation, selection before icon payload construction, and measured icon loading changes. Further frontend work follows allocation evidence. Main-window release remains experimental, and native UI work is deferred.

For icons, select dimensions from the actual display size and scale. At a scale of 2, a 36-pixel result icon needs 72 by 72 pixels. The current 64-pixel preview needs 128 by 128 pixels. Keep larger versions for higher display scales. A decoded 192 by 192 RGBA image uses 144 KiB before engine overhead. A 72 by 72 image uses about 20 KiB. These are image-buffer calculations, not measured total application savings.

Main-window destruction requires lifecycle work. [Window show and toggle](../src-tauri/src/launcher/window.rs) currently require an existing window. [Startup](../src-tauri/src/launcher/mod.rs) starts services through the frontend-ready command. The implementation must support window recreation, keep services running once, and deliver query and category state after the new frontend is ready. Test focus restoration and unsaved Settings changes. Preserve the current fast reopen path for recent use.

If the product must stay ready at about 50 MiB, test a small native launcher UI with the existing Rust services. The current measurements do not show that an always-loaded WebKit UI can meet that target. A native UI has a larger implementation and cross-platform maintenance cost. Measure a prototype before choosing that change.

Use a separate workload for daily-use memory. Populate a file index and clipboard history, then measure steady memory and refresh peaks. Clipboard entries have a 16 KiB text limit, but [the provider](../src-tauri/src/providers/clipboard.rs) keeps the original text, normalized text, and matcher text. [Database pruning](../src-tauri/src/db/clipboard.rs) also preserves pinned entries. A history count limit therefore does not set a complete byte limit. These are secondary targets because the screenshot's profile excluded them.

Check for leaks with repeated operations and retained-object measurements. High memory in one sample does not prove a leak. Web Inspector separates JavaScript, decoded images, render layers, and other page memory. Its allocation snapshots can show objects that remain after repeated operations. See [WebKit's memory debugging guide](https://webkit.org/blog/6425/memory-debugging-with-web-inspector/).

For each proposed change, repeat the release benchmark with the same test profile. Count the Rust host and all attributed helpers. Record hidden memory, visible memory, peak memory, app-search latency, arithmetic latency, and reopen latency. Add Settings-open-and-close and repeated-search cases. Keep cold recreation latency separate from warm reopen latency. Repeat on Windows and Linux before choosing cross-platform defaults.
