# Clipboard history

On a fresh installation, TinyDash asks before it saves clipboard text. Existing installations keep their saved capture choice. When capture is enabled, TinyDash captures text while it runs, including the current clipboard at startup. Select Clipboard mode to see recent entries. Pins stay at the top, and a fresh empty search selects the latest entry. Search saved text, use the arrow keys to inspect its full preview, and press Enter to copy it. Paste it in the target application with Command/Ctrl + V.

The default limit is 100 unpinned entries. Pins in Clipboard or All keep an entry outside this limit. Each entry can contain up to 16 KiB of UTF-8 text. Empty text, whitespace-only text, embedded null characters, images, and larger values are skipped. TinyDash preserves the accepted text exactly. Repeated consecutive values do not create writes. Copying an older value moves its existing entry to the top. Equal search scores keep the newest entries first.

Use the actions menu or Command/Ctrl + Backspace to delete the selected entry. **Clear unpinned** keeps entries pinned in All or Clipboard. The separate **Clear all clipboard history** action includes pins. Both ask for confirmation. Deleting an entry removes its pins from both categories. These actions leave the system clipboard unchanged. An unchanged clipboard is not captured again during the same session. Restarting TinyDash captures the current clipboard again when capture is enabled.

The actions menu can edit a copy, combine selected entries in a chosen order with a separator, or save the full text as a file. Edited and combined copies leave history unchanged. Editing and combining leave the original entries unchanged. Combining accepts up to 100 entries and 16,384 output bytes. Saving uses the native Save dialog.

History is local plain text in the same SQLite database as usage. It is not encrypted. On Unix, the database is restricted to its owner. SQLite secure deletion is enabled, but backups and filesystem snapshots can retain earlier data. Turn off **Save clipboard history** in Settings to stop capture. Existing history remains searchable and can be cleared.

## Secrets

TinyDash does not read or save clipboard content that its source marks as secret. Each platform has its own convention:

- macOS: the [nspasteboard.org](http://nspasteboard.org/) concealed and transient types, and Apple's `com.apple.is-sensitive` type.
- Windows: the documented [`ExcludeClipboardContentFromMonitorProcessing`](https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats#cloud-clipboard-and-clipboard-history-formats) format, `CanIncludeInClipboardHistory` set to 0, and the older `Clipboard Viewer Ignore` format.
- Linux: the `x-kde-passwordManagerHint` target.

Most password managers set one of these markers. TinyDash's password generator sets them too, so other clipboard managers also skip its copies. The markers stay on the clipboard, so a generated password is also skipped after TinyDash restarts. There is no detection of unmarked passwords.

On macOS, Apple Passwords and Keychain Access copy passwords without a marker. TinyDash skips a copy while one of these apps is frontmost. The one-second check can attribute a copy to the wrong app when you switch apps quickly.

On macOS and Windows, a source that empties the clipboard shows that the content was sensitive. Password managers do this some time after a copy. If TinyDash saved the cleared value in the previous 120 seconds, it deletes that entry. Pinned entries, copies from TinyDash, and older history stay. Linux cannot tell a clear from an application exit, so it skips this step.

macOS checks the [pasteboard change counter](https://developer.apple.com/documentation/appkit/nspasteboard/changecount). Windows checks the [clipboard sequence number](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber). Each check runs once per second and when the launcher opens. Text is read only when the counter changes. Rapid copies within one interval can be missed. Linux uses GTK [owner-change events](https://docs.gtk.org/gtk3/signal.Clipboard.owner-change.html) and asynchronous text requests, with no polling timer. Wayland can restrict background access; opening TinyDash requests the current clipboard again. Desktop session checks remain necessary for Wayland.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/settings.spec.ts tests/tinycast-features.spec.ts --grep "clipboard|Clipboard|copy|Copy|history|first-use"
```

For affected backend behavior:

```sh
bun run test:rust -- clipboard
```

Start with a fresh capture choice. Enable capture, copy known multiline text from another process, find it in Clipboard and All, and copy it back. Compare exact bytes. Delete an entry and confirm the system clipboard stays unchanged. Check clear, pin, edit, and combine paths when affected.

Run `bun run verify:native` on Windows and Linux X11. Use the clipboard desktop checks on macOS and Wayland, and for restart persistence or native Save dialogs. Use disposable history; a settings backup cannot restore deleted history.

Enable history in an isolated profile, copy known text from another process, find it, copy an older entry, then delete it. Verify copied bytes and confirm deletion leaves the system clipboard unchanged.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tinycast-features.spec.ts](../../../tests/tinycast-features.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Native checks replace clipboard contents. Never run them against a normal Windows profile. Settings backup alone does not restore clipboard history.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
