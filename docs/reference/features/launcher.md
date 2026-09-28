# Launcher and navigation

Open TinyDash with Control + Shift + Space, the tray menu, or the command line. The window stays resident after it hides. Drag the handle above the search field to move it. Window position lasts until the process exits.

Use **Actions > Reset window position** to center the launcher in the current monitor's work area. The query, category, and selected result stay unchanged. On a display smaller than the window, the drag handle stays visible at the top left. Wayland controls placement through the compositor, so this action reports that limit there.

All searches applications, files, clipboard text, emoji, calculations, system commands, and explicit tool commands. Empty All shows welcome examples or pinned items. Text categories have a fixed priority before the 30-result limit. See [ranking](../../explanation/data-and-privacy.md).

Search keeps one expensive request in flight and replaces waiting input with the latest query. New input, hiding, or disposal sends a separate lightweight cancellation for the active request. Stale replies are still ignored. A cancellation failure does not dispatch overlapping searches; the latest waiting query follows the old response. Cancellation acknowledgements finish before the next dispatch.

A request has one 250 ms cooperative budget, starting in the backend command before blocking-worker queueing and including search-lock wait, providers, and pins. App/file matching checks every 64 entries, clipboard matching every 16 entries, and calculator evaluation uses its interrupt callback. The backend also checks between providers and pins and before returning a response. Expired requests report a notice/error rather than a partially ranked list. This is not a hard input-to-paint guarantee: runtime scheduling, individual matcher calls, sorting, fixed-size providers, serialization, IPC, and WebView rendering can overrun the cooperative checkpoint interval.

Tab and Shift + Tab change categories while preserving the query and search focus. Arrow keys select results. Enter runs the selected action. Escape closes a dialog or menu before hiding the launcher. [Keyboard reference](../keyboard-shortcuts.md) lists the remaining controls.

While an input method composes text, its keys do not run launcher actions. Enter that commits composition leaves the launcher open. A later Enter can run the selected result.

The actions menu uses Command/Ctrl + K. The tray can open the launcher, open Settings, refresh data, and quit. A second app launch shows the existing process.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/welcome.spec.ts tests/categories.spec.ts tests/launcher.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- launcher
```

Open with the global shortcut, tray, and a second process when those entry points change. Check focus, arrow selection, Enter, Escape, and reopen. The selected fixture must launch and a second launcher process must not remain.

Move the window, enter a query, select a result, and use **Reset window position** from Actions. Check that it centers on the same monitor and keeps the query, category, selection, and input focus. Repeat on a second monitor when available. Browser tests check the command and retained state; Rust tests check work-area geometry. Desktop checks prove the actual move.

Run `bun run verify:native` on Windows and Linux X11 for launch and reopen. Use the desktop launcher checks for macOS, focus return, tray, physical shortcuts, input methods, and Wayland. The native suite does not prove those additional paths.

Open All with no pins. The welcome controls appear and the search field has focus. Choose Apps by keyboard, select a result, and confirm that Enter launches the selected fixture application.

Tests: [tests/welcome.spec.ts](../../../tests/welcome.spec.ts), [tests/categories.spec.ts](../../../tests/categories.spec.ts), [tests/launcher.spec.ts](../../../tests/launcher.spec.ts).

Native tests check launch and reopen on Windows and Linux. They also retain synthetic query-to-DOM and rendering-opportunity timings; see [performance samples and limits](../performance-samples.md). Global shortcuts, focus, tray behavior, and Wayland need desktop checks.

### Synthetic search measurements

Run the ignored in-process benchmark separately from desktop checks:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --release synthetic_search_percentiles -- --ignored --nocapture --test-threads=1
```

It reports p50/p95/p99 and maximum microseconds for whole search, providers, and pins at 1k, 50k, and 100k synthetic files, old file pins, 1,000 near-16-KiB clipboard entries with old pins, and 30 expensive calculator pins. The default is 40 measured requests per case after three warmups; `TINYDASH_BENCH_SAMPLES` selects 20–500 samples. Timeout counts are reported, not silently excluded. The file-ID table estimate is a lower bound, not RSS. Fixture construction is reported separately. This harness does not measure IPC, contended worker queues, or native input-to-paint.

Backend debug events contain only aggregate durations and counts: blocking-worker queue wait, search-lock wait, provider calls/time, pin attempts/time, indexed item counts, response rows, and total request/search time. Provider order is apps/files/clipboard/system/emoji/calculator/tools; calculator time inside pins is included in both phases. No query text, paths, clipboard contents, result IDs, passwords, or tool values are logged. The frontend queue has an optional duration-only instrumentation callback; it does not log by default.

The finite seeded selection-equivalence test disables request/calculator wall clocks only in its test fixture so host scheduling cannot change one side into a timeout. Separate regressions exercise normal deadlines and cancellation. No production deadline is disabled.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
