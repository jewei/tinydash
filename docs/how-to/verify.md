# Verify a change

Run commands from the repository root. Install dependencies with `bun install --frozen-lockfile`. Install the browser once with `bunx --bun --no-install playwright install chromium`. Linux may need `--with-deps`.

## Run the fast check

```sh
bun run verify
```

This checks public file paths, local Markdown links and anchors, formatting, Rust formatting, and TypeScript. It then runs the browser tests tagged `@smoke`. These cover welcome search, category navigation, password actions, and the repository guards.

The command selects a free loopback port. Playwright starts its own Vite process and refuses to reuse another server. Each run uses a separate browser context and output directory. The app backend is mocked in browser tests.

## Run all source checks

```sh
bun run verify:full
```

This adds the frontend build, Clippy, Rust tests, and all browser tests. A warm Rust cache reduces the time. Compilation is part of the first run. The fast check does not replace these checks before a merge.

For a focused browser check, use a file from the [feature map](../reference/features/README.md):

```sh
bun run test:ui tests/pins.spec.ts
```

Direct browser runs use port 1421. Set `TINYDASH_TEST_PORT` to a different unused port for concurrent runs. No test attaches to an existing Vite server.

## Drive the real desktop app

Use Windows or Linux X11 in a test session. Quit an existing TinyDash process first. The native suite replaces clipboard contents and clears test clipboard history. On Windows, use a separate test user with no personal TinyDash data. On Linux, the suite creates isolated application data and configuration folders, but the session clipboard is shared.

Install `tauri-driver` 2.0.6. Linux also needs `WebKitWebDriver`, `xclip`, `xdg-utils`, `desktop-file-utils`, and `scrot`, with an active X11 session. Windows needs Edge WebDriver matching WebView2. Run without administrator privileges; the CI wrapper handles its disposable elevated runner separately.

```sh
cargo install tauri-driver --version 2.0.6 --locked
bun run tauri build --no-bundle
bun run verify:native
```

On a separate local Windows test account, set `$env:TINYDASH_NATIVE_TEST_PROFILE = '1'` in PowerShell before the command. Set `TINYDASH_NATIVE_BINARY` to test a specific installed executable.

The initial check verifies the platform and executable and refuses to drive an existing TinyDash process. A lock prevents two native wrapper runs in the same checkout. If a previous run was forcibly terminated, inspect its PID in `test-results/verification/native.lock` and confirm it has stopped before removing that lock.

The suite checks application launch markers, calculator and emoji clipboard values, clipboard history, file watching, and canceled system commands. It starts and stops its own driver and fixture processes. Native evidence is separate from browser evidence.

Use the Native app checks workflow to verify an existing CI package without rebuilding it. Set its build run ID. Use `release_artifacts=true` for a release candidate. The workflow records both the build commit and test-code commit.

macOS has no WebDriver adapter in this repository. Use [desktop checks](desktop-checks.md). `tests/native/focus-macos.swift` is an optional interactive focus check that needs Accessibility access; it is not an isolated replacement for the native suite. Adding an embedded macOS driver is separate work.

## Inspect the evidence

Each wrapper run writes `test-results/verification/<timestamp>-<pid>-<mode>/`. It contains numbered command logs and `result.json` with the source commit, working-tree state, platform, command results, and times. Native results go in its `native/` subfolder.

Fast browser tests retain screenshots and traces in the run's `browser/` folder. A trace records actions and their results. Full browser runs retain traces on failure. Open a trace with:

```sh
bunx --bun --no-install playwright show-trace path/to/trace.zip
```

Check the user action, the resulting state, and relevant side effects. A browser pass does not prove OS integration. A native copy check needs the exact clipboard value. A launch check needs the marker from the selected executable. A canceled power dialog must not execute the power action.

Playwright and the native suite stop the processes they own. Cleanup retains the run directory. Keep local evidence private. CI uploads only results from its synthetic test environment with a short retention period.

Update the feature page when an entry point, expected result, test, or platform limit changes.
