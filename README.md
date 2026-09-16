# TinyDash

A small desktop application launcher. This version implements Phases 1 and 2.

Search installed applications by name, available aliases, or path. Use the keyboard to open an application or show its location. The app stays running after the window hides.

## Run

Install [Bun](https://bun.sh/docs/installation) 1.4.2, stable Rust, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode command line tools. Windows needs the C++ build tools and WebView2. Linux needs the GTK and WebKitGTK development packages.

Bun manages dependencies and runs all JavaScript development tools. `bunfig.toml` selects Bun's runtime for project commands and their child processes. Node.js is not required. The Bun version is set in `package.json`; CI uses the same version and installs from `bun.lock`.

`@types/node` provides TypeScript declarations for the tooling APIs that Bun supports. It does not install the Node.js runtime.

```sh
bun install --frozen-lockfile
bun run tauri dev
```

The launcher opens on startup. The default global shortcut is `Command+Shift+Space` on macOS and `Ctrl+Shift+Space` on Windows and X11 Linux. The shortcut shows or hides the existing window. The tray menu also opens the launcher, refreshes applications, and quits the app.

`bun run dev` runs only the frontend in a browser. It displays an empty state because app discovery requires the desktop backend. It does not load test data.

## Keys

| Key                          | Action                                       |
| ---------------------------- | -------------------------------------------- |
| Up / Down                    | Select a result; wrap at either end          |
| Enter                        | Open the selected application                |
| Escape                       | Close the actions menu, or hide the launcher |
| Command / Ctrl + 1 through 9 | Open the corresponding result                |
| Command / Ctrl + Enter       | Show the selected app in its folder          |
| Command / Ctrl + K           | Open the actions menu                        |
| Command / Ctrl + R           | Refresh the application index                |
| Command / Ctrl + Q           | Quit TinyDash                                |

## Settings

TinyDash writes `settings.json` in its application configuration directory on first launch. Edit the file, then restart TinyDash.

| Platform | Default location                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/settings.json`                                        |
| Windows  | `%APPDATA%\dev.tinydash.launcher\settings.json`                                                            |
| Linux    | `$XDG_CONFIG_HOME/dev.tinydash.launcher/settings.json`, or `~/.config/dev.tinydash.launcher/settings.json` |

```json
{
  "clearQueryOnOpen": true,
  "hideOnBlur": true,
  "shortcut": "CommandOrControl+Shift+Space"
}
```

Set `clearQueryOnOpen` to `false` to keep the previous query. TinyDash selects that text when the window opens. Set `hideOnBlur` to `false` to keep the window visible when another app receives focus. Shortcut changes take effect after restart. An invalid settings file is left unchanged; the app uses defaults and displays a warning.

This small settings file is the only durable state in this phase. SQLite, migrations, usage frequency, and recency ranking belong to Phase 4.

## Build

```sh
bun run tauri build
```

Build on each target operating system. Output is in `src-tauri/target/release/bundle/`. For an unsigned local macOS application without a DMG:

```sh
bun run tauri build --bundles app
```

Signing, notarization, installers on other operating systems, and distribution need separate release checks.

The release profile preserves symbols in build tools to avoid a [Rust linker issue on macOS 27](https://github.com/rust-lang/rust/issues/157750). The shipped application remains stripped.

## Architecture

The path is `query → SearchManager → AppProvider → ranking → top 30 results → SolidJS`.

- One Rust crate owns discovery, matching, ranking, indexed app IDs, launch actions, window lifecycle, settings, and shortcuts.
- `SearchManager` reuses a `nucleo-matcher` instance. Names, aliases, and paths are prepared when the app index changes. Matching ignores case and supports Unicode normalization. Exact and prefix matches receive bonuses in `ranking/mod.rs`. Empty queries use a stable alphabetical order.
- Discovery builds a new index off the UI thread. The old index remains available during refresh. Tauri's existing async runtime runs blocking scan, search, and launch work. There are no polling loops or background timers.
- SolidJS keeps UI state and sends queries without a debounce delay. Request numbers prevent late replies from replacing newer results. Enter cannot open an old result while a new query is pending.
- The frontend sends an indexed app ID and an action. Rust resolves the path. The webview has no general shell, opener, filesystem, or global-shortcut permissions.
- The official global shortcut and opener plugins supply desktop integration. The official single-instance plugin brings the existing process forward when the user starts TinyDash again.
- App icons use initials in this version. The UI uses system fonts and local CSS. It makes no network requests.

No provider trait is needed for one provider. The shared result and action enums contain only implemented variants. Future providers can join `SearchManager` without moving logic into TypeScript.

```text
src/
  App.tsx                 Query state, results, keyboard input, actions menu
  bridge.ts               Typed calls to Rust
  components/             App initials and small SVG controls
  styles/app.css          Launcher layout and states
tokens.css                Colors, fonts, and spacing
src-tauri/src/
  lib.rs                  Tauri setup, tray, and shortcut
  main.rs                 Process entry and startup error handling
  launcher/               SearchManager, results, actions, window commands
  providers/apps.rs       In-memory application index
  platform/               macOS, Windows, and Linux discovery and launch
  ranking/mod.rs          Query normalization and score bonuses
  settings.rs             Small startup configuration file
  error.rs                Internal errors
tests/                    Browser tests and native WebDriver checks
scripts/ci/               Platform test setup
docs/desktop-checks.md    Interactive checks for each desktop
.github/workflows/        Checks for macOS, Windows, and Linux
```

## Platform scope

| Platform | Application discovery                                                              | Limits                                                                                                                                                                                 |
| -------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | `/Applications`, `~/Applications`, `/System/Applications`, and Finder              | Four directory levels. Does not enter app bundles, hidden directories, or directory symlinks. Localized display-name files are not read. App-bundle symlinks are resolved.             |
| Windows  | User and shared Start menu program folders and desktops, through known-folder APIs | Finds `.lnk`, `.exe`, and `.appref-ms` entries. Does not enumerate packaged apps without shortcuts or scan all of Program Files. Shortcut targets and their aliases are not extracted. |
| Linux    | GIO's installed desktop entries                                                    | GIO handles desktop visibility, localization, aliases, XDG precedence, launch arguments, and D-Bus activation. AppImages without desktop entries are not discovered.                   |

New applications appear after a refresh or restart. There is no filesystem watcher yet. Inaccessible directories are skipped and counted in logs. The UI retains the old index if a refresh fails.

The global shortcut backend supports Linux X11, not native Wayland. On Wayland, assign a compositor or desktop shortcut to run the TinyDash binary. The single-instance handler then requests focus for the existing window. Window placement and focus remain subject to compositor policy. This limit comes from the [global-hotkey platform support](https://docs.rs/global-hotkey/latest/global_hotkey/).

Some Linux desktops need a tray extension. If the tray cannot be created, TinyDash keeps a taskbar entry. Starting the binary again also requests the existing window. OS launch APIs confirm dispatch; an application can still fail after the OS accepts that request.

## Checks

```sh
bun run format:check
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --all-targets --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
bun run typecheck
bun run build
bunx --bun --no-install playwright install chromium
bun run test:ui
```

Rust tests cover normalization, ranking, fuzzy words, aliases, paths, result limits, duplicate IDs, settings, and platform filtering. UI tests cover navigation, actions, stale replies, IME input, errors, reopening, and layout widths. Browser tests use Tauri's official IPC mock and cannot prove native shortcut or OS launch behavior.

Run the optional host discovery check and search timing sample:

```sh
cargo test --manifest-path src-tauri/Cargo.toml installed_apps_smoke -- --ignored --nocapture
```

The [Checks workflow](.github/workflows/check.yml) builds on macOS, Windows, and Ubuntu 24.04. Each successful build produces a downloadable `TinyDash-<OS>-<architecture>` artifact with the app, commit information, and [desktop check instructions](docs/desktop-checks.md). Artifacts remain available for 14 days. They are unsigned development builds.

Windows and Linux jobs also run the actual release application through `tauri-driver`. The Bun test script installs two temporary application entries, searches through the real Rust backend, checks initial input focus and arrow-key selection, then launches a harmless executable that records which entry was selected. It uses no mocked IPC and adds no application dependencies. The test removes its entries and stops its driver processes when it finishes. CI retains screenshots and failure logs in `test-results-<OS>-<architecture>` artifacts.

To run the native check locally, quit TinyDash first. Install `tauri-driver` 2.0.6 and the platform driver. Windows requires Edge WebDriver matching its WebView2 Runtime. Linux requires `WebKitWebDriver` and an active X11 session. Then run:

```sh
cargo install tauri-driver --version 2.0.6 --locked
bun run tauri build --no-bundle
bun run test:native
```

The direct Tauri WebDriver supports Windows and Linux. macOS receives build and Rust test checks in CI; its desktop checks remain manual. See the [Tauri WebDriver setup](https://v2.tauri.app/develop/tests/webdriver/).

Use the [desktop check guide](docs/desktop-checks.md) for global shortcuts, focus changes, tray controls, single-instance behavior, and Linux Wayland sessions. The Linux native CI test uses a virtual X11 display. It does not replace these interactive checks.

## Next step

Implement Phase 3: local emoji search with `emojis`, and offline arithmetic and unit conversion with `fend-core`. Add only those dependencies when that work starts. Keep currency refresh, SQLite, clipboard history, file search, system commands, and filesystem watching in their specified later phases.
