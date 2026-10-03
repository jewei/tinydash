# Launcher and navigation

Open TinyDash with Control + Shift + Space, the tray menu, or the command line. On macOS, TinyDash has no Dock icon and hides its menu bar icon by default. Enable **Shortcut > Show menu bar icon** in Settings to use that menu. The window stays resident after it hides. Drag the handle above the search field to move it. Window position lasts until the process exits.

Use **Actions > Reset window position** to center the launcher in the current monitor's work area. The query, category, and selected result stay unchanged. On a display smaller than the window, the drag handle stays visible at the top left. Wayland controls placement through the compositor, so this action reports that limit there.

All searches applications, files, clipboard text, emoji, calculations, system commands, and explicit tool commands. Empty All shows pins first, followed by up to six usage-based suggestions for apps, files, and emoji. It shows welcome examples when neither list has entries. Hidden, disabled, missing, and already pinned items are excluded from suggestions. Clipboard text, power commands, and generated values are never suggested. Settings → Search can turn suggestions off without deleting usage or pins. Text categories have a fixed priority before the 30-result limit. See [ranking](../../explanation/data-and-privacy.md).

Suggestions use the existing local frequency and recency scores. Ties use last-use time, then item ID, so repeated searches keep their order. Selecting a suggestion uses the same actions and keyboard controls as a search result. No network request or additional usage record is needed.

Search keeps one expensive request in flight and replaces waiting input with the latest query. New input, hiding, or disposal sends a separate lightweight cancellation for the active request. Stale replies are still ignored. A cancellation failure does not dispatch overlapping searches; the latest waiting query follows the old response. Cancellation acknowledgements finish before the next dispatch.

A successful current search clears a previous search failure, including a background retry of the same query after a data-change event. It does not dismiss an action or startup failure. Those errors take precedence while present and keep the existing explicit dismissal paths, such as editing the query or reopening the launcher. Obsolete successes and failures cannot change the current search error.

A request has one 250 ms cooperative budget, starting in the backend command before blocking-worker queueing and including search-lock wait, providers, and pins. App/file matching checks every 64 entries, clipboard matching every 16 entries, and calculator evaluation uses its interrupt callback. The backend also checks between providers and pins and before returning a response. Expired requests report a notice/error rather than a partially ranked list. This is not a hard input-to-paint guarantee: runtime scheduling, individual matcher calls, sorting, fixed-size providers, serialization, IPC, and WebView rendering can overrun the cooperative checkpoint interval.

Tab and Shift + Tab change categories while preserving the query and search focus. Arrow keys select results. Enter runs the selected action. Escape closes a dialog or menu before hiding the launcher. [Keyboard reference](../keyboard-shortcuts.md) lists the remaining controls.

While an input method composes text, its keys do not run launcher actions. Enter that commits composition leaves the launcher open. A later Enter can run the selected result.

The search field disables automatic text correction, capitalization, spelling checks, and inline writing suggestions. This keeps application names and commands as entered. These field settings do not control system Siri activation or its keyboard shortcuts.

The actions menu uses Command/Ctrl + K. When no result is selected, it shows appearance options and launcher commands without an empty result-action column. Use Search actions to filter commands. The search field's Clear search button stays available while the query has text, including after an empty result or a search error. Clearing the query returns focus to the search field.

On macOS and Windows, select a supported result and choose **Actions > Share** to open the system share chooser. Files, folders, apps, calculations, emoji, clipboard text, cleaned URLs, and web search URLs support this action. Generated passwords do not. Text is limited to 64 KiB. Rust resolves the selected result again before opening the chooser. The action does not change the clipboard or count as a launch.

Select a receiving app or cancel the chooser. TinyDash stays open and blocks other result actions during the native session. Windows releases that state when a target has accepted the prepared data, or when focus returns to the launcher after closing the chooser. A return from Share does not confirm delivery to a recipient. Available targets and accepted formats depend on the operating system and receiving app. Linux does not show Share; use Copy or the file actions instead.

Select an app result, including a pin or suggestion, then choose **Actions > Quit** or **Force Quit**. Both require confirmation, with Cancel focused when the app check completes. Force Quit can lose unsaved work. The launcher stays open after the request and does not count it as a launch. A successful request does not mean that the app has exited.

Rust resolves the app's catalog ID to its bundle or executable path on demand. Search does not enumerate processes. A single-use confirmation expires after 30 seconds and is bound to the process identity and action. An app that has exited, a protected process, or an ambiguous match is not terminated. Use **Actions > Utilities > Processes** when direct app matching is unavailable. See [app process limits](utilities.md#quit-from-app-results).

Empty search results suggest another search. A search failure shows **Search unavailable** and the error message, so users can distinguish a failed search from a search with no matches.

The tray can open the launcher, open Settings, refresh data, and quit. A second app launch shows the existing process.

## Verification

For Share, check the supported result kinds, Linux exclusion, menu filtering, disabled actions, and error feedback. On an identified macOS/Windows desktop build, share disposable text, an HTTPS URL, a file, a folder, and an app reference. Confirm the content in the receiving app and cancel before sending. Cancel the chooser without selecting a target, then reopen it. Check missing files, expired results, oversized text, target failure, and returning focus to the launcher. Confirm that other actions become available again and that the system clipboard is unchanged. Browser mocks and static compilation cannot prove the native chooser or transfer.

For app Quit and Force Quit, check menu filtering, Cancel and Escape, initial Cancel focus after preparation, preparation errors, expired confirmation, repeated clicks, background search updates, and launcher hide/reopen. Use a disposable app with unsaved text on each desktop. Confirm that Quit requests normal closure on macOS/Windows, Force Quit stops only the selected app, and another app with the same display name remains open. On Linux, check SIGTERM/SIGKILL and the pidfd kernel requirement. Check stopped apps, multiple matching processes, protected processes, and unsupported launchers. Static compilation does not prove process effects.

Run `bun run verify:browser tests/daily-workflows.spec.ts` for suggestions, pin order, keyboard activation, and the persisted settings choice. Run `bun run test:rust -- suggestions` for ranking, exclusions, limits, and restored usage. On each desktop, launch a disposable app fixture, reopen All, activate its suggestion, and check its output marker. Mocked results do not establish native launch or restart persistence.

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/welcome.spec.ts tests/categories.spec.ts tests/launcher.spec.ts tests/ui-polish.spec.ts
bun run verify:browser tests/launcher-controller.spec.ts tests/search-errors.spec.ts tests/native-subscriptions.spec.ts tests/launcher-warnings.spec.ts tests/ipc-contract.spec.ts tests/ipc-drift.spec.ts
```

The standalone controller checks cover result reconciliation, refresh selection, hidden/disposed replies, reopening before an old reply settles (the original single-flight queue must ignore the old result and keep the new search pending), failure state, same-query error recovery, superseded-success isolation, and browser-preview gating. Mocked-IPC browser regressions drive input and actions, then emit background events to check search-error recovery without an input edit, action/startup error survival, and stale success isolation while the latest retry is pending. These tests prove frontend ownership, not native event delivery or OS action outcomes. Subscription checks cover late registrations and teardown. Warning tests use changed display text to prove that settings repair follows structured codes rather than English prefixes; unrelated platform warnings remain visible. A storage warning must leave search results usable.

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

On macOS, type a misspelled word or application name in the search field. Confirm that no automatic correction popup or inline prediction appears and that the query remains as entered after a space. Browser tests cannot prove the absence of native text correction popups.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
