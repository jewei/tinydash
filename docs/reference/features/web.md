# Web search

Select Web or use `web rust`, `google rust`, `ddg rust`, `bing rust`, `brave rust`, `yt rust`, or `gh rust`.

Web search supports Google, DuckDuckGo, Bing, Brave, YouTube, and GitHub. Enter opens the selected search in your default browser. **Copy search URL** copies its URL. TinyDash does not send the search until you open it. Other queries are limited to 256 characters.

In All mode, an engine name or shortcut starts a web search only when search text follows it. For example, `gh` can find Ghostty, while `gh rust` searches GitHub. The same rule lets `brave` find Brave Browser and `google` find Google Chrome.

Settings, Search can add custom keyword templates. See [configuration](../../how-to/configure.md).

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/tools.spec.ts tests/settings.spec.ts --grep "web search|custom web"
```

For affected backend behavior:

```sh
bun run test:rust -- providers::tools::web
```

Use Web, an explicit engine command in All, and an affected custom keyword. Check that spaces, Unicode, and `&` remain in one query value. Exercise Copy search URL and Open separately. A bare engine alias must still allow application matches.

Use the web-search desktop checks on each affected OS for copied bytes and the default-browser request. There is no automated native web-search journey. Browser tests prove the requested action against mocked IPC.

Enter `web rust`, select an engine, and verify the encoded URL for that engine. Check Copy search URL separately.

Tests: [tests/tools.spec.ts](../../../tests/tools.spec.ts), [tests/settings.spec.ts](../../../tests/settings.spec.ts).

Opening a result starts an external browser request. Mocked checks only prove the requested URL and action.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
