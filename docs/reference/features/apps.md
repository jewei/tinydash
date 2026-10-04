# Application search

Select Apps to browse installed applications, or type an application name in All. Names, aliases, and paths can match. Enter opens the selected application. Command/Ctrl + Enter reveals its location. Actions and the tray menu can refresh discovery.

Settings, Search adds aliases and hides applications. Item shortcuts and aliases also assigns global app hotkeys. Hiding an item removes it from search but keeps an explicitly assigned hotkey; disabling the item also rejects its actions and stops registering its shortcut. App icons and descriptions depend on platform metadata. A fallback appears when an icon or description is missing. New and removed applications update automatically, usually within a few seconds after an installer finishes. Actions and the tray menu can still refresh discovery.

On macOS and Windows, TinyDash watches the application folders. It compares each changed bundle or shortcut with the index and scans again only when an application was added, removed, or renamed. An app update or launch does not start a scan. Filesystem metadata is observed outside the search mutex; only confirmed absence is treated as removal, not a permission or other metadata error. A new or removed folder in these locations also starts a scan. On Linux, GIO reports changes to desktop entries in all XDG data folders, including Flatpak and Snap exports.

Settings loads the app catalog on a blocking worker, so a competing search does not block the event thread. Settings publication waits for search before taking its brief settings write lock; launcher shortcuts, blur handling, and clipboard callbacks can still read the previous settings while publication waits. Rust contention regressions cover these lock paths; they are not native responsiveness measurements.

On macOS, search returns icon keys. The launcher loads icons for visible results at a size bucket that meets the required physical resolution. Nearby row and preview sizes share one payload. Text results and keyboard selection remain available while icons load. Hiding the launcher releases its image subscriptions and cancels pending requests. A frontend cache retains up to 64 completed icon payloads and 512 KiB of accounted string data for reuse. This limit excludes active payloads and decoded images. Rust limits its icon cache and pending work. Windows and Linux keep their existing icon behavior.

See [platform support](../platform-support.md) for the directories and application formats that each operating system discovers.

Visible native icons can start loading when they mount. A shared observer defers clipped icons until scrolling makes them visible. CSS tokens define row and preview sizes. Compact mode, window resizing, and display-scale changes update the required resolution. The launcher groups layout reads before updating images. It shares its display-scale listener and disconnects display observers when no active native icon needs them. Later results reuse the disconnected observers without retaining old targets. Static images and fallback icons do not register native loading observers. Fallback icons construct only the selected SVG shape. A name change replaces the shape and keeps the outer SVG element. Native avatars check the warm cache before mounting a fallback shape.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/settings.spec.ts
bun run verify:browser tests/app-icons.spec.ts tests/app-icon-cache.spec.ts tests/icon-display.spec.ts tests/icons.spec.ts tests/performance-regressions.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- providers::apps
bun run test:rust -- launcher::icons
```

On macOS or Windows, also run the application-folder watcher regressions:

```sh
bun run test:rust -- launcher::app_watch::tests
```

These cover installs, removals, renames, metadata failures, and bounded event coalescing. A controlled metadata-probe latch checks that an actual `SearchManager` search completes while filesystem observation is blocked. They do not establish native watcher delivery or desktop latency.

Use Apps and All. Find an application by name, abbreviation, and alias. Press Enter on the selected fixture and check its marker. Verify reveal and refresh through each changed entry point. A hidden application must remain hidden after refresh and restart.

Run `bun run verify:native` on Windows and Linux X11 for real discovery and launch. Use the application and Settings desktop checks for reveal, tray refresh, aliases, and macOS. Platform discovery changes need a check on the affected OS.

Search for a temporary application fixture, select it, and press Enter. Check both the selected UI result and the marker written by the launched process.

On macOS, check icons in the result list and preview. Change the query and selection while icons load. Hide and reopen the launcher, then refresh applications. Confirm that current icons appear and stale images do not replace them. Native memory and response-time claims need a comparison against the baseline build; browser tests and Windows or Linux native checks do not prove a macOS performance benefit.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Browser tests mock discovery and execution. Only native checks prove that the OS launched the application.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
