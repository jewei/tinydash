# URL cleaning

Select URLs or paste an HTTP or HTTPS URL in All.

The URL cleaner removes common tracking fields, including `utm_*`, `fbclid`, `gclid`, and `msclkid`. It has additional Amazon, YouTube, and Spotify rules. It keeps unrelated query values, duplicate keys, encoded values, and fragments. Video IDs, timestamps, playlist IDs, and product options remain in place. It shows the removal count and provides **Copy cleaned URL** and **Open cleaned URL**. Cleaning works offline, supports URLs up to 8,192 characters, and does not follow shortened links.

## Verification

Clean a URL containing `utm_source` and a useful query parameter. Check that the tracking field disappears and the useful parameter remains. Verify Copy and Open as separate actions.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts).

The browser suite records mocked action calls. It does not open the OS browser.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
