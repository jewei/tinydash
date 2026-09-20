# Calculator and currency

Select Calculator or enter an expression in All. The `=` prefix selects calculator parsing. Examples include `12 * 8`, `sqrt(144)`, `5 ft to cm`, and `=32 C to F`. Enter copies the selected value and hides the launcher.

Arithmetic and unit conversion work offline. Unit case is preserved. Calculator mode shows errors for invalid expressions. Each query is independent; variables and scripts are not supported.

Currency queries such as `100 USD to MYR` use daily ECB rates from [Frankfurter](https://frankfurter.dev/). The result shows the rate date. SQLite saves the complete rate table so conversions keep working offline after the first successful refresh. Old rates remain usable and show a cached-rates label. Unsupported currencies produce an error.

Opening the launcher starts a background refresh when rates are missing or the last download is at least 24 hours old. Failed automatic requests wait at least one hour before another open can retry. There is no refresh timer. Choose **Actions → Refresh currency rates** in any mode to request a refresh. Command/Ctrl + R also refreshes rates in Calculator mode. Requests have a 10-second limit and a 64 KiB response limit. Only the rate-table request goes to the service; queries, amounts, clipboard text, and file paths stay local. Set `currencyRatesEnabled` to `false` to stop network requests and keep using saved rates.

## Verification

Enter `12 * 8`, check `96`, copy it, and verify the clipboard bytes. Repeat a unit conversion. Test currency parsing and cached rates without a live service dependency.

Tests: [tests/launcher.spec.ts](../../../tests/launcher.spec.ts), [tests/native/smoke.ts](../../../tests/native/smoke.ts).

Currency needs one successful rate download before offline use. Native CI disables live requests; it does not prove service availability.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
