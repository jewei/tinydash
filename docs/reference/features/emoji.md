# Emoji search

Select Emoji, enter a matching term in All, or use the `:` prefix. Search uses local names, shortcodes, and categories. For example, `:coffee` finds coffee emoji.

Use arrow keys to navigate the grid. Enter copies the selected emoji and hides the launcher. Paste with the target application's paste command. TinyDash does not paste automatically.

The dataset uses default skin tones. Glyph appearance depends on operating system fonts.

## Verification

Search for `:coffee`, select an emoji by keyboard, copy it, and compare the system clipboard text with the selected value.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

A screenshot alone does not prove the copied Unicode sequence. Include the clipboard check.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
