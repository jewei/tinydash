# File search

TinyDash scans Desktop, Documents, and Downloads after the launcher opens. It uses the OS folder locations, including Windows Known Folders and Linux XDG user directories. Missing default folders are skipped. Select Files to search file and folder names and paths, or search in All mode. Press Enter to open a file with its default application, or a folder in Finder, File Explorer, or the default file manager. Command/Ctrl + Enter shows the item in its parent folder.

The index contains regular files and folders under the roots. The roots themselves are not results. By default it excludes dot files, hidden files, Windows system files, symbolic links, and the `node_modules` and `target` folders. **Include hidden files** opts into hidden entries; symbolic links and symlink ancestors remain rejected. Ignore patterns match a basename or root-relative path, using `*`, `?`, `**`, or a trailing `/` for folders. Use forward slashes; negation and character classes are not supported. Patterns are limited to 64 entries of 256 bytes each. Hidden and excluded folders are not traversed. Configured roots must be folders and cannot be symbolic links. Overlapping roots are scanned once. Paths that cannot be represented as UTF-8 are skipped. Access errors appear as a scan warning; other folders remain searchable. The index does not read file contents. Selecting a result can request a separate bounded preview outside the search lock.

The default limit is 50,000 files and folders together. A scan also stops after visiting 500,000 entries, including folders. The UI reports a limit when the index is incomplete. Use smaller roots if a large directory reaches a limit. Scans and index preparation run on a background worker. The previous index remains searchable until the new scan finishes. Search uses memory, returns at most 30 results, and applies usage scores before limiting file results.

File matching uses Unicode NFC so composed and decomposed accents match. The small [unicode-normalization crate](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/) supplies canonical normalization. This fixes accented filenames returned by macOS. Display text, file IDs, and open actions keep the original path.

Operating system notifications update the file index after creation, renaming, moving, and deletion. A single worker combines a burst of changes, waits for 300 ms of quiet, and starts a scan within two seconds of continuous changes. It keeps one pending request during a scan. There is no folder polling timer. Content-write and access events do not require filename indexing.

The [notify](https://docs.rs/notify/8.2.0/notify/) watcher uses FSEvents on macOS, ReadDirectoryChangesW on Windows, and inotify on Linux. Linux watches only folders accepted by the scanner, up to 8192 folders, so excluded trees do not consume recursive watches. Parent watches detect a removed or recreated search root. Registration limits or failures produce a warning. Network filesystems and restricted folders can omit events. **Refresh files** in the actions or tray menu remains available; Command/Ctrl + R refreshes files in Files mode. Set `fileWatchEnabled` to `false` for manual updates. File contents and file metadata are not stored in SQLite.

## Previews and actions

The detail pane shows size, modification time, and a bounded UTF-8 text or raster-image preview. Text is rendered inertly, never as HTML; unsupported, oversized, missing, or changed files report a limitation. Preview requests are serialized and stale selections are discarded. The text cap is 64 KiB; image previews have byte and dimension bounds.

File actions include **Copy Path**, **Open With** using an indexed application, **Open in Terminal**, **Quick Look**, **Copy File**, and **Move to Trash**. Trash requires explicit confirmation in both UI and Rust. File IDs and paths are revalidated before operations. Copy File and Quick Look currently require macOS. Open With supports macOS/Linux; Windows reports unsupported. Terminal opening requires Windows Terminal on Windows or `x-terminal-emulator` on Linux. Copy Path works without these tools. Native OS effects are not proved by browser mocks.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts --grep "file|scan"
bun run verify:browser tests/file-actions.spec.ts tests/roadmap.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- providers::files
bun run test:rust -- launcher::file_actions
```

Use Files and All with an isolated root. Create, rename, and delete a file without manual refresh. Confirm the visible result changes. Open a selected file and check the handler marker. Exercise Refresh files and Ctrl/Command + R when manual refresh changes.

Run `bun run verify:native` on Windows and Linux X11. Run the file-search desktop checks on macOS for FSEvents, permissions, and OS associations, and on Wayland when session behavior changes. A mocked result update does not prove filesystem notifications.

Create, rename, and delete files in the isolated test root. Confirm the result list follows each change. Open a result and check the temporary handler marker.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

File contents are not searched. Test previews with disposable text and raster files, and test every action against an identified desktop build. Cancel Trash first; use only disposable files for its confirmed path. Confirm hidden-file changes update automatically when inclusion is enabled. Browser tests cannot prove file watching, clipboard file formats, Trash, or OS associations.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
