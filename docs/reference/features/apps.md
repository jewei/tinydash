# Application search

Select Apps to browse installed applications, or type an application name in All. Names, aliases, and paths can match. Enter opens the selected application. Command/Ctrl + Enter reveals its location. Actions and the tray menu can refresh discovery.

Settings, Search adds aliases and hides applications. App icons and descriptions depend on platform metadata. A fallback appears when an icon or description is missing. New and removed applications update automatically, usually within a few seconds after an installer finishes. Actions and the tray menu can still refresh discovery.

On macOS and Windows, TinyDash watches the application folders. It compares each changed bundle or shortcut with the index and scans again only when an application was added, removed, or renamed. An app update or launch does not start a scan. A new or removed folder in these locations also starts a scan. On Linux, GIO reports changes to desktop entries in all XDG data folders, including Flatpak and Snap exports.

See [platform support](../platform-support.md) for the directories and application formats that each operating system discovers.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/settings.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- providers::apps
```

Use Apps and All. Find an application by name, abbreviation, and alias. Press Enter on the selected fixture and check its marker. Verify reveal and refresh through each changed entry point. A hidden application must remain hidden after refresh and restart.

Run `bun run verify:native` on Windows and Linux X11 for real discovery and launch. Use the application and Settings desktop checks for reveal, tray refresh, aliases, and macOS. Platform discovery changes need a check on the affected OS.

Search for a temporary application fixture, select it, and press Enter. Check both the selected UI result and the marker written by the launched process.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Browser tests mock discovery and execution. Only native checks prove that the OS launched the application.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
