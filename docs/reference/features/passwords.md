# Password generation

Select Passwords or enter `password`, `password letters 24`, `passphrase 6`, or `pin 6` in All.

Character passwords support 6 to 64 characters and default to 20. Passphrases support 3 to 12 words and default to six. PINs support 4 to 12 digits and default to six. Generation uses the operating system's [secure random source](https://docs.rs/getrandom/0.3.4/getrandom/fn.fill.html) with unbiased selection. Passphrases use the bundled [EFF Large Wordlist](https://www.eff.org/dice), with attribution in Settings → About and the source data directory. Strength estimates use the number of possible generated values, in bits. They do not predict a cracking time. **Generate another** replaces the selected result. Copy uses the exact displayed value and skips TinyDash's clipboard history for that copy.

## Verification

Generate a password, select another type, use Generate another, then copy. Check that copying uses the value currently displayed.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts), [tests/pins.spec.ts](../../../tests/pins.spec.ts).

Use test values only in evidence. Browser tests mock generation; Rust tests verify generation rules. Generated passwords are not stored in pins.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
