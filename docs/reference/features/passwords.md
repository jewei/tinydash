# Password generation

Select Passwords or enter `password`, `password letters 24`, `passphrase 6`, or `pin 6` in All.

Character passwords support 6 to 64 characters and default to 20. Passphrases support 3 to 12 words and default to six. PINs support 4 to 12 digits and default to six. Generation uses the operating system's [secure random source](https://docs.rs/getrandom/0.3.4/getrandom/fn.fill.html) with unbiased selection. Passphrases use the bundled [EFF Large Wordlist](https://www.eff.org/dice), with attribution in Settings → About and the source data directory. Strength estimates use the number of possible generated values, in bits. They do not predict a cracking time. **Generate another** replaces the selected result. Copy uses the exact displayed value. It marks the copy as secret, so TinyDash and other clipboard managers do not save it. See [Clipboard history](clipboard.md#secrets).

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/tools.spec.ts tests/pins.spec.ts --grep "password|Password"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::tools::password
```

Use Passwords and explicit commands in All. Generate each affected type, select Generate another, and copy the displayed value. Check length and alphabet rules in Rust. Verify that clipboard history does not capture the generated test password, also after a restart, and pins do not store it.

Use the password desktop checks on each affected OS for system clipboard bytes and history exclusion. There is no native password journey in the current automated suite. Browser generation uses fixtures.

Generate a password, select another type, use Generate another, then copy. Check that copying uses the value currently displayed.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts), [tests/pins.spec.ts](../../../tests/pins.spec.ts).

Use test values only in evidence. Browser tests mock generation; Rust tests verify generation rules. Generated passwords are not stored in pins.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
