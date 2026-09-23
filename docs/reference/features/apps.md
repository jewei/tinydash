# Application search

Select Apps to browse installed applications, or type an application name in All. Names, aliases, and paths can match. Enter opens the selected application. Command/Ctrl + Enter reveals its location. Actions and the tray menu can refresh discovery.

Settings, Search adds aliases and hides applications. App icons and descriptions depend on platform metadata. A fallback appears when an icon or description is missing. New installations appear after refresh or restart.

On macOS, search returns icon keys. The launcher loads icons for visible results at the required display size. Text results and keyboard selection remain available while icons load. Hiding the launcher releases its image subscriptions and cancels pending requests. Rust limits the icon cache and pending work. Windows and Linux keep their existing icon behavior.

See [platform support](../platform-support.md) for the directories and application formats that each operating system discovers.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/settings.spec.ts
bun run verify:browser tests/performance-regressions.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- providers::apps
bun run test:rust -- launcher::icons
```

Use Apps and All. Find an application by name, abbreviation, and alias. Press Enter on the selected fixture and check its marker. Verify reveal and refresh through each changed entry point. A hidden application must remain hidden after refresh and restart.

Run `bun run verify:native` on Windows and Linux X11 for real discovery and launch. Use the application and Settings desktop checks for reveal, tray refresh, aliases, and macOS. Platform discovery changes need a check on the affected OS.

Search for a temporary application fixture, select it, and press Enter. Check both the selected UI result and the marker written by the launched process.

On macOS, check icons in the result list and preview. Change the query and selection while icons load. Hide and reopen the launcher, then refresh applications. Confirm that current icons appear and stale images do not replace them. Native memory and response-time claims need a comparison against the baseline build; browser tests and Windows or Linux native checks do not prove a macOS performance benefit.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Browser tests mock discovery and execution. Only native checks prove that the OS launched the application.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
