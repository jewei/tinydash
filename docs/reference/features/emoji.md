# Emoji search

Select Emoji, enter a matching term in All, or use the `:` prefix. Search uses local names, shortcodes, and categories. For example, `:coffee` finds coffee emoji.

Use arrow keys to navigate the grid. Enter copies the selected emoji and hides the launcher. Paste with the target application's paste command. TinyDash does not paste automatically.

The dataset uses default skin tones. Glyph appearance depends on operating system fonts.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts tests/pins.spec.ts --grep "emoji|Emoji"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::emoji
```

Use Emoji and the colon prefix in All. Navigate by keyboard, copy the selected emoji, and compare its exact Unicode text with the system clipboard. Check grid movement and pinned rows when affected.

Run `bun run verify:native` on Windows and Linux X11 for search and copy. Use the emoji desktop checks on macOS and for platform font changes. A screenshot cannot establish the copied Unicode sequence.

Search for `:coffee`, select an emoji by keyboard, copy it, and compare the system clipboard text with the selected value.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

A screenshot alone does not prove the copied Unicode sequence. Include the clipboard check.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
