# File search

TinyDash scans Desktop, Documents, and Downloads after the launcher opens. It uses the OS folder locations, including Windows Known Folders and Linux XDG user directories. Missing default folders are skipped. Select Files to search filenames and paths, or search in All mode. Press Enter to open a file with its default application. Command/Ctrl + Enter shows the file in its folder.

The index contains regular files only. It excludes dot files, hidden files, Windows system files, symbolic links, and the `node_modules` and `target` folders. Hidden and excluded folders are not traversed. Configured roots must be folders and cannot be symbolic links. Overlapping roots are scanned once. Paths that cannot be represented as UTF-8 are skipped. Access errors appear as a scan warning; other folders remain searchable. TinyDash does not read file contents.

The default limit is 50,000 files. A scan also stops after visiting 500,000 entries, including folders. The UI reports a limit when the index is incomplete. Use smaller roots if a large directory reaches a limit. Scans and index preparation run on a background worker. The previous index remains searchable until the new scan finishes. Search uses memory, returns at most 30 results, and applies usage scores before limiting file results.

File matching uses Unicode NFC so composed and decomposed accents match. The small [unicode-normalization crate](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/) supplies canonical normalization. This fixes accented filenames returned by macOS. Display text, file IDs, and open actions keep the original path.

Operating system notifications update the file index after creation, renaming, moving, and deletion. A single worker combines a burst of changes, waits for 300 ms of quiet, and starts a scan within two seconds of continuous changes. It keeps one pending request during a scan. There is no folder polling timer. Content-write and access events do not require filename indexing.

The [notify](https://docs.rs/notify/8.2.0/notify/) watcher uses FSEvents on macOS, ReadDirectoryChangesW on Windows, and inotify on Linux. Linux watches only folders accepted by the scanner, up to 8192 folders, so excluded trees do not consume recursive watches. Parent watches detect a removed or recreated search root. Registration limits or failures produce a warning. Network filesystems and restricted folders can omit events. **Refresh files** in the actions or tray menu remains available; Command/Ctrl + R refreshes files in Files mode. Set `fileWatchEnabled` to `false` for manual updates. File contents and file metadata are not stored in SQLite.

## Verification

Create, rename, and delete files in the isolated test root. Confirm the result list follows each change. Open a result and check the temporary handler marker.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

File contents are not searched. Browser tests cannot prove file watching or OS associations.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
