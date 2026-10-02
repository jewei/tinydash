# Clipboard privacy and rich formats

[Text clipboard history](clipboard.md) remains separate from saved images and file references. Its first-use consent still controls all capture. Image and file capture each require an additional Settings opt-in; both default off.

## Exclusions and retention

Settings accepts application identities or names to exclude from capture. Matching is exact and ASCII case-insensitive, not a substring or wildcard:

- macOS: bundle identifier or application display name. Attribution uses the frontmost application at the polling check, so switching applications quickly can misattribute a copy.
- Windows: clipboard-owner executable path or executable filename, including `.exe`.
- Linux: source identity is unavailable. Configuring any exclusion suspends clipboard capture rather than silently recording an excluded source. An unknown source on macOS or Windows is also skipped when exclusions are configured.

Exclusions affect future external captures, not previously saved entries or TinyDash's explicit text copy actions. Secret-marked clipboard contents remain excluded regardless of settings.

Retention days defaults to zero, meaning no age expiry. A positive value expires unpinned text using its original capture time, not its last use. Re-copying retained text does not renew that time. Text pins remain exempt from age and count pruning. Retention is applied at startup, when settings change, during captures, and periodically while the monitor runs. Expired text re-captured after removal is a new entry.

## Image and file history

The **Clipboard images and files** command opens a separate history view. Select an entry to preview it. **Copy original format** writes the saved image or file references to the native clipboard; paste them in the target application. **Delete saved entry** removes history without changing the system clipboard. The existing clear-history actions also clear rich entries.

Native rich capture and copy currently support **macOS only**. Windows and Linux display an explicit unsupported notice; their text history remains available.

Supported macOS formats are native PNG and Finder-compatible file lists (`NSFilenamesPboardType`). TIFF-only images, animated PNGs, URL-only file sources, and arbitrary text that happens to look like a file path are not captured as images or files. Files are references, not backups: their contents are never saved. A file must exist when captured, previewed, and copied. Moving or deleting a referenced file makes that entry unavailable for preview and copy.

Limits:

- PNG: 4 MiB encoded, at most 4,194,304 pixels, and at most 4,096 pixels per edge.
- Files: up to 64 existing absolute paths, 32 KiB of path text, and 64 KiB encoded metadata.
- Rich history: up to 32 entries and 16 MiB of payloads. A lower text-history count setting also lowers this count.

Rich entries do not support pins, text editing, or text combining. They use the same age-retention setting.

## Local storage

Rich history is stored in `clipboard-rich.sqlite3`, separately from the existing text database. No existing text rows are converted or removed by a rich-format migration. Payloads are SQLite blobs, so pruning removes payloads along with metadata rather than leaving orphan files. SQLite secure deletion is enabled, and Unix permissions restrict the file to its owner. History is not encrypted. Backups and filesystem snapshots may retain earlier data.

## Verification

```sh
bun run test:rust -- clipboard
bun run verify:browser tests/rich-clipboard.spec.ts
```

Rust tests cover age retention, pin preservation, expired-text recapture, rollback, exclusion matching, rich persistence, format validation, byte/count bounds, missing references, and newer-schema refusal. Browser tests mount the real rich-history component with mocked IPC and exercise preview, copy failures, deletion, and return navigation. They do not establish native clipboard effects or launcher command routing.

Session tests combine age retention with storage contention and pending sensitive cleanup. An unsuccessful capture keeps the previous entry and cleanup identity. A later successful capture can replace expired text with a new entry; retrying cleanup for the old identity cannot remove the new capture.

Native proof requires an isolated macOS desktop session and an identified build. Enable capture and each rich opt-in separately; copy a small native PNG and files from another application, inspect previews, copy back, and verify the target receives an image or file references rather than text. Test a missing file, excluded source, secret markers, oversized image, restart persistence, retention, and clear. Do not use a personal clipboard for this check. Windows/Linux exclusion behavior needs separate platform checks; rich formats remain unsupported there.

Follow the [verification procedure](../../how-to/verify.md). No live clipboard changes are made by the focused unit or component tests.
