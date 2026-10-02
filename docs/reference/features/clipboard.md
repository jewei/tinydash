# Clipboard history

On a fresh installation, TinyDash asks before it saves clipboard text. Existing installations keep their saved capture choice. When capture is enabled, TinyDash captures text while it runs, including the current clipboard at startup. Select Clipboard mode to see recent entries. Pins stay at the top, and a fresh empty search selects the latest entry. Search saved text, use the arrow keys to inspect its full preview, and press Enter to copy it. Paste it in the target application with Command/Ctrl + V, or choose **Paste to previous app** / Command/Ctrl + Shift + Enter. See [direct paste](workflows.md#direct-paste) for permissions and platform limits.

The default limit is 100 unpinned entries. Pins in Clipboard or All keep an entry outside this limit. Each entry can contain up to 16 KiB of UTF-8 text. Empty text, whitespace-only text, embedded null characters, and larger values are skipped by text history. Images and file references have a separate opt-in history described below. TinyDash preserves the accepted text exactly. Repeated consecutive values do not create writes. Copying an older value moves its existing entry to the top. Equal search scores keep the newest entries first.

An empty history search constructs at most 30 recent rows before pins are applied. Missing pins resolve their saved entry directly instead of constructing the entire history per pin. Direct clipboard lookup still scans numeric IDs in newest-first storage; it allocates only the requested result. Pins remain first and a full pinned page still leaves a slot for the newest entry.

Use the actions menu or Command/Ctrl + Backspace to delete the selected entry. **Clear unpinned** keeps entries pinned in All or Clipboard. The separate **Clear all clipboard history** action includes pins. Both ask for confirmation. Deleting an entry removes its pins from both categories. These actions leave the system clipboard unchanged. An unchanged clipboard is not captured again during the same session. Restarting TinyDash captures the current clipboard again when capture is enabled.

The actions menu can edit a copy, combine selected entries in a chosen order with a separator, or save the full text as a file. Edited and combined copies leave history unchanged. Editing and combining leave the original entries unchanged. Combining accepts up to 100 entries and 16,384 output bytes. Saving uses the native Save dialog.

History is local plain text in the same SQLite database as usage. It is not encrypted. On Unix, the database is restricted to its owner. SQLite secure deletion is enabled, but backups and filesystem snapshots can retain earlier data. Turn off **Save clipboard history** in Settings to stop capture. Existing history remains searchable and can be cleared.

## Retention, exclusions, and rich history

Settings adds **Retention in days** (0 means no age limit, maximum 3650). Text entries expire by original capture time; copying an old entry does not reset its age. Pins remain exempt. Retention is checked at startup, capture, policy changes, and while the monitor runs. Count limits still apply.

**Excluded applications** accepts exact case-insensitive application names or identifiers. macOS samples the frontmost app's name/bundle ID; this is best-effort attribution. Windows uses the clipboard-owner executable path/basename. Unknown attribution fails closed when exclusions are configured. Linux cannot reliably attribute the source here, so a nonempty exclusion list suspends capture rather than pretending it can enforce per-app exclusions. Secret markers remain independently enforced.

Enable **Capture clipboard images** or **Capture copied files** separately, alongside clipboard-history consent. Open **Actions → Images and files clipboard**, or its All/System command, to preview, copy, and delete these entries. Native rich capture/copy currently supports macOS only; other platforms display this limit. PNG images are supported; TIFF-only and animated images are skipped. Copied files are references to existing local originals, not backups. No text edit/combine action is applied to rich content.

Rich history is a separate owner-restricted `clipboard-rich.sqlite3`, not an overloaded text table. It holds at most 32 entries and 16 MiB of payloads; the configured history count can lower that limit. Each PNG is limited to 4 MiB, 4 megapixels, and a 4096-pixel edge. File lists are limited to 64 references. Rich entries have no pins and obey age retention. Clear-history actions also clear rich entries; the system clipboard is unchanged. Settings exports and the existing text-database migration archive do not back up rich history.

See [clipboard format details](clipboard-formats.md) for source identity, additional format bounds, and native acceptance checks.

## Storage contention

If another process holds the SQLite write lock, TinyDash waits up to 250 ms per SQLite lock attempt, then shows a busy warning. It keeps the connection and retries on the next storage operation; no restart is needed after the other process releases its lock. Waiting happens on storage workers, never while holding the search lock.

A failed user delete or clear leaves history and pins visible. Repeat the action after contention ends; failed user actions are not queued for later execution. Failed captures are not replayed either: copying a new value resumes capture. Consecutive unchanged clipboard values remain suppressed. Usage changes continue in memory and are saved together on the next successful storage operation. If TinyDash quits before that succeeds, those changes are lost. A successful storage operation clears the ordinary busy warning, but not an unresolved sensitive-cleanup warning.

Automatic cleanup after an upstream clear is different from a user delete. TinyDash retains up to 64 pending capture identities in memory, containing only an entry ID and capture revision, not clipboard text. On macOS and Windows, the existing clipboard worker retries one pending identity per monitor wake, normally once per second, even when the clipboard is unchanged or new capture has been disabled. A clear also triggers an immediate attempt. Unrelated storage operations need not perform cleanup; their success leaves its warning visible until cleanup resolves.

Each attempt checks the current capture revision and pin state in the same SQLite write transaction as deletion. A later capture of the same text supersedes the old obligation, even though deduplication preserves its entry ID and creation timestamp. Touching an entry by copying it from history changes its order, not its capture revision, and does not cancel already-pending cleanup. Pinned entries rotate to the back of the queue so other cleanup can progress. They remain pending until unpinned in both All and Clipboard, explicitly deleted, or superseded by recapture. Successful manual deletion or pruning also lets a later retry retire an absent entry; a successful full clear resolves all pending obligations immediately.

At 64 pending identities, capture pauses until cleanup makes room. Skipped values are not replayed. A defensive overflow warning also pauses capture and requires a successful **Clear all clipboard history**, including pins; ordinary storage success or **Clear unpinned** cannot dismiss it. This prevents a capacity error from silently losing a privacy obligation.

Corruption, file-access failures, backup failures other than lock contention, and incompatible schemas instead require [data recovery](../../how-to/recover-data.md) or a compatible app version; TinyDash does not replace or reopen the failed database in that session. Pending cleanup and its warning remain, but automatic attempts stop after a permanent failure. There is no shutdown cleanup guarantee: intentions, capacity pauses, and warnings are memory-only and are lost on quit or crash. A restart does not reconstruct or replay them, and saved text may remain. Resolve the warning before quitting where possible; after recovery, inspect and explicitly clear retained history.

## Secrets

TinyDash does not read or save clipboard content that its source marks as secret. Each platform has its own convention:

- macOS: the [nspasteboard.org](http://nspasteboard.org/) concealed and transient types, and Apple's `com.apple.is-sensitive` type.
- Windows: the documented [`ExcludeClipboardContentFromMonitorProcessing`](https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats#cloud-clipboard-and-clipboard-history-formats) format, `CanIncludeInClipboardHistory` set to 0, and the older `Clipboard Viewer Ignore` format.
- Linux: the `x-kde-passwordManagerHint` target.

Most password managers set one of these markers. TinyDash's password generator sets them too, so other clipboard managers also skip its copies. The markers stay on the clipboard, so a generated password is also skipped after TinyDash restarts. There is no detection of unmarked passwords.

On macOS, Apple Passwords and Keychain Access copy passwords without a marker. TinyDash skips a copy while one of these apps is frontmost. The one-second check can attribute a copy to the wrong app when you switch apps quickly.

On macOS and Windows, a source that empties the clipboard may be clearing sensitive content. Password managers do this some time after a copy. If TinyDash saved the cleared value in the previous 120 seconds, it requests automatic cleanup of that capture. Only the matching, unpinned capture can be deleted. Older history is not targeted, and TinyDash's own copy actions do not create new cleanup obligations. A blocked cleanup is retained and warned about as described under [Storage contention](#storage-contention). Linux cannot tell a clear from an application exit, so it skips this step.

macOS checks the [pasteboard change counter](https://developer.apple.com/documentation/appkit/nspasteboard/changecount). Windows checks the [clipboard sequence number](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber). Each check runs once per second and when the launcher opens. Text is read only when the counter changes. Rapid copies within one interval can be missed. Linux uses GTK [owner-change events](https://docs.gtk.org/gtk3/signal.Clipboard.owner-change.html) and asynchronous text requests, with no polling timer. Wayland can restrict background access; opening TinyDash requests the current clipboard again. Desktop session checks remain necessary for Wayland.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/settings.spec.ts tests/tinycast-features.spec.ts --grep "clipboard|Clipboard|copy|Copy|history|first-use"
```

For affected backend behavior:

```sh
bun run test:rust -- clipboard
bun run test:rust -- launcher::storage::
bun run test:rust -- db::
```

Also run `bun run verify:browser tests/rich-clipboard.spec.ts tests/roadmap.spec.ts`. In a disposable macOS profile, opt into each rich format separately, capture a PNG and a Finder file list, restart, preview, copy, delete, and compare native clipboard contents. Test disabled capture, exclusions, secret markers, limits, missing referenced files, and retention. Windows/Linux unsupported rich-format messages are not proof of native support.

The storage session tests use a second real SQLite connection holding `BEGIN IMMEDIATE`. They check that failed captures and deletes leave search available, failed clears preserve entries and pins, and capture, deletion, warnings, and pending usage recover after unlocking. They also cover a busy startup merging session-only usage exactly once. `launcher::storage::cleanup_tests` exercises the production observation/automatic-cleanup path: a blocked upstream clear, unrelated successful storage with the privacy warning still pending, a quiet retry, pins in both categories, and same-ID recapture. It also covers queue capacity/overflow, permanent failures, manual-delete isolation, and the memory-only restart limit. Database tests check migration from schema 5 to 6, revision overflow, unchanged creation timestamps, order reuse, touches, and revision rechecks across connections. These are Rust session-layer checks, not proof of Tauri events or system clipboard integration.

Start with a fresh capture choice. Enable capture, copy known multiline text from another process, find it in Clipboard and All, and copy it back. Compare exact bytes. Delete an entry and confirm the system clipboard stays unchanged. Check clear, pin, edit, and combine paths when affected.

For the upstream-clear path, use Windows or macOS with synthetic text: capture it from another process, hold an external SQLite `BEGIN IMMEDIATE`, empty the system clipboard, and confirm the sensitive-cleanup warning and saved row remain. Release the lock without copying anything else; the quiet monitor must finish cleanup and clear the warning. Repeat with the row pinned, then unpin both categories; repeat with a later same-text capture and ensure the earlier obligation does not delete its new revision. Linux has no upstream-clear heuristic and cannot prove this path.

Run `bun run verify:native` on Windows and Linux X11. The suite holds an external SQLite write transaction, copies and attempts deletion through the launcher, checks the failure remains visible, then releases the lock and retries without restarting. Fresh database connections verify durable deletion, the retained usage increment, and resumed capture. On Windows, the suite also empties all OS clipboard formats while the database is locked, checks the pending privacy warning and retained row, then verifies automatic deletion and warning recovery after unlock without changing the empty clipboard or restarting. This Windows journey does not establish macOS integration or native pin/recapture race coverage; those policy cases have separate Rust tests. `tests/storage-contention.spec.ts` separately checks that the test fixture releases its lock on failure; it is not desktop proof. Use the clipboard desktop checks on macOS and Wayland, and for restart persistence or native Save dialogs. Use disposable history; a settings backup cannot restore deleted history.

Enable history in an isolated profile, copy known text from another process, find it, copy an older entry, then delete it. Verify copied bytes and confirm deletion leaves the system clipboard unchanged.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tinycast-features.spec.ts](../../../tests/tinycast-features.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Native checks replace clipboard contents. Never run them against a normal Windows profile. Settings backup alone does not restore clipboard history.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
