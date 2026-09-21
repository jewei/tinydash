# Launcher and navigation

Open TinyDash with Control + Shift + Space, the tray menu, or the command line. The window stays resident after it hides. Drag the handle above the search field to move it. Window position lasts until the process exits.

All searches applications, files, clipboard text, emoji, calculations, system commands, and explicit tool commands. Empty All shows welcome examples or pinned items. Text categories have a fixed priority before the 30-result limit. See [ranking](../../explanation/data-and-privacy.md).

Tab and Shift + Tab change categories while preserving the query and search focus. Arrow keys select results. Enter runs the selected action. Escape closes a dialog or menu before hiding the launcher. [Keyboard reference](../keyboard-shortcuts.md) lists the remaining controls.

The actions menu uses Command/Ctrl + K. The tray can open the launcher, open Settings, refresh data, and quit. A second app launch shows the existing process.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/welcome.spec.ts tests/categories.spec.ts tests/launcher.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- launcher
```

Open with the global shortcut, tray, and a second process when those entry points change. Check focus, arrow selection, Enter, Escape, and reopen. The selected fixture must launch and a second launcher process must not remain.

Run `bun run verify:native` on Windows and Linux X11 for launch and reopen. Use the desktop launcher checks for macOS, focus return, tray, physical shortcuts, input methods, and Wayland. The native suite does not prove those additional paths.

Open All with no pins. The welcome controls appear and the search field has focus. Choose Apps by keyboard, select a result, and confirm that Enter launches the selected fixture application.

Tests: [tests/welcome.spec.ts](../../../tests/welcome.spec.ts), [tests/categories.spec.ts](../../../tests/categories.spec.ts), [tests/launcher.spec.ts](../../../tests/launcher.spec.ts).

Native tests check launch and reopen on Windows and Linux. Global shortcuts, focus, tray behavior, and Wayland need desktop checks.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
