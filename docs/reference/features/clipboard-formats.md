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

The **Clipboard images and files** command opens a separate history view. Select an entry to preview it. **Copy original format** writes the saved image or file references to the native clipboard; paste them in the target application. **Delete saved entry** removes history without changing the system clipboard. **Pin saved entry** keeps an image or file reference during automatic cleanup and **Clear unpinned**. Pins appear first and persist across restart. **Unpin saved entry** applies the current retention and count policy immediately; an expired entry can disappear. **Clear all clipboard history** removes rich entries including pins. Pinning does not change the system clipboard.

Native rich capture, copy, and direct paste currently support **macOS only**. Windows and Linux display an explicit unsupported notice; their text history remains available.

Supported macOS formats are native PNG and Finder-compatible file lists (`NSFilenamesPboardType`). TIFF-only images, animated PNGs, URL-only file sources, and arbitrary text that happens to look like a file path are not captured as images or files. Files are references, not backups: their contents are never saved. A file must exist when captured, previewed, and copied. Moving or deleting a referenced file makes that entry unavailable for preview and copy.

Limits:

- PNG: 4 MiB encoded, at most 4,194,304 pixels, and at most 4,096 pixels per edge.
- Files: up to 64 existing absolute paths, 32 KiB of path text, and 64 KiB encoded metadata.
- Rich history: up to 32 entries and 16 MiB of payloads. A lower text-history count setting lowers the capture limit. Cleanup never removes existing pins, even if they exceed that lower limit. Pins still count toward the hard 32-entry and 16 MiB bounds.

New captures replace the oldest unpinned entries. When pins leave too little room, the new capture is skipped and pinned payloads remain intact. The panel shows storage use, pin count, and whether pins fill the capture limit. Unpin or delete entries to make room. Unpinned rich entries use the same age-retention setting as text. Rich entries do not support text editing or text combining.

## Custom names

Select a pinned image or file list, then choose **Name pinned entry**. Enter a name such as “Company logo” or “Release files” and choose **Save name**. Use **Rename pinned entry** to change it later. **Restore original title**, or a blank name, removes the custom name. **Cancel naming** or Escape closes the editor without a write. Enter saves from the name field, except during input-method composition or repeated key events. A failed write keeps the draft for retry.

Names are stored separately from the original title. They do not change PNG bytes, file paths, capture times, or clipboard contents. Search matches both the custom name and original title. The view shows the original title below a named entry. A name survives restart and repeat capture. Unpinning keeps the name while the entry remains in history, but only pinned entries can be renamed. Missing file references can still be named. Duplicate names are allowed.

Names are trimmed and normalized to Unicode NFC. The name field accepts up to 120 UTF-16 code units. The backend permits up to 120 Unicode scalar values and 480 UTF-8 bytes, rejects control characters, and limits each input to 4 KiB before normalization. Names share the history's local, unencrypted storage.

## Search and filters

Use **Search history** to find custom names, saved titles, filenames, paths, or source app identifiers. Matching ignores letter case and uses Unicode NFC normalization. Space-separated terms must all match the entry; terms can match different fields. Characters such as `%` and `_` are literal, not wildcards. Search does not inspect image pixels or file contents.

Use **Content type** to select All types, Images, or Files. **Source app** selects an exact saved app identifier, such as `com.apple.finder`. The source list includes apps from the full retained history, so it stays available when a search has no matches. Entries with unknown sources appear under All apps. Source attribution has the platform limits described above.

The view shows the matching count and total history count. Pins remain first, and the current entry stays selected if it still matches. Otherwise, the first match is selected. **Clear filters** restores the full list. Filters stay until cleared or the view is closed. Refresh, pinning, deletion, and new capture retain the active filters. A failed search keeps its filters and offers Refresh; actions stay disabled until the current result is available.

Rust filters metadata and reads bounded saved file lists only when needed. It does not load image blobs or check file existence during search. Missing file references therefore remain findable for unpinning or deletion; preview, copy, and paste still recheck file availability. Search input is limited to 256 characters in the view and 1 KiB of UTF-8 through IPC. Storage limits and retention do not change.

## Keyboard controls

With focus in **Search history** or the result list, use **Up** and **Down** to select the previous or next matching entry. Selection stops at either end. The view scrolls the selected entry into view and updates its preview. Search keeps focus while navigating from the search field. Tab can move to the selected list option; arrow keys then move focus with selection. The search field exposes the selected option to assistive technology.

