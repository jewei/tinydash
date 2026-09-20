# Dates and time zones

**Datetime** calculates dates and converts times offline. Enter `next friday + 2 week`, `today + 3 days`, `in 2 weeks`, or `2028-02-28 + 1 week - 2 days`. Date calculations use your local date. "Next Friday" means the next Friday after today. Day and week offsets accept whole numbers, with up to 36,600 days per step. The result shows the date used for the calculation. **Copy this date** copies `YYYY-MM-DD`.

Time conversion uses the bundled [IANA database through chrono-tz](https://docs.rs/chrono-tz/0.10.4/chrono_tz/). City names, aliases, regions such as US and Australia, and full names such as `America/New_York` work offline. `10:00 a.m. Pacific Time` uses the current date in Los Angeles and applies its daylight saving rule. Add a date for another day, such as `2026-12-15 10:00 a.m. Pacific Time`. `PST` and `Pacific Standard Time` mean a fixed UTC−08:00 offset; `PDT` and `Pacific Daylight Time` mean UTC−07:00. Relative dates in a conversion use the source city's date. Today, tomorrow, yesterday, weekdays, next week, and `in N days/weeks` are supported. The output shows explicit dates and UTC offsets. Missing times during a clock change produce an error; repeated times produce two results. Current times refresh each minute while visible. An app update is needed for new time zone rules.

## Verification

Enter `2028-02-28 + 1 day` in Datetime and check `2028-02-29`. Check an explicit source date and zone, including an ambiguous daylight saving time.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts).

Current-time results depend on the clock. Use explicit dates for fixed expected results. Backend coverage is in the Rust tools tests.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
