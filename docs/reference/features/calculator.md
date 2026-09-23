# Calculator and currency

Select Calculator or enter an expression in All. The `=` prefix selects calculator parsing. Examples include `12 * 8`, `sqrt(144)`, `5 ft to cm`, and `=32 C to F`. Enter copies the selected value and hides the launcher.

Arithmetic and unit conversion work offline. Unit case is preserved. Calculator mode shows errors for invalid expressions. Each query is independent; variables and scripts are not supported.

Data rates distinguish bytes from bits. `Bps` means bytes per second, and `bps` means bits per second. For example, `8 Mbps to MBps` gives `1 MBps`. Decimal and binary prefixes work with these rates.

Currency input can omit `to`: `100 USD MYR` means `100 USD to MYR`. This shorthand needs a simple number and two uppercase currency codes.

Currency queries such as `100 USD to MYR` use daily ECB rates from [Frankfurter](https://frankfurter.dev/). The result shows the rate date. SQLite saves the complete rate table so conversions keep working offline after the first successful refresh. Old rates remain usable and show a cached-rates label. Unsupported currencies produce an error.

Opening the launcher starts a background refresh when rates are missing or the last download is at least 24 hours old. Failed automatic requests wait at least one hour before another open can retry. There is no refresh timer. Choose **Actions → Refresh currency rates** in any mode to request a refresh. Command/Ctrl + R also refreshes rates in Calculator mode. Requests have a 10-second limit and a 64 KiB response limit. Only the rate-table request goes to the service; queries, amounts, clipboard text, and file paths stay local. Set `currencyRatesEnabled` to `false` to stop network requests and keep using saved rates.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/launcher.spec.ts --grep "calculation|calculator|currency|conversion"
```

For affected backend behavior:

```sh
bun run test:rust -- calculator
bun run test:rust -- currency
```

Use Calculator and All. Enter `12 * 8`, select `96`, and copy it. Compare the system clipboard with `96`. Repeat `5 ft to cm` and an invalid expression. For currency changes, check refresh, failed refresh, disabled requests, and saved rates after restart.

Run `bun run verify:native` on Windows and Linux X11 for real calculation and copy. Use the calculation desktop checks on macOS and for currency downloads and offline restart. Native CI disables live currency requests.

Enter `12 * 8`, check `96`, copy it, and verify the clipboard bytes. Repeat a unit conversion. Test currency parsing and cached rates without a live service dependency.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Currency needs one successful rate download before offline use. Native CI disables live requests; it does not prove service availability.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
