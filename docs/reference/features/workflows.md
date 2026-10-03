# Quicklinks, snippets, and direct paste

Open **Actions → Quicklinks and snippets**, or search for **Quicklinks** or **Snippets** in All or System. Saved item names and keywords also appear in All. Selecting a snippet without `{clipboard}` inserts it into the previous app. Its assigned item shortcut inserts it into the active app without opening the launcher. Snippets with `{clipboard}` open the item for consent. Quicklinks open the item so arguments remain explicit. Use the library to preview, edit, or copy a snippet without inserting it.

Create, edit, search, or delete quicklinks and snippets in the library. Unsaved edits require confirmation before navigation; deleting an item also requires confirmation. While a native view is open, shortcuts that open another view or request a system-action confirmation ask you to close it first, preserving drafts. Data is stored locally in `library.json` in the application data directory. It is plaintext, not a credential vault, and is not included in settings exports. Writes replace the file atomically. A damaged library is preserved and reported without preventing the rest of TinyDash from starting.

Creating or editing an item puts keyboard focus in Name. Confirmation dialogs start on the cancel action and keep keyboard focus inside the dialog. Escape cancels the confirmation. Focus returns to the control that opened it, or to Search library when that control is removed. The library shows loading, empty-library, and no-match states separately. Retry loading library repeats a failed list request without changing an editor draft.

An empty library explains quicklinks and snippets and directs users to **New quicklink** or **New snippet**. Selected items use the shared theme's selection colors.

Quicklinks support HTTP(S), local file/folder paths, and a restricted set of application URL schemes. Unsupported or executable schemes are rejected. Use `{query}` or `{argument:name}` for explicit input fields. URL arguments are encoded as components and cannot change the destination origin. There is no arbitrary shell execution or custom application routing in the library.

Snippets support literal text, `{date}`, `{time}`, `{clipboard}`, and named argument placeholders. Clipboard interpolation requires consent for each invocation; previews never read the clipboard. Copy leaves the library open. Paste inserts into the previous application where the OS permits it. Keywords are search terms: they do not monitor typing or automatically expand in other applications.

The library is limited to 256 items, 32 KiB per template, 16 arguments, 64 KiB rendered output, and a 2 MiB saved document. Search indexes names/keywords rather than snippet bodies. No network, clipboard watcher, or keyboard listener runs on behalf of the library.

## Direct paste

For a copyable result, choose **Paste to previous app** in the detail pane or Actions, or press **Command/Ctrl + Shift + Enter**. Enter retains the existing Copy behavior. The backend resolves the result ID, writes secret-marked clipboard text, dismisses the launcher, restores the captured target, verifies focus, and sends a paste chord. It never accepts frontend-supplied executable paths or clipboard content through the result action.

- macOS requires Accessibility permission. A closed previous application is rejected.
- Windows waits up to 750 ms for shortcut modifiers to be released before insertion. Foreground restrictions and elevated applications may block insertion.
- Linux X11 requires `xdotool`; native Wayland reports unsupported. Use Copy and paste manually there.

Failures explain whether to grant permission or paste manually. Once clipboard writing succeeds, failed insertion can leave the copied content on the clipboard. TinyDash does not restore old clipboard contents before the target consumes the new value. Direct paste is an explicit action, not a guarantee that every third-party text field accepts simulated keys.

## Verification

Run `bun run test:rust -- direct_insertion` for shortcut eligibility and clipboard-consent rules. The existing library tests cover explicit clipboard consent and paste errors. Desktop proof must also bind an item shortcut to a disposable snippet containing `{date}` and `{time}`, insert it into an editable fixture, and compare the exact bytes. Repeat from an All result. A clipboard-dependent snippet must show consent on each invocation; a disabled snippet must not insert. Check missing Accessibility permission, a closed target, changed focus, and Wayland rejection. Native global shortcuts and insertion require separate desktop evidence.

```sh
bun run verify:browser tests/library.spec.ts tests/roadmap.spec.ts
bun run test:rust -- launcher::library
bun run test:rust -- launcher::roadmap_tests
```

Browser tests cover library CRUD, encoded arguments, per-invocation clipboard consent, copy/paste dispatch, error recovery, unsaved-edit confirmation, dialog and editor focus, loading and retry states, launcher routes, and result-ID-only paste. Rust tests cover bounds, unsafe targets, persistence, unavailable storage, template parsing, and consent. Browser IPC is mocked.

Desktop proof requires a controlled profile and identified build: open TinyDash from an editable fixture, paste exact text/emoji/calculation results, and verify both destination and bytes. Deny Accessibility, close the target, change focus during the action, and exercise Wayland rejection. Use only disposable clipboard contents and URLs/files. Verify persistence after restart and confirm corrupt-library recovery leaves the original file unchanged. See [verification](../../how-to/verify.md).