Press **Enter** to copy the selected entry in its original format. Press **Command + Shift + Enter** on macOS, or **Ctrl + Shift + Enter** on other platforms, to request direct paste. Rich copy and paste remain available only on macOS. These shortcuts require a current preview and are blocked while history is loading, after a search error, or during an action. Holding Enter does not repeat the action. Input-method composition does not trigger copy, paste, or selection changes. Keyboard actions restore focus to their starting control after completion or failure.

The type/source menus and action buttons keep their normal keyboard behavior. Shortcuts apply only in search and results; Enter on **Pin saved entry**, for example, pins the entry. The text-history default-action setting does not change Enter in this view. Escape cannot close the view while an action is in progress.

## Paste to the previous app

On macOS, choose **Paste to previous app** in the image and file history view. Open TinyDash from the destination app first. Direct paste needs Accessibility access. The action reads the selected saved entry again, checks its format and current file references, writes the original format to the clipboard, restores the previous app, and sends Command + V. It shares the text-paste operation lock and target identity/focus checks. Copy remains available and does not need paste permission.

The destination decides whether to accept an image or file list. Successful dispatch is not a receipt confirmation. A missing file or permission error stops the action; a later focus or dispatch error restores the launcher with the history view and error available. The clipboard can already contain the saved content after a later failure. Use **Copy original format** and paste manually if needed. The text default-action setting does not affect this view.

## Local storage

Rich history is stored in `clipboard-rich.sqlite3`, separately from the existing text database. Rich schema 3 adds an optional custom name, after schema 2 added the saved pin flag. Upgrading preserves payloads, titles, pin state, and ordering. Schema 1 entries start unpinned; existing entries start without custom names. The upgrade runs in one transaction. No existing text rows are converted or removed by a rich-format migration. Payloads are SQLite blobs, so pruning removes payloads along with metadata rather than leaving orphan files. SQLite secure deletion is enabled, and Unix permissions restrict the file to its owner. History is not encrypted. Backups and filesystem snapshots may retain earlier data.

## Verification

```sh
bun run test:rust -- clipboard
bun run verify:browser tests/rich-clipboard.spec.ts
```

Rust tests cover age retention, pin preservation, expired-text recapture, rollback, exclusion matching, rich persistence and schema upgrade, pinned count/byte budgets, failed pin writes, name persistence and validation, name-write failure, original content preservation, case and Unicode matching, combined metadata/type/source filters, literal search, missing references, image-blob avoidance, query bounds, format validation, byte/count bounds, missing references, and newer-schema refusal. Browser tests mount the real rich-history component with mocked IPC and exercise preview, copy failures, image/file paste dispatch, paste failure and Copy fallback, busy controls, platform gating, search/filter combinations, empty results, stale reply rejection, search-error retry, keyboard selection and activation, platform modifiers, composition/repeat guards, pending-preview and busy guards, focus recovery, name save/search/restore, name-write retry, editor cancellation and composition guards, pin/unpin, stable selection after sorting, pin-write retry, unavailable previews, deletion, and return navigation. They do not establish native clipboard effects or launcher command routing.

Session tests combine age retention with storage contention and pending sensitive cleanup. An unsuccessful capture keeps the previous entry and cleanup identity. A later successful capture can replace expired text with a new entry; retrying cleanup for the old identity cannot remove the new capture.

Native proof requires an isolated macOS desktop session and an identified build. Enable capture and each rich opt-in separately; copy a small native PNG and files from another application, inspect previews, copy back, and verify the target receives an image or file references rather than text. Use Paste to previous app for both a PNG and multiple files in compatible disposable destinations, and verify actual received data. Repeat with denied Accessibility access, a closed target, focus changed during dispatch, two rapid requests, and a simultaneous text-queue paste. Failures must retain the selected history entry and allow Copy; no keys may reach a different app. Search by filename and source, combine type/source filters, clear them, and check that missing references remain visible for deletion. Confirm filtering does not alter stored entries or the system clipboard. Repeat selection, copy, and paste from the search field and from a focused result using the keyboard; compare exact native output. Check input-method composition, shortcut modifiers, focus after errors, and that Enter on filter/action controls keeps its normal behavior. Name a pinned image and file list, restart, search by the name and original title, then restore the title. Repeat capture and confirm the name persists without changing the exact clipboard data. Check naming with a missing reference and a storage write error. Test a missing file, excluded source, secret markers, oversized image, restart persistence, retention, and clear. Do not use a personal clipboard for this check. Windows/Linux exclusion behavior needs separate platform checks; rich formats remain unsupported there.

Follow the [verification procedure](../../how-to/verify.md). No live clipboard changes are made by the focused unit or component tests.
