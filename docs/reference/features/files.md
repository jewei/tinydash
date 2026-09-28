# File search

TinyDash scans Desktop, Documents, and Downloads after the launcher opens. It uses the OS folder locations, including Windows Known Folders and Linux XDG user directories. Missing default folders are skipped. Select Files to search file and folder names and paths, or search in All mode. Press Enter to open a file with its default application, or a folder in Finder, File Explorer, or the default file manager. Command/Ctrl + Enter shows the item in its parent folder.

The index contains regular files and folders under the roots. The roots themselves are not results. It excludes dot files, hidden files, Windows system files, symbolic links, and the `node_modules` and `target` folders. Hidden and excluded folders are not traversed. Configured roots must be folders and cannot be symbolic links. Overlapping roots are scanned once. Paths that cannot be represented as UTF-8 are skipped. Access errors appear as a scan warning; other folders remain searchable. TinyDash does not read file contents.

The default limit is 50,000 files and folders together. A scan also stops after visiting 500,000 entries, including folders. The UI reports a limit when the index is incomplete. Use smaller roots if a large directory reaches a limit. Scans and index preparation run on a background worker. The previous index remains searchable during a scan with the same settings. Changing file settings immediately clears the old snapshot so removed roots cannot remain searchable. Each settings change advances a generation, including changing roots back to an earlier value. Root lookup, alias resolution, traversal, and watch registration check that generation and shutdown state between operations; obsolete partial results and partially registered watches are discarded and cannot replace the current index. A cancelled cycle notifies the UI before waiting for its next budget. For the current settings, the status is queued when replacement work is pending, or disabled when no search folders are selected; an obsolete cycle cannot restore its old status or warning. Cancellation cannot interrupt an OS filesystem call that is already blocked. Search uses memory, returns at most 30 results, and applies usage scores before limiting file results.

File actions and missing pins use an expected O(1) hash-table ID lookup rather than scanning the whole index. This deliberately adds one owned ID and one table entry per indexed file. On a 64-bit host, ID bytes plus 24 bytes per table capacity slot are a lower bound; control bytes, spare buckets, and allocator overhead add more. The table is replaced and released with its index. It caches neither file contents nor complete result objects. The [synthetic search benchmark](launcher.md#synthetic-search-measurements) reports this estimate for each fixture size.

File matching uses Unicode NFC so composed and decomposed accents match. The small [unicode-normalization crate](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/) supplies canonical normalization. This fixes accented filenames returned by macOS. Display text, file IDs, and open actions keep the original path.

Operating system notifications update the file index after creation, renaming, moving, and deletion. A single worker combines a burst of changes, waiting for 300 ms of quiet or at most two seconds of continuous changes. Separately, after every worker cycle (including cancelled work), it rests for at least one second or three times that cycle's duration, whichever is longer. The measured work includes root lookup, alias resolution, watcher creation, traversal, index preparation, and watch registration. Rest starts only after that work completes; registration cannot consume the cooldown. Disposing obsolete watches during a wait also counts as work and extends the rest. This bounds sustained filesystem-work duty to 25% of work-plus-rest time; long cycles therefore delay automatic and manual refreshes beyond the settle window. Refresh requests and settings changes cannot bypass the workload budget. The worker retains only one pending request, reads the latest settings after waiting, and drops obsolete watches during the wait. There is no folder polling timer. Content-write and access events do not require filename indexing.

The launcher distinguishes **Waiting to refresh files...** from **Scanning files...**. Waiting includes burst settling and budget cooldown, even when the worker has already consumed and combined the queued signals. Existing same-settings results remain usable while waiting. A root change clears the old results but shows waiting, not a finished empty index. With no configured folders it shows **File search is off**. Only an idle, completed empty index shows **No files in the index**. A scanner startup failure shows **File scan unavailable**, preserves its warning, and can be retried with Refresh files. The internal file status uses one `phase` (`disabled`, `idle`, `queued`, `scanning`, or `failed`), rather than an additional independent indexing flag. Events announce transitions, not a countdown or repeated cooldown ticks.

Aggregate `File scan workload` logs report worker cycles as `scans`, cancellations, visited entries, total work milliseconds as `scan_ms`, and elapsed worker-window milliseconds (for cycle frequency). Preparation cancelled before traversal and cancelled registration contribute to these totals, not just completed scans. Metrics contain no full paths, filenames, queries, or error strings.

The [notify](https://docs.rs/notify/8.2.0/notify/) watcher uses FSEvents on macOS, ReadDirectoryChangesW on Windows, and inotify on Linux. Linux watches only folders accepted by the scanner, up to 8192 folders, so excluded trees do not consume recursive watches. Parent watches detect a removed or recreated search root. Registration limits or failures produce a warning. Network filesystems and restricted folders can omit events. **Refresh files** in the actions or tray menu remains available; Command/Ctrl + R refreshes files in Files mode. Set `fileWatchEnabled` to `false` for manual updates. File contents and file metadata are not stored in SQLite.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts --grep "file|scan"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::files
bun run test:rust -- launcher::files
bun run test:rust -- launcher::file_watch
```

Rust regressions pause a 4096-entry traversal during rapid root changes, reject an obsolete completed scan even after roots change back, and accept only the newest generation. They also cover stop/disable cancellation, same-settings snapshot retention, immediate removed-root invalidation, stale-warning rejection, aggregate counters, and workload limits under refresh floods. Worker-loop regressions use an injected clock at preparation, registration, rest, and settling boundaries to check complete-cycle accounting and cancellation status notifications without timing sleeps. They check queued follow-ups during scans, refresh floods consumed during cooldown, newest-root selection, disabling, stale phase/warning rejection, and retry after scanner startup failure. Browser regressions distinguish queued, scanning, finished-empty, disabled, and failed states, including Settings-window root-change notifications; they do not simulate real filesystem timing. Controlled watcher operations cancel at batch start, removal, addition, and commit to prove that the remaining obsolete operations are skipped; root-resolution checks cover cancellation between metadata and ancestor lookups. These are backend proofs, not integrated desktop checks.

Use Files and All with an isolated root. Create, rename, and delete a file without manual refresh. Confirm the visible result changes. Open a selected file and check the handler marker. Exercise Refresh files and Ctrl/Command + R when manual refresh changes.

Run `bun run verify:native` on Windows and Linux X11. Run the file-search desktop checks on macOS for FSEvents, permissions, and OS associations, and on Wayland when session behavior changes. A mocked result update does not prove filesystem notifications.

Create, rename, and delete files in the isolated test root. Confirm the result list follows each change. Open a result and check the temporary handler marker.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

File contents are not searched. Browser tests cannot prove file watching or OS associations.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
