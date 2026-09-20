# Appearance and categories

Choose Light, Dark, or Compact in Actions, Appearance, or in Settings. Light and Dark use a list and detail panel. Compact uses shorter rows in one column. The choice applies immediately and stays in sync between windows.

The app bundles Figtree and Caprasimo with their license files. It does not need a font service. The desktop window is 980 by 620. At narrow widths, the result list uses the full width and clipboard previews appear below it.

Settings, Categories controls visible categories. Keep at least one selected. The category bar scrolls when required. Hiding a category does not remove its results from All.

## Verification

Change appearance, select a result, and inspect desktop and narrow widths. Hide a category, save, reopen, and confirm keyboard navigation uses the visible categories.

Tests: [tests/categories.spec.ts](../../../tests/categories.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tools.spec.ts](../../../tests/tools.spec.ts), [tests/welcome.spec.ts](../../../tests/welcome.spec.ts).

Browser dimensions do not prove native transparent corners or window placement. Check those on the desktop.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
