# Application search

Select Apps to browse installed applications, or type an application name in All. Names, aliases, and paths can match. Enter opens the selected application. Command/Ctrl + Enter reveals its location. Actions and the tray menu can refresh discovery.

Settings, Search adds aliases and hides applications. App icons and descriptions depend on platform metadata. A fallback appears when an icon or description is missing. New installations appear after refresh or restart.

See [platform support](../platform-support.md) for the directories and application formats that each operating system discovers.

## Verification

Search for a temporary application fixture, select it, and press Enter. Check both the selected UI result and the marker written by the launched process.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Browser tests mock discovery and execution. Only native checks prove that the OS launched the application.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
