# Appearance and categories

Choose Light, Dark, Sage, Rose, or Ink in Actions, Appearance, or in Settings, Appearance. Each theme has its own background, text, selection, and accent colors. Compact is a separate layout option. It uses shorter rows in one column with any theme. Both choices apply immediately, stay in sync between windows, and persist after restart. An older Compact choice becomes Light with Compact enabled.

In Settings, Appearance, **Follow macOS Liquid Glass** is on by default. Turn it off to use the selected theme’s solid background and ignore changes to the system glass setting. The choice applies immediately, stays in sync between windows, persists after restart, and is included in settings exports. Imports without this preference keep the current choice. This switch appears only on macOS and does not change any system settings.

With the switch on, macOS 26 or later uses an AppKit `NSGlassEffectView`. macOS draws the blur and transparency. On macOS 27, use the Liquid Glass slider in System Settings, Appearance. The native view follows the system's glass and accessibility settings, including Reduce transparency. TinyDash does not store a separate blur value. Menus and dialogs retain solid backgrounds. Earlier macOS versions, Windows, Linux, and browser previews use solid theme backgrounds. A native setup failure also keeps the solid background.

The native view uses the regular glass style and matches the selected theme's light or dark appearance. Its content is clipped to the launcher's 20-point corner radius, including after glass is turned off. It does not set a custom glass tint or opacity. See [Apple's Liquid Glass updates](https://developer.apple.com/videos/play/wwdc2026/102/) and [NSGlassEffectView](https://developer.apple.com/documentation/appkit/nsglasseffectview).

Ink uses a monochrome palette with off-white backgrounds, black controls, stronger borders, and bold sans-serif headings. Selected results use white text on black. The native glass can still show colors from the desktop behind the launcher.

The app bundles Figtree and Caprasimo with their license files. It does not need a font service. The desktop window is 980 by 620. The launcher has rounded corners and a native shadow, with no added outer border in any theme. At narrow widths, the result list uses the full width and clipboard previews appear below it.

Settings, Categories controls visible categories. Keep at least one selected. The category bar scrolls when required. Hiding a category does not remove its results from All.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/categories.spec.ts tests/settings.spec.ts tests/tools.spec.ts tests/welcome.spec.ts tests/tinycast-features.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- settings
bun run test:rust -- launcher::portability
```

Change all five themes through Actions and Settings. Toggle Compact without changing the theme. Inspect 320, 375, 414, 768, and 980-pixel widths. Reload and confirm both choices persist. Import and export both values, and check the older Compact appearance migration. Save visible categories, reopen, and check Tab and Shift + Tab use only those categories. Verify the selected fallback when hiding the active category.

Use desktop launcher and Settings checks on each affected OS for transparent corners, multiple windows, persistence after process restart, scaling, and placement. Browser viewport checks cannot establish native window behavior.

On macOS 27, turn Follow macOS Liquid Glass off and on in Settings. Confirm the launcher changes immediately between a solid theme background and native glass. Restart with the switch off and confirm the choice persists. With the switch off, change the system glass slider and confirm the background stays solid. Restore the system value, then enable the switch. Change the Liquid Glass slider while the launcher is visible over a patterned test window. Confirm the background changes without a restart. Restore the system value. Repeat with Reduce transparency and restore it. Check all themes, Compact, menus, dialogs, typing, dragging, hide/reopen, and restart. Check the solid fallback on an older macOS version. Browser tests only prove the frontend's response to mocked native setup success or failure.

Tests: [tests/categories.spec.ts](../../../tests/categories.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tools.spec.ts](../../../tests/tools.spec.ts), [tests/welcome.spec.ts](../../../tests/welcome.spec.ts).

Browser dimensions do not prove native transparent corners or window placement. Check those on the desktop.

On macOS, inspect all four corners with glass on, then off, and after hide/reopen. Check over a light background for square dark edges outside the rounded border. A window capture without the OS shadow must have transparent pixels at the outer corners.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
