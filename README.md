# TinyDash

A small desktop launcher. This version implements Phases 1 through 6.

Search installed applications and local filenames or paths. Find saved clipboard text, calculate values, convert units, and search local emoji data. Use the keyboard to open an application or file, or copy a result. The app stays running after the window hides.

## Run

Install [Bun](https://bun.sh/docs/installation) 1.4.2, stable Rust, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode command line tools. Windows needs the C++ build tools and WebView2. Linux needs the GTK and WebKitGTK development packages.

Bun manages dependencies and runs all JavaScript development tools. `bunfig.toml` selects Bun's runtime for project commands and their child processes. Node.js is not required. The Bun version is set in `package.json`; CI uses the same version and installs from `bun.lock`.

`@types/node` provides TypeScript declarations for the tooling APIs that Bun supports. It does not install the Node.js runtime.

```sh
bun install --frozen-lockfile
bun run tauri dev
```

The launcher opens on startup. The default global shortcut is `Command+Shift+Space` on macOS and `Ctrl+Shift+Space` on Windows and X11 Linux. The shortcut shows or hides the existing window. The tray menu also opens the launcher, refreshes applications, and quits the app.

After building a new version, quit the running TinyDash process through the tray menu before opening the new build. Hiding or reopening its window keeps the existing code running.

`bun run dev` runs only the frontend in a browser. It displays an empty state because app discovery requires the desktop backend. It does not load test data.

## Search

The **All** mode combines application, file, clipboard, emoji, and calculation results. Select **Apps**, **Files**, **Clipboard**, **Emoji**, or **Calculator** to limit the search. In All mode, start a query with `:` for emoji or `=` for the calculator. An empty All query shows applications only.

| Query            | Result                         |
| ---------------- | ------------------------------ |
| `rocket`         | 🚀, plus matching applications |
| `:coffee`        | ☕                             |
| `:laugh`         | 😂                             |
| `:heart`         | ❤️                             |
| `12 * 8`         | `96`                           |
| `sqrt(144)`      | `12`                           |
| `5 ft to cm`     | `152.4 cm`                     |
| `20 km/h to mph` | Approximately `12.427 mph`     |
| `=32 C to F`     | `89.6 °F`                      |

Arithmetic and unit conversion work offline. Unit case is preserved. Calculator mode shows errors for invalid expressions. Each query is independent; variables and scripts are not supported. Currency conversion needs exchange rates and remains scheduled for Phase 8.

Enter copies the selected emoji or calculation result and hides the launcher. Paste the value in another application with its normal paste command. TinyDash does not paste automatically. Emoji search uses names, shortcodes, and categories from the local dataset. The list uses the default skin tones; glyph appearance depends on the operating system fonts.

## File search

TinyDash scans Desktop, Documents, and Downloads after the launcher opens. It uses the OS folder locations, including Windows Known Folders and Linux XDG user directories. Missing default folders are skipped. Select Files to search filenames and paths, or search in All mode. Press Enter to open a file with its default application. Command/Ctrl + Enter shows the file in its folder.

The index contains regular files only. It excludes dot files, hidden files, Windows system files, symbolic links, and the `node_modules` and `target` folders. Hidden and excluded folders are not traversed. Configured roots must be folders and cannot be symbolic links. Overlapping roots are scanned once. Paths that cannot be represented as UTF-8 are skipped. Access errors appear as a scan warning; other folders remain searchable. TinyDash does not read file contents.

The default limit is 50,000 files. A scan also stops after visiting 500,000 entries, including folders. The UI reports a limit when the index is incomplete. Use smaller roots if a large directory reaches a limit. Scans and index preparation run on a background worker. The previous index remains searchable until the new scan finishes. Search uses memory, returns at most 30 results, and applies usage scores before limiting file results.

File matching uses Unicode NFC so composed and decomposed accents match. The small [unicode-normalization crate](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/) supplies canonical normalization. This fixes accented filenames returned by macOS. Display text, file IDs, and open actions keep the original path.

Use Refresh files in the actions or tray menu after adding, moving, or deleting files. Command/Ctrl + R refreshes files in Files mode. A full process restart also scans again. This phase has no filesystem watcher, periodic scan, file content index, or persisted file metadata. Only usage records for opened files are saved in SQLite.

## Clipboard history

TinyDash captures text while it is running, including the current clipboard at startup. Select Clipboard mode to see recent entries. Search any word in the saved text, use the arrow keys to inspect its full preview, and press Enter to copy it. Paste it in the target application with Command/Ctrl + V.

The default limit is 100 entries. Each entry can contain up to 16 KiB of UTF-8 text. Empty text, whitespace-only text, embedded null characters, images, and larger values are skipped. TinyDash preserves the accepted text exactly. Repeated consecutive values do not create writes. Copying an older value moves its existing entry to the top. Equal search scores keep the newest entries first.

Use the actions menu or Command/Ctrl + Backspace to delete the selected entry. Clear history asks for confirmation before deleting all entries. These actions leave the system clipboard unchanged. An unchanged clipboard is not captured again during the same session. Restarting TinyDash captures the current clipboard again. The schema includes a pin flag for later use; this version has no pin controls.

History is local plain text in the same SQLite database as usage. It can contain sensitive text that you copy; there is no password detection or encryption. On Unix, the database is restricted to its owner. SQLite secure deletion is enabled, but backups and filesystem snapshots can retain earlier data. Set `clipboardHistoryEnabled` to `false` and restart to stop capture. Existing history remains searchable and can be cleared.

macOS checks the [pasteboard change counter](https://developer.apple.com/documentation/appkit/nspasteboard/changecount). Windows checks the [clipboard sequence number](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber). Each check runs once per second and when the launcher opens. Text is read only when the counter changes. Rapid copies within one interval can be missed. Linux uses GTK [owner-change events](https://docs.gtk.org/gtk3/signal.Clipboard.owner-change.html) and asynchronous text requests, with no polling timer. Wayland can restrict background access; opening TinyDash requests the current clipboard again. Desktop session checks remain necessary for Wayland.

## Keys

| Key                          | Action                                              |
| ---------------------------- | --------------------------------------------------- |
| Up / Down                    | Select a result; wrap at either end                 |
| Enter                        | Open the app or file, or copy the selected result   |
| Escape                       | Close the actions menu, or hide the launcher        |
| Command / Ctrl + 1 through 9 | Run the corresponding result's primary action       |
| Command / Ctrl + Enter       | Show the selected app or file in its folder         |
| Command / Ctrl + Backspace   | Delete the selected clipboard entry                 |
| Command / Ctrl + K           | Open the actions menu                               |
| Command / Ctrl + R           | Refresh files in Files mode; otherwise refresh apps |
| Command / Ctrl + Q           | Quit TinyDash                                       |

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
  "shortcut": "CommandOrControl+Shift+Space",
  "clipboardHistoryEnabled": true,
  "clipboardHistoryLimit": 100,
  "fileSearchRoots": null,
  "fileSearchLimit": 50000,
  "fileSearchExcludedDirs": ["node_modules", "target"]
}
```

Set `clearQueryOnOpen` to `false` to keep the previous query and search mode. TinyDash selects that text when the window opens. With the default setting, it clears the query and returns to All mode. Set `hideOnBlur` to `false` to keep the window visible when another app receives focus. Shortcut changes take effect after restart. An invalid settings file is left unchanged; the app uses defaults and displays a warning.

Set `clipboardHistoryLimit` to a value between 1 and 500. TinyDash applies the limit on restart and after each capture. Missing settings use their defaults. The settings file remains the editable startup configuration. Usage and clipboard data are stored separately in SQLite.

Set `fileSearchRoots` to `null` for the default folders, `[]` to disable file scanning, or an array of absolute paths. A leading `~` refers to your home folder on all three platforms. For example, `["~/Documents", "~/Projects"]` scans those two folders; `["~"]` scans your home folder. Windows paths in JSON need escaped backslashes, such as `"C:\\Users\\Alex\\Documents"`. `fileSearchLimit` is restricted to 1 through 100,000. `fileSearchExcludedDirs` contains exact folder names, not patterns. These settings take effect after a process restart.

## Usage and ranking

Successful app launches, file opens, and emoji copies update a use count and last-used time. Frequently used and recently used results move higher in the list. This also applies when the search field is empty. Exact and prefix match bonuses still favor close matches. A usage bonus cannot add an item that does not match the query.

The frequency bonus is 25 points per use, up to 500 points. The recency bonus starts at 500 points and decreases with the number of days since the last use. Their combined limit is 1,000 points, compared with 10,000 for an exact match and 2,000 for a prefix match. Results with equal scores retain their existing order. Calculator results remain first for valid calculations.

TinyDash stores `result_id`, `use_count`, and `last_used_at` in `tinydash.sqlite3`:

| Platform | Default database location                                                                                           |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/tinydash.sqlite3`                                              |
| Windows  | `%APPDATA%\dev.tinydash.launcher\tinydash.sqlite3`                                                                  |
| Linux    | `$XDG_DATA_HOME/dev.tinydash.launcher/tinydash.sqlite3`, or `~/.local/share/dev.tinydash.launcher/tinydash.sqlite3` |

Opening a location, failed actions, and calculation copies do not change usage counts. Calculation IDs are temporary. Search queries are not stored. Copied calculation results can enter clipboard history while capture is enabled. App and file usage is tied to the indexed path; moving an item gives it a new ID. File usage IDs contain the full path.

The database loads once on a background worker. Search uses an in-memory copy and performs no database reads while typing. SQLite writes use one connection, a short lock timeout, and explicit transactions for schema migrations and clipboard capture with pruning. There are no database polling timers. SQLite is bundled through [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/), so users do not need a separate SQLite installation.

If the database cannot load or save, TinyDash shows a warning, pauses clipboard capture, and keeps ranking changes in memory until exit. A failed delete or clear reports an error and retains the entries. It leaves an unreadable or newer database intact. Fix the file access problem and restart to restore persistence. To reset all stored data, quit TinyDash, move `tinydash.sqlite3` to a backup location, then reopen the app.

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

The path is `query → SearchManager → providers → ranking → top 30 results → SolidJS`.

- One Rust crate owns discovery, matching, ranking, usage persistence, indexed app IDs, launch actions, window lifecycle, settings, and shortcuts.
- `SearchManager` reuses a `nucleo-matcher` instance. Names, aliases, and paths are prepared when the app index changes. Matching ignores case and supports Unicode normalization. Match and usage bonuses are applied in `ranking/mod.rs` before selecting the top 30. Unused apps keep a stable alphabetical order when the query is empty.
- `AppProvider`, `FileProvider`, `ClipboardProvider`, `EmojiProvider`, and `CalculatorProvider` return the same result model. Rust parses search modes and prefixes. The emoji index loads on its first search. Calculations use a fresh `fend-core` context with random values disabled and a cooperative 50 ms time limit.
- Discovery builds a new index off the UI thread. The old index remains available during refresh. Tauri's existing async runtime runs blocking scan, search, and launch work. A dedicated clipboard worker handles observations and database writes. Only macOS and Windows use the one-second clipboard counter timer.
- Clipboard capture, copy, delete, and clear use the same storage lock. A generation number rejects reads already in progress when an entry is removed. Search never waits for disk access. Linux coalesces pending clipboard observations in a bounded queue. Results include short text summaries; a separate request fetches the selected entry's preview.
- SolidJS keeps UI state and sends queries without a debounce timer. It sends one search request at a time and retains only the newest waiting query. This prevents IPC arrival order from cancelling the current query when startup events overlap. Request numbers prevent late replies from replacing newer results. Enter cannot execute an old result while a new query is pending.
- The frontend sends a result ID and an action. Rust resolves app paths, indexed file paths, emoji values, and calculation values. It checks that a file still exists before opening it. A bounded cache holds the last 32 calculation results so copying an issued result does not evaluate it again. The webview has no general shell, opener, filesystem, clipboard, or global-shortcut permissions.
- The official global shortcut, opener, and clipboard manager plugins supply desktop integration through Rust. The official single-instance plugin brings the existing process forward when the user starts TinyDash again.
- App icons use initials in this version. The UI uses system fonts and local CSS. It makes no network requests.

The providers use direct methods. No provider trait is needed. The shared result and action enums contain only implemented variants. Future providers can join `SearchManager` without moving logic into TypeScript.

```text
src/
  App.tsx                 Query state, results, keyboard input, actions menu
  bridge.ts               Typed calls to Rust
  components/             Icons, clipboard preview, and clear confirmation
  styles/app.css          Launcher layout and states
tokens.css                Colors, fonts, and spacing
src-tauri/src/
  lib.rs                  Tauri setup, tray, and shortcut
  main.rs                 Process entry and startup error handling
  launcher/               SearchManager, file scan worker, actions, storage, clipboard monitor, window commands
  providers/              App, file, clipboard, emoji, and calculator providers
  platform/               OS app discovery, launch, clipboard detection, and hidden file flags
  ranking/mod.rs          Query normalization and score bonuses
  db/                     SQLite operations and transactional migrations
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

Rust tests cover normalization, ranking, fuzzy matching, query parsing, provider selection, offline calculations, calculator interruption, emoji data, copy validation, settings, and platform filtering. Database tests cover migration rollback, upgrades, corrupt files, lock failures, saved usage, clipboard deduplication, limits, deletion, and reopening. UI tests cover navigation, actions, mode changes, stale replies and previews, plain text rendering, clear confirmation, IME input, errors, storage warnings, reopening, and layout widths. Browser tests use Tauri's official IPC mock and cannot prove native shortcut or OS launch behavior.

Run the optional host discovery check and search timing sample:

```sh
cargo test --manifest-path src-tauri/Cargo.toml installed_apps_smoke -- --ignored --nocapture
```

The [Checks workflow](.github/workflows/check.yml) builds on macOS, Windows, and Ubuntu 24.04. Each successful build produces a downloadable `TinyDash-<OS>-<architecture>` artifact with the app, commit information, and [desktop check instructions](docs/desktop-checks.md). Artifacts remain available for 14 days. They are unsigned development builds.

After the build jobs pass, the [native check workflow](.github/workflows/native.yml) downloads and tests the Windows and Linux artifacts through `tauri-driver`. The Bun test script checks arithmetic, unit conversion, and emoji search through the real Rust backend. It copies results, reads the OS clipboard to verify their values, and reopens the resident app. It also installs two temporary application entries, checks initial input selection and arrow-key selection, then launches a harmless executable that records which entry was selected. It verifies that a copied emoji and a launched app move up the result list. It uses no mocked IPC and adds no application dependencies. The test removes its entries and stops its app and driver processes when it finishes. CI retains screenshots and failure logs in `native-results-<OS>-<architecture>` artifacts.

The native workflow can also test an existing build without compiling it again. Select **Native app checks**, choose **Run workflow**, and enter the Checks run ID that contains the build artifacts. The diagnostics record both the build commit and the test-code commit. This makes native failures faster to reproduce.

The native checks also copy test text from a separate process, verify capture and duplicate filtering, copy an older entry, delete it, and clear history with confirmation. They verify that deletion leaves the system clipboard unchanged.

File checks scan a temporary folder, test filename and path matching, open a document through a temporary OS file association, report a deleted file, and refresh after file changes. Windows uses a unique test extension and restores the settings file after the check. Linux uses an isolated MIME association and configuration directory. Rust tests also cover overlapping roots, scan limits, hidden files, symbolic links, inaccessible folders, path validation, and file usage ranking before result limits.

To run the native check locally, quit TinyDash first. Install `tauri-driver` 2.0.6 and the platform driver. Windows requires Edge WebDriver matching its WebView2 Runtime; use a terminal without administrator privileges and a separate test user profile with no prior TinyDash data. Linux requires `WebKitWebDriver`, `xclip`, `xdg-utils`, `desktop-file-utils`, and an active X11 session; its data and configuration directories are isolated by the test. This check replaces the current clipboard text, clears saved clipboard history, and records test usage. Then run:

```sh
cargo install tauri-driver --version 2.0.6 --locked
bun run tauri build --no-bundle
bun run test:native
```

On Windows, the test starts TinyDash with a temporary WebView2 profile and a local debug port, then attaches Edge WebDriver. See Microsoft's [WebView2 attach procedure](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver#approach-2-attaching-microsoft-edge-webdriver-to-a-running-webview2-app). The hosted Windows runner has administrator privileges, and [elevated WebView2 hosts ignore environment-based browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags). The CI wrapper therefore sets a temporary machine policy for `tinydash.exe` and restores it after the test. This wrapper refuses to run outside a disposable GitHub-hosted runner. Local tests use process environment variables. The application build contains no test flags.

The direct Tauri WebDriver supports Windows and Linux. macOS receives build and Rust test checks in CI; its desktop checks remain manual. See the [Tauri WebDriver setup](https://v2.tauri.app/develop/tests/webdriver/).

Use the [desktop check guide](docs/desktop-checks.md) for global shortcuts, focus changes, tray controls, single-instance behavior, and Linux Wayland sessions. The Linux native CI test uses a virtual X11 display. It does not replace these interactive checks.

## Next step

Implement Phase 7: a small system-command provider with platform-specific actions and confirmation for restart and shutdown. Filesystem watching and cached currency rates remain in Phase 8.
