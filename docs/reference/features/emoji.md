# Emoji search

Select Emoji, enter a matching term in All, or use the `:` prefix. Search uses local names, shortcodes, and categories. In **Settings > Search > Emoji**, enable Simplified Chinese, Malay, or Spanish keywords. An equally strong localized short-name match ranks above a related keyword; for example, the rocket ranks above an astronaut for `火箭`. English names and shortcodes always remain available; result names stay in English. For example, `火箭`, `roket`, and `cohete` find the rocket when their language is enabled. For example, `:coffee` finds coffee emoji.

Use arrow keys to navigate the grid. Enter copies the selected emoji and hides the launcher. Paste with the target application's paste command, or explicitly choose **Paste to previous app** / Command/Ctrl + Shift + Enter. Enter still copies. See [direct paste](workflows.md#direct-paste) for focus verification, permissions, and unsupported desktops.

Choose **Preferred skin tone** in the same settings group, then save. Default preserves the standard emoji; the five other choices apply to supported emoji in name and keyword searches. Unsupported emoji stay unchanged. The result icon and name show the selected variant, and Copy, Paste, and Share use its exact Unicode sequence. Typing an emoji directly preserves its tone, including mixed-tone sequences.

Pins and usage scores use the base emoji key. Existing pins stay valid, and pinned results and suggestions follow the saved preference. Changing the preference does not change the bytes of an older result that is still displayed. Pinning an explicitly typed variant pins its base emoji; reopening that pin uses the saved preference.

Extra language keywords are bundled from Unicode CLDR 48.0.0. Only selected languages are indexed, and no network request is made during search. Newer emoji without a localized keyword keep their English names and shortcodes. Composed and decomposed accents match. Glyph appearance depends on operating system fonts. See the [data source, license, and update instructions](../../../src-tauri/data/emoji/README.md).

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/emoji-preferences.spec.ts tests/settings-draft.spec.ts
bun run verify:browser tests/launcher.spec.ts tests/pins.spec.ts --grep "emoji|Emoji"
```

For affected backend behavior:

```sh
bun run test:rust -- emoji
```

Check settings Save, Discard, reload, failed saves, and draft merging. Backend cases cover all six tone settings, typed mixed tones, selected-language keywords, English fallback, NFC accents, cancellation, unchanged pin keys, usage persistence, and copying an older result after a preference change.

Use Emoji and the colon prefix in All. Navigate by keyboard, copy the selected emoji, and compare its exact Unicode text with the system clipboard. Check grid movement and pinned rows when affected.

Run `bun run verify:native` on Windows and Linux X11 for search and copy. Use the emoji desktop checks on macOS and for platform font changes. A screenshot cannot establish the copied Unicode sequence.

Search for `:coffee` and a tone-capable emoji, select by keyboard, copy, and compare exact Unicode bytes with the displayed value. Repeat direct paste into a disposable field. Save a new tone while an older result is displayed, then copy that older result. Pin an emoji, change tones, restart, and confirm that the pin and usage rank remain. Check `火箭`, `roket`, and `cohete` with each language enabled and disabled. Keep network access off during these checks.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

A screenshot alone does not prove the copied Unicode sequence. Include the clipboard check.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
