# URL cleaning

Select URLs or paste an HTTP or HTTPS URL in All.

The URL cleaner removes common tracking fields, including `utm_*`, `fbclid`, `gclid`, and `msclkid`. It has additional Amazon, YouTube, and Spotify rules. It keeps unrelated query values, duplicate keys, encoded values, and fragments. Video IDs, timestamps, playlist IDs, and product options remain in place. It shows the removal count and provides **Copy cleaned URL** and **Open cleaned URL**. Cleaning works offline, supports URLs up to 8,192 characters, and does not follow shortened links.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/tools.spec.ts --grep "cleaned URLs"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::tools::url_cleaner
```

Use URLs and a URL pasted in All. Remove `utm_source` while preserving a useful query value and fragment. Exercise Copy cleaned URL and Open cleaned URL separately. Compare copied bytes and the address received by the default browser.

Use the URL desktop checks on each affected OS for system clipboard and default-browser actions. There is no automated native URL journey. Rust tests prove URL transformations without a network request.

Clean a URL containing `utm_source` and a useful query parameter. Check that the tracking field disappears and the useful parameter remains. Verify Copy and Open as separate actions.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts).

The browser suite records mocked action calls. It does not open the OS browser.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
