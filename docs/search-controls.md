# Search and control guide

TinyDash opens with the global shortcut `Control+Shift+Space` on macOS, Windows, and X11 Linux. On macOS, use Control for this shortcut. Command+Shift+Space belongs to Siri Visual Intelligence on current macOS versions.

From the search field:

| Control                  | Action                                                          |
| ------------------------ | --------------------------------------------------------------- |
| Arrow Up / Arrow Down    | Move through results                                            |
| Enter                    | Run the selected result's primary action                        |
| Command/Ctrl + Enter     | Reveal a result when supported                                  |
| Escape                   | Hide the launcher; close the actions menu first when it is open |
| Tab / Shift+Tab          | Move to the next or previous visible category                   |
| Option/Alt + Left/Right  | Move between categories                                         |
| Command/Ctrl + K         | Open and filter the actions menu                                |
| Command/Ctrl + 1 to 9    | Run the primary action for a visible result                     |
| Command/Ctrl + Backspace | Delete the selected entry when supported                        |
| Command/Ctrl + Comma     | Open Settings                                                   |
| Command/Ctrl + R         | Refresh the current data source                                 |
| Command/Ctrl + Q         | Quit TinyDash                                                   |

The application uses Command on macOS and Control on Windows or Linux for the Command/Ctrl controls. Emoji mode supports arrow navigation in a grid. The category bar also accepts mouse clicks.

Category shortcuts are set in Settings. Each shortcut needs a modifier and a key. Shortcuts must be unique, and a category must be visible before it can have a shortcut. If a shortcut conflicts with another application, TinyDash reports the conflict and keeps the previous saved settings. On Wayland, TinyDash cannot register a global shortcut. Assign a desktop or compositor shortcut to `tinydash`, or use a category command such as `tinydash --mode clipboard`.

The command line supports `--settings`, `--background`, and `--mode CATEGORY`. `--mode` accepts `all`, `apps`, `files`, `emoji`, `calculator`, `clipboard`, `system`, `password`, `timezone`, `url`, and `web`. A `--mode` launch always opens an empty search in that category. `--background` starts the resident process without opening the launcher window.

## Applications and custom searches

Settings > Search lets you add aliases or hide an application. An alias adds another search name. Hiding removes the application from normal search and actions. Remove the preference, or clear its aliases and hidden state, to restore the discovered application and its original aliases.

Custom searches use one `{query}` placeholder. Use an HTTP or HTTPS URL, for example:

```text
https://example.com/search?q={query}
```

Type the keyword and search text, then choose the result to open it. TinyDash percent-encodes spaces, Unicode, and reserved characters as one query component. The URL must contain exactly one placeholder. TinyDash rejects other schemes, credentials, a placeholder in the hostname, extra placeholders, control characters, and templates that can change the target origin.

## Clipboard history

TinyDash asks before it saves new clipboard text on a fresh installation. Choosing **Keep history off** stops new capture. Choosing **Enable history** starts capture. Saved text is local plain text and can include sensitive content. Turning history off keeps entries already saved.

The history limit is 1 to 500 unpinned entries. Pinned entries do not count toward removal by the limit. Each entry is limited to 16,384 bytes. Copying an existing entry updates its use order. **Edit a copy** puts the changed text on the system clipboard. It does not create a new history entry or change the original. **Copy selected entries** preserves the selected order and the exact text, with a chosen separator. It accepts up to 100 entries and the combined output is limited to 16,384 bytes. **Save text as a file** uses a native Save dialog and writes the complete text. Canceling the dialog does not change history or the system clipboard.

**Clear unpinned history** removes only entries with no pin in All or Clipboard. **Clear all clipboard history** removes every saved entry, including pinned entries. Both actions leave the current system clipboard unchanged.

## Settings portability and recovery

Settings export writes a version 1 JSON envelope with the known settings and the selected appearance. Import first opens a preview. It validates the version, types, settings limits, and file roots, and reports unknown keys. Preview does not change settings. Export and clipboard file saving refuse the live `settings.json`, database, and `recovery.tar` paths.

TinyDash creates `recovery.tar` automatically before a database migration. Use [data-recovery.md](data-recovery.md) for manual recovery. TinyDash does not restore files automatically.
