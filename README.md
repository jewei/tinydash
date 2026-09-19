# TinyDash

A small desktop launcher with local tools and a separate settings window.

Search installed applications and local filenames or paths. Find saved clipboard text, calculate values, convert units, search local emoji data, and run system commands. The app stays running after the window hides.

## Run

Install [Bun](https://bun.sh/docs/installation) 1.4.2, Rust 1.98.1 or later, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode command line tools. Windows needs the C++ build tools and WebView2. Linux needs the GTK and WebKitGTK development packages.

Bun manages dependencies and runs all JavaScript development tools. `bunfig.toml` selects Bun's runtime for project commands and their child processes. Node.js is not required. The Bun version is set in `package.json`; CI uses the same version and installs from `bun.lock`.

Dependencies were checked against the stable registry releases on 17 September 2026. `bun.lock` and `Cargo.lock` include the latest versions allowed by each dependency's requirements. GitHub Actions use explicit release tags. Tauri, SolidJS, the direct frontend packages, Bun, and `tauri-driver` were already current.

On Linux, GTK stays at 0.18.2 because Tauri 2.11 requires the 0.18 series. GTK 0.19 cannot resolve alongside it because both link `gtk-3`. Application discovery and system commands use GIO 0.22.9 independently. The new GIO API puts `DesktopAppInfo` in `gio-unix` 0.22.8. Tauri and GTK retain their internal GIO/GLib 0.18 dependencies. Recheck this constraint when upgrading Tauri. See [Cargo's native-library constraint](https://doc.rust-lang.org/cargo/reference/resolver.html#links).

This dependency update does not change the SQLite schema. Startup still applies the existing migrations. No manual data migration is required.

`@types/node` provides TypeScript declarations for the tooling APIs that Bun supports. It does not install the Node.js runtime.

```sh
bun install --frozen-lockfile
bun run tauri dev
```

The launcher opens on startup. The default global shortcut is `Control+Shift+Space` on macOS, Windows, and X11 Linux. The shortcut shows or hides the existing window. The tray menu also opens the launcher, refreshes applications, and quits the app.

Drag the handle above the search field to move the window. TinyDash keeps that position until it exits. Each time the window opens, it moves back inside the current screen's work area if needed. On Wayland, the compositor controls placement.

On macOS, use Control, not Command, for this global shortcut. macOS 27 uses `Command+Shift+Space` for [Siri Visual Intelligence](https://support.apple.com/en-my/102650). The previous TinyDash default can open Siri and cause a "Siri Unavailable" alert.

After building a new version, quit the running TinyDash process through the tray menu before opening the new build. Hiding or reopening its window keeps the existing code running.

`bun run dev` runs only the frontend in a browser. It displays an empty state because app discovery requires the desktop backend. It does not load test data.

## Appearance

Open **Actions → Appearance** to choose **Light**, **Dark**, or **Compact**. The Canvas layout uses a result list and a detail panel. Light has cream surfaces and peach highlights. Dark has charcoal and olive surfaces. Compact uses a single column with shorter rows. The app saves your choice on this device. Previous Mint, Paper, and Graphite choices map to Dark, Light, and Compact.

The design follows the approved warm refinement in `designs/polished.html`. It uses smaller row corners, a peach selection marker, and separate location and filename details. The main action stays visible when details scroll. Figtree and Caprasimo are bundled locally with their SIL Open Font License files in `public/fonts/`; no font service is required. The desktop window is 980 × 620. At small widths, the list takes the full width and clipboard previews appear below it.

On macOS, TinyDash loads application icons through NSWorkspace and converts them to bounded PNG images during the background scan. Application names become searchable before the icons finish loading. The list and detail panel use the same image, with a fallback if an icon is unavailable.

App rows show short descriptions. On macOS, TinyDash uses built-in descriptions for known bundle identifiers, then the app's category, then "Application" if neither is available. Linux uses the desktop entry's generic name or description. Apps without this data show "Application". Full paths remain in the detail panel and can still be searched.

Rounded corners require a transparent native window and a transparent page background. The macOS build enables Tauri's `macos-private-api` feature for transparency. This requires a separate approach for Mac App Store distribution, as described in the [Tauri transparency configuration](https://v2.tauri.app/reference/config/#transparent).

Press **Tab** from the search field to select the next visible category. **Shift + Tab** selects the previous category. Selection wraps at both ends. The search field keeps focus and the query stays unchanged. You can also click a category in the bar. Use the arrow keys to select results, **Enter** to open the selected result, and **Escape** to hide the launcher. Emoji mode uses a grid with arrow-key navigation. The detail panel provides the selected result's supported actions; app paths and clipboard content come from the existing backend.

To compare the designs, run `bun run dev` and open [the design preview](http://127.0.0.1:1420/designs/). Each preview uses the real launcher components with labelled sample data. Search for `Safari`, `:coffee`, or `12 * 8`, or change the search mode. Preview actions do not open applications, change the system clipboard, or run system commands. The separate preview entry and its sample backend are excluded from the desktop build.

All categories are visible by default. Open **Settings > Categories**, select the checkboxes for the categories you want to show, then select **Save changes**. Keep at least one category selected. Hiding a category does not remove its results from All. In small windows, the category bar scrolls to keep the selected category visible. **Option/Alt + Left/Right** also changes the category from the search field.

Select an item and use **Pin to All** or **Pin to Apps**, or the item's category, in the detail panel or Actions menu. Each category has its own pin list. You can pin the same item to both All and its category. Removing one pin keeps the other pin. Pinned items appear in a Pinned section when that category has an empty query. Typed searches keep their match order. Pins stay saved in the local SQLite database after a restart. Existing app pins move to both Apps and All when the database updates.

Tool pins save their input and output choice. When you open a category, TinyDash uses this input to show current tool results. Password pins save the generator type and length. They generate a new value after a restart. Generated passwords are not stored as pins.

## Search

The **All** mode searches applications, files, clipboard entries, emoji, calculations, and system commands. It also recognizes tool commands and pasted URLs. Use the category bar to select Apps, Files, Clipboard, Calculator, System, Emoji, Passwords, Datetime, URLs, or Web. In All mode, start a query with `:` for emoji or `=` for the calculator. An empty All query shows a welcome screen with search examples and keyboard hints. If you have pinned items to All, it shows those items instead. Select Apps to browse installed applications. The welcome examples work on macOS, Windows, and Linux.

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

Arithmetic and unit conversion work offline. Unit case is preserved. Calculator mode shows errors for invalid expressions. Each query is independent; variables and scripts are not supported.

Currency queries such as `100 USD to MYR` use daily ECB rates from [Frankfurter](https://frankfurter.dev/). The result shows the rate date. SQLite saves the complete rate table so conversions keep working offline after the first successful refresh. Old rates remain usable and show a cached-rates label. Unsupported currencies produce an error.

Opening the launcher starts a background refresh when rates are missing or the last download is at least 24 hours old. Failed automatic requests wait at least one hour before another open can retry. There is no refresh timer. Choose **Actions → Refresh currency rates** in any mode to request a refresh. Command/Ctrl + R also refreshes rates in Calculator mode. Requests have a 10-second limit and a 64 KiB response limit. Only the rate-table request goes to the service; queries, amounts, clipboard text, and file paths stay local. Set `currencyRatesEnabled` to `false` to stop network requests and keep using saved rates.

Enter copies the selected emoji or calculation result and hides the launcher. Paste the value in another application with its normal paste command. TinyDash does not paste automatically. Emoji search uses names, shortcodes, and categories from the local dataset. The list uses the default skin tones; glyph appearance depends on the operating system fonts.

## System commands

Select **System** to see the available commands. Search by name or alias: `reboot` finds Restart, `suspend` finds Sleep, and `preferences` finds Open system settings. The same commands appear in nonempty All searches.

Sleep, Restart, and Shut down require confirmation. Cancel receives focus. Enter on Cancel or Escape closes the dialog without sending a command. Select the named confirmation button to continue. This rule also applies to the numbered result shortcuts and actions menu. Rust rejects power requests without explicit confirmation. The dialog keeps the selected command if background results change.

| Platform | Implementation and limits                                                                                                                                                                                                                                                                                                                                                                           |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | Native Apple events request sleep, restart, and shutdown from the system process. Settings opens the installed System Settings or System Preferences app. Lock screen is offered only when Apple's legacy `CGSession` helper exists. It is absent on the macOS 27 development machine; use Control + Command + Q there. TinyDash does not use private lock APIs or request Accessibility access.    |
| Windows  | Native Win32 APIs request lock, sleep, restart, and shutdown. Power commands temporarily enable the process's shutdown privilege and restore it after the call. Settings uses the `ms-settings:` URI. Permissions and session policy can deny a request.                                                                                                                                            |
| Linux    | Power commands use the system `login1` D-Bus service and respect session policy and inhibitors. Lock uses an existing GNOME, freedesktop, Cinnamon, or MATE screen-lock service. These services can work on X11 or Wayland; a compositor without one is unsupported. Settings opens a known desktop entry for GNOME, KDE, Xfce, Cinnamon, MATE, LXQt, or COSMIC. Missing services produce an error. |

Commands run on the existing background action worker. They add no polling or startup service queries. TinyDash does not force applications to quit or bypass OS policy. An accepted request does not guarantee completion: the OS, another application, or unsaved work can prevent the transition. Only accepted requests update usage history. Empty trash and volume controls are not part of this command set.

The Mac integration adds the small `objc2-core-services` binding with only the required Apple event modules. Windows and Linux reuse their existing dependencies. Platform APIs follow [Apple's loginwindow lifecycle](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/Lifecycle.html), [Win32 ExitWindowsEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-exitwindowsex), and [systemd's login1 interface](https://github.com/systemd/systemd/blob/main/man/org.freedesktop.login1.xml).

## File search

TinyDash scans Desktop, Documents, and Downloads after the launcher opens. It uses the OS folder locations, including Windows Known Folders and Linux XDG user directories. Missing default folders are skipped. Select Files to search filenames and paths, or search in All mode. Press Enter to open a file with its default application. Command/Ctrl + Enter shows the file in its folder.

The index contains regular files only. It excludes dot files, hidden files, Windows system files, symbolic links, and the `node_modules` and `target` folders. Hidden and excluded folders are not traversed. Configured roots must be folders and cannot be symbolic links. Overlapping roots are scanned once. Paths that cannot be represented as UTF-8 are skipped. Access errors appear as a scan warning; other folders remain searchable. TinyDash does not read file contents.

The default limit is 50,000 files. A scan also stops after visiting 500,000 entries, including folders. The UI reports a limit when the index is incomplete. Use smaller roots if a large directory reaches a limit. Scans and index preparation run on a background worker. The previous index remains searchable until the new scan finishes. Search uses memory, returns at most 30 results, and applies usage scores before limiting file results.

File matching uses Unicode NFC so composed and decomposed accents match. The small [unicode-normalization crate](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/) supplies canonical normalization. This fixes accented filenames returned by macOS. Display text, file IDs, and open actions keep the original path.

Operating system notifications update the file index after creation, renaming, moving, and deletion. A single worker combines a burst of changes, waits for 300 ms of quiet, and starts a scan within two seconds of continuous changes. It keeps one pending request during a scan. There is no folder polling timer. Content-write and access events do not require filename indexing.

The [notify](https://docs.rs/notify/8.2.0/notify/) watcher uses FSEvents on macOS, ReadDirectoryChangesW on Windows, and inotify on Linux. Linux watches only folders accepted by the scanner, up to 8192 folders, so excluded trees do not consume recursive watches. Parent watches detect a removed or recreated search root. Registration limits or failures produce a warning. Network filesystems and restricted folders can omit events. **Refresh files** in the actions or tray menu remains available; Command/Ctrl + R refreshes files in Files mode. Set `fileWatchEnabled` to `false` for manual updates. File contents and file metadata are not stored in SQLite.

## Clipboard history

On a fresh installation, TinyDash asks before it saves clipboard text. Existing installations keep their saved capture choice. When capture is enabled, TinyDash captures text while it runs, including the current clipboard at startup. Select Clipboard mode to see recent entries. Pins stay at the top, and a fresh empty search selects the latest entry. Search saved text, use the arrow keys to inspect its full preview, and press Enter to copy it. Paste it in the target application with Command/Ctrl + V.

The default limit is 100 unpinned entries. Pins in Clipboard or All keep an entry outside this limit. Each entry can contain up to 16 KiB of UTF-8 text. Empty text, whitespace-only text, embedded null characters, images, and larger values are skipped. TinyDash preserves the accepted text exactly. Repeated consecutive values do not create writes. Copying an older value moves its existing entry to the top. Equal search scores keep the newest entries first.

Use the actions menu or Command/Ctrl + Backspace to delete the selected entry. **Clear unpinned** keeps entries pinned in All or Clipboard. The separate **Clear all clipboard history** action includes pins. Both ask for confirmation. Deleting an entry removes its pins from both categories. These actions leave the system clipboard unchanged. An unchanged clipboard is not captured again during the same session. Restarting TinyDash captures the current clipboard again when capture is enabled.

The actions menu can edit a copy, combine selected entries in a chosen order with a separator, or save the full text as a file. Edited and combined copies leave history unchanged. See the [search and control guide](docs/search-controls.md) for limits and examples.

History is local plain text in the same SQLite database as usage. It can contain sensitive text that you copy; there is no general password detection or encryption. Copies from TinyDash's password generator skip capture during that session. On Unix, the database is restricted to its owner. SQLite secure deletion is enabled, but backups and filesystem snapshots can retain earlier data. Turn off **Save clipboard history** in Settings to stop capture. Existing history remains searchable and can be cleared.

macOS checks the [pasteboard change counter](https://developer.apple.com/documentation/appkit/nspasteboard/changecount). Windows checks the [clipboard sequence number](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclipboardsequencenumber). Each check runs once per second and when the launcher opens. Text is read only when the counter changes. Rapid copies within one interval can be missed. Linux uses GTK [owner-change events](https://docs.gtk.org/gtk3/signal.Clipboard.owner-change.html) and asynchronous text requests, with no polling timer. Wayland can restrict background access; opening TinyDash requests the current clipboard again. Desktop session checks remain necessary for Wayland.

## Keys

| Key                          | Action                                                              |
| ---------------------------- | ------------------------------------------------------------------- |
| Up / Down                    | Select a result; wrap at either end                                 |
| Enter                        | Run the primary action; power commands ask first                    |
| Escape                       | Cancel a dialog, close the actions menu, or hide the launcher       |
| Command / Ctrl + 1 through 9 | Run the corresponding result's primary action                       |
| Command / Ctrl + Enter       | Show the selected app or file in its folder                         |
| Command / Ctrl + Backspace   | Delete the selected clipboard entry                                 |
| Command / Ctrl + ,           | Open TinyDash Settings                                              |
| Command / Ctrl + K           | Open the actions menu                                               |
| Command / Ctrl + R           | Refresh rates in Calculator, files in Files, or apps in other modes |
| Command / Ctrl + Q           | Quit TinyDash                                                       |

## Passwords, Datetime, URLs, and web search

| Input                                                                      | Result                                                                                  |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `password` or `password 32`                                                | Passwords with symbols, passwords with letters and digits, a word passphrase, and a PIN |
| `password letters 24`                                                      | A password with letters and digits                                                      |
| `passphrase 6`                                                             | Six random words                                                                        |
| `pin 6`                                                                    | A six-digit PIN                                                                         |
| `time in tokyo`                                                            | The current time in Tokyo, with your local time                                         |
| `next friday + 2 week`                                                     | Two weeks after the next Friday, using your local date                                  |
| `2028-02-28 + 1 day`                                                       | 29 February 2028                                                                        |
| `10:00 a.m. Pacific Time`                                                  | 10:00 in Los Angeles on its current date, converted to your local time                  |
| `tomorrow 3pm london`                                                      | Tomorrow at 15:00 in London, converted to your local time                               |
| `in 2 days 11pm new york`                                                  | A relative date conversion                                                              |
| `2026-10-25 1:30 london`                                                   | Both possible times during the daylight saving change                                   |
| `https://youtu.be/example?si=tracking&t=90`                                | A cleaned URL that keeps the video time                                                 |
| `web rust programming`                                                     | Search choices for six engines                                                          |
| `google rust`, `ddg rust`, `bing rust`, `brave rust`, `yt rust`, `gh rust` | A search with one engine                                                                |

Character passwords support 6 to 64 characters and default to 20. Passphrases support 3 to 12 words and default to six. PINs support 4 to 12 digits and default to six. Generation uses the operating system's [secure random source](https://docs.rs/getrandom/0.3.4/getrandom/fn.fill.html) with unbiased selection. Passphrases use the bundled [EFF Large Wordlist](https://www.eff.org/dice), with attribution in Settings → About and the source data directory. Strength estimates use the number of possible generated values, in bits. They do not predict a cracking time. **Generate another** replaces the selected result. Copy uses the exact displayed value and skips TinyDash's clipboard history for that copy.

**Datetime** calculates dates and converts times offline. Enter `next friday + 2 week`, `today + 3 days`, `in 2 weeks`, or `2028-02-28 + 1 week - 2 days`. Date calculations use your local date. "Next Friday" means the next Friday after today. Day and week offsets accept whole numbers, with up to 36,600 days per step. The result shows the date used for the calculation. **Copy this date** copies `YYYY-MM-DD`.

Time conversion uses the bundled [IANA database through chrono-tz](https://docs.rs/chrono-tz/0.10.4/chrono_tz/). City names, aliases, regions such as US and Australia, and full names such as `America/New_York` work offline. `10:00 a.m. Pacific Time` uses the current date in Los Angeles and applies its daylight saving rule. Add a date for another day, such as `2026-12-15 10:00 a.m. Pacific Time`. `PST` and `Pacific Standard Time` mean a fixed UTC−08:00 offset; `PDT` and `Pacific Daylight Time` mean UTC−07:00. Relative dates in a conversion use the source city's date. Today, tomorrow, yesterday, weekdays, next week, and `in N days/weeks` are supported. The output shows explicit dates and UTC offsets. Missing times during a clock change produce an error; repeated times produce two results. Current times refresh each minute while visible. An app update is needed for new time zone rules.

The URL cleaner removes common tracking fields, including `utm_*`, `fbclid`, `gclid`, and `msclkid`. It has additional Amazon, YouTube, and Spotify rules. It keeps unrelated query values, duplicate keys, encoded values, and fragments. Video IDs, timestamps, playlist IDs, and product options remain in place. It shows the removal count and provides **Copy cleaned URL** and **Open cleaned URL**. Cleaning works offline, supports URLs up to 8,192 characters, and does not follow shortened links.

Web search supports Google, DuckDuckGo, Bing, Brave, YouTube, and GitHub. Enter opens the selected search in your default browser. **Copy search URL** copies its URL. TinyDash does not send the search until you open it. Other queries are limited to 256 characters.

In All mode, an engine name or shortcut starts a web search only when search text follows it. For example, `gh` can find Ghostty, while `gh rust` searches GitHub. The same rule lets `brave` find Brave Browser and `google` find Google Chrome.

## Settings

Open **Actions → Settings**, press **Command+,** on macOS or **Ctrl+,** on Windows and Linux, or select **Settings** from the tray menu. The separate window opens on Shortcut. Click **Record new**, press a key combination, then click **Save changes**. Shortcut changes apply at once. If registration or saving fails, TinyDash keeps the previous shortcut.

Settings includes Shortcut, Appearance, Categories, Search, Clipboard history, File search, Currency, Privacy, and About. Save changes to apply visible categories, window behaviour, clipboard limits, file folders, file watching, and currency updates while TinyDash runs. Appearance applies immediately and stays in sync between windows. Closing Settings keeps an unfinished form. Use Discard to restore saved values.

The Search section adds app aliases, hidden apps, and custom web search templates with a URL preview. Shortcut settings include direct category keys and start at login. Category shortcuts and `tinydash --mode clipboard` open an empty category search. Start at login uses `--background` and keeps the window hidden. Privacy settings can export saved settings, preview an import before saving, and show the recovery archive. About includes an explicit update check. Release builds need an updater key and HTTPS feed; Ubuntu updates use a new `.deb` package. See [search controls](docs/search-controls.md), [data recovery](docs/data-recovery.md), and [release setup](docs/release-setup.md).

You can also open Settings directly with `tinydash --settings`. TinyDash writes `settings.json` in its application configuration directory on first launch. Manual file edits still require a restart. The settings screen preserves unknown JSON fields and refuses to overwrite a damaged file.

| Platform | Default location                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/settings.json`                                        |
| Windows  | `%APPDATA%\dev.tinydash.launcher\settings.json`                                                            |
| Linux    | `$XDG_CONFIG_HOME/dev.tinydash.launcher/settings.json`, or `~/.config/dev.tinydash.launcher/settings.json` |

```json
{
  "clearQueryOnOpen": true,
  "hideOnBlur": true,
  "shortcut": "Control+Shift+Space",
  "clipboardHistoryEnabled": true,
  "clipboardHistoryLimit": 100,
  "fileSearchRoots": null,
  "fileSearchLimit": 50000,
  "fileSearchExcludedDirs": ["node_modules", "target"],
  "fileWatchEnabled": true,
  "currencyRatesEnabled": true
}
```

Set `clearQueryOnOpen` to `false` to keep the previous query and search mode. TinyDash selects that text when the window opens. With the default setting, it clears the query and selects the first visible category, which is All by default. Set `hideOnBlur` to `false` to keep the window visible when another app receives focus. Use the settings screen to apply shortcut changes at once. An invalid settings file is left unchanged; the app uses defaults and displays a warning.

On macOS, a saved `CommandOrControl+Shift+Space` value from earlier TinyDash versions now uses `Control+Shift+Space`. This compatibility rule keeps the settings file intact, including other user settings. Other custom shortcuts remain unchanged. New settings files contain `Control+Shift+Space`.

Set `clipboardHistoryLimit` to a value between 1 and 500. TinyDash applies the limit when saved in Settings, on restart, and after each capture. Missing settings use their defaults. The settings file remains the editable startup configuration. Usage and clipboard data are stored separately in SQLite.

Set `fileSearchRoots` to `null` for the default folders, `[]` to disable file scanning, or an array of absolute paths. A leading `~` refers to your home folder on all three platforms. For example, `["~/Documents", "~/Projects"]` scans those two folders; `["~"]` scans your home folder. Windows paths in JSON need escaped backslashes, such as `"C:\\Users\\Alex\\Documents"`. `fileSearchLimit` is restricted to 1 through 100,000. `fileSearchExcludedDirs` contains exact folder names, not patterns. Changes saved in Settings start a new scan and update the file watcher. Manual JSON edits take effect after a process restart.

## Usage and ranking

The **All** mode keeps text results together by category, in this order: Apps, Files, Clipboard, System, then Emoji. Matching apps appear first, so `sa` puts Safari above the exact emoji shortcode `:sa:`. Valid calculations appear before these groups. Explicit tool commands and the `:` and `=` prefixes keep their own search scope. Category priority applies before the 30-result limit.

Successful app launches, file opens, emoji copies, and accepted system commands update a use count and last-used time. Frequently used and recently used results move higher within their category. This also applies when the search field is empty. Exact and prefix match bonuses still favor close matches within each category. A usage bonus cannot add an item that does not match the query.

The frequency bonus is 25 points per use, up to 500 points. The recency bonus starts at 500 points and decreases with the number of days since the last use. Their combined limit is 1,000 points, compared with 10,000 for an exact match and 2,000 for a prefix match. Results with equal scores in the same category retain their existing order.

TinyDash stores `result_id`, `use_count`, and `last_used_at` in `tinydash.sqlite3`:

| Platform | Default database location                                                                                           |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/tinydash.sqlite3`                                              |
| Windows  | `%APPDATA%\dev.tinydash.launcher\tinydash.sqlite3`                                                                  |
| Linux    | `$XDG_DATA_HOME/dev.tinydash.launcher/tinydash.sqlite3`, or `~/.local/share/dev.tinydash.launcher/tinydash.sqlite3` |

Opening a location, failed actions, and calculation copies do not change usage counts. Calculation IDs are temporary. Tool pins store the input needed to run that tool again. Other search queries are not stored. Copied calculation results can enter clipboard history while capture is enabled. App and file usage is tied to the indexed path; moving an item gives it a new ID. File usage IDs contain the full path.

The database loads once on a background worker. Search uses an in-memory copy and performs no database reads while typing. SQLite writes use one connection, a short lock timeout, and explicit transactions for schema migrations and clipboard capture with pruning. There are no database polling timers. SQLite is bundled through [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/), so users do not need a separate SQLite installation.

If the database cannot load or save, TinyDash shows a warning, pauses clipboard capture, and keeps ranking changes in memory until exit. A failed delete or clear reports an error and retains the entries. It leaves an unreadable or newer database intact. Fix the file access problem and restart to restore persistence. To reset all stored data, quit TinyDash, move `tinydash.sqlite3` to a backup location, then reopen the app.

## Build

```sh
bun run tauri build
```

Build on each target operating system. Tauri uses the platform configuration to produce a macOS DMG and app, a Windows NSIS installer, or a Linux Debian package. Output is in `src-tauri/target/release/bundle/`. See the [installation guide](docs/install.md) for downloads, checksums, system requirements, and removal. For a local macOS application without a DMG:

```sh
bun run tauri build --bundles app
```

CI produces development packages for Apple silicon Macs, Windows x64, and Ubuntu 24.04 x64. The release policy uses a signed and notarized Mac app, an unsigned Windows preview, and an Ubuntu package with SHA-256 checksums. Mac and Windows updates use separate Tauri signatures. Actual release packages still need verification. See [release setup](docs/release-setup.md).

The release profile preserves symbols in build tools to avoid a [Rust linker issue on macOS 27](https://github.com/rust-lang/rust/issues/157750). The shipped application remains stripped.

## Architecture

The path is `query → SearchManager → providers → ranking → top 30 results → SolidJS`.

- One Rust crate owns discovery, matching, ranking, usage persistence, indexed app IDs, launch actions, window lifecycle, settings, and shortcuts.
- `SearchManager` reuses a `nucleo-matcher` instance. Names, aliases, and paths are prepared when the app index changes. Matching ignores case and supports Unicode normalization. Match and usage bonuses are applied in `ranking/mod.rs` before selecting the top 30. Unused apps keep a stable alphabetical order when the query is empty.
- `AppProvider`, `FileProvider`, `ClipboardProvider`, `EmojiProvider`, `CalculatorProvider`, `SystemCommandProvider`, and `ToolProvider` return the same result model. Rust parses search modes and prefixes. The emoji index and small system command catalog load on their first search. Calculations use a fresh `fend-core` context with random values disabled and a cooperative 50 ms time limit. Tool results use a bounded cache of 64 issued IDs. Password refreshes keep the displayed value; a new password search or **Generate another** produces new values.
- Discovery builds a new index off the UI thread. The old index remains available during refresh. Tauri's existing async runtime runs application scans, searches, launch work, and currency requests. A file worker owns the scanner and watcher. A clipboard worker handles observations and database writes. Only macOS and Windows use the one-second clipboard counter timer.
- Currency lookup reads a shared, immutable rate table in memory. Network requests and SQLite writes run outside the search lock. `currency.rs` contains the source request and parser; the calculator does not depend on the HTTP response format. The HTTP client reuses the reqwest version already required by Tauri and uses the OS TLS stack.
- Clipboard capture, copy, delete, and clear use the same storage lock. A generation number rejects reads already in progress when an entry is removed. Search never waits for disk access. Linux coalesces pending clipboard observations in a bounded queue. Results include short text summaries; a separate request fetches the selected entry's preview.
- SolidJS keeps UI state and sends queries without a debounce timer. It sends one search request at a time and retains only the newest waiting query. This prevents IPC arrival order from cancelling the current query when startup events overlap. Request numbers prevent late replies from replacing newer results. Enter cannot execute an old result while a new query is pending.
- Hiding the window stops frontend search requests, drops waiting input, and ignores any late search reply. File indexing and clipboard capture continue in Rust. Reopening requests current results. A background refresh preserves the user's latest selection for the same query; a new query selects its first result.
- The frontend sends a result ID and an action. Rust resolves app paths, indexed file paths, emoji values, and calculation values. It checks that a file still exists before opening it. A bounded cache holds the last 32 calculation results so copying an issued result does not evaluate it again. The webview has no general shell, opener, filesystem, clipboard, or global-shortcut permissions.
- System command IDs resolve against the Rust catalog. Rust owns command aliases, confirmation text, and the confirmation rule. The frontend supplies explicit consent after the dialog. Platform modules contain the native API calls; no shell command text comes from the webview.
- The official global shortcut, opener, and clipboard manager plugins supply desktop integration through Rust. The official single-instance plugin brings the existing process forward when the user starts TinyDash again.
- macOS app icons come from NSWorkspace. Unavailable icons use initials. The UI uses bundled fonts and local CSS. It makes no network requests.

The providers use direct methods. No provider trait is needed. The shared result and action enums contain only implemented variants. Future providers can join `SearchManager` without moving logic into TypeScript.

```text
src/
  App.tsx                 Query state, results, keyboard input, actions menu
  bridge.ts               Typed calls to Rust
  components/             Icons, clipboard preview, and shared confirmation dialog
  styles/app.css          Launcher layout and states
tokens.css                Colors, fonts, and spacing
src-tauri/src/
  lib.rs                  Tauri setup, tray, and shortcut
  main.rs                 Process entry and startup error handling
  currency.rs             Rate validation and exchange-rate source request
  launcher/               SearchManager, file scan worker, actions, storage, clipboard monitor, window commands
  providers/              App, file, clipboard, emoji, calculator, and system providers
  platform/               OS app discovery, launch, clipboard, file flags, and system commands
  ranking/mod.rs          Query normalization and score bonuses
  db/                     SQLite operations and transactional migrations
  settings.rs             Small startup configuration file
  error.rs                Internal errors
tests/                    Browser tests and native WebDriver checks
scripts/ci/               Installer checks, checksums, and native test setup
docs/install.md           Install, replace, and remove test builds
docs/desktop-checks.md    Interactive checks for each desktop
.github/workflows/        Checks for macOS, Windows, and Linux
```

## Platform scope

| Platform | Application discovery                                                              | Limits                                                                                                                                                                                 |
| -------- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | `/Applications`, `~/Applications`, `/System/Applications`, and Finder              | Four directory levels. Does not enter app bundles, hidden directories, or directory symlinks. Localized display-name files are not read. App-bundle symlinks are resolved.             |
| Windows  | User and shared Start menu program folders and desktops, through known-folder APIs | Finds `.lnk`, `.exe`, and `.appref-ms` entries. Does not enumerate packaged apps without shortcuts or scan all of Program Files. Shortcut targets and their aliases are not extracted. |
| Linux    | GIO's installed desktop entries                                                    | GIO handles desktop visibility, localization, aliases, XDG precedence, launch arguments, and D-Bus activation. AppImages without desktop entries are not discovered.                   |

New applications appear after a refresh or restart. The file watcher applies only to file-search roots. Inaccessible application directories are skipped and counted in logs. The UI retains the old app index if a refresh fails.

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
cargo test --release --manifest-path src-tauri/Cargo.toml profile_search_50k_files -- --ignored --nocapture
```

See the [performance check results](docs/performance.md) for the measured search times, idle host memory, and measurement limits.

The [Checks workflow](.github/workflows/check.yml) builds on macOS, Windows, and Ubuntu 24.04. Each successful build produces a downloadable `TinyDash-<OS>-<architecture>` artifact with the installer, standalone app, commit information, SHA-256 checksums, and [installation instructions](docs/install.md). Artifacts remain available for 14 days. They are unsigned development builds. Mac CI mounts the DMG, copies its app to a temporary directory, and checks the copy.

After the build jobs pass, the [native check workflow](.github/workflows/native.yml) verifies checksums and installs the Windows and Linux packages. It checks the installed executable, Start menu shortcut or desktop entry, and same-version reinstallation. It runs the installed app through `tauri-driver`, then checks removal and retained test data. The Bun test script checks arithmetic, unit conversion, and emoji search through the real Rust backend. It copies results, reads the OS clipboard to verify their values, and reopens the resident app. It also installs two temporary application entries, checks initial input selection and arrow-key selection, then launches a harmless executable that records which entry was selected. It verifies that a copied emoji and a launched app move up the result list. It uses no mocked IPC and adds no application dependencies. The test removes its entries and stops its app and driver processes when it finishes. CI retains screenshots and failure logs in `native-results-<OS>-<architecture>` artifacts.

The native workflow can also test an existing installer build without compiling it again. Select **Native app checks**, choose **Run workflow**, and enter the Checks run ID that contains the build artifacts. The diagnostics record both the build commit and the test-code commit. This makes native failures faster to reproduce.

The native checks also copy test text from a separate process, verify capture and duplicate filtering, copy an older entry, delete it, and clear history with confirmation. They verify that deletion leaves the system clipboard unchanged.

File checks scan a temporary folder, test filename and path matching, and open a document through a temporary OS file association. They verify automatic updates after file creation, renaming, and deletion, including a new subfolder and a recreated search root. They also check manual refresh. Windows uses a unique test extension and restores the settings file after the check. Linux uses an isolated MIME association and configuration directory. Rust tests also cover overlapping roots, scan limits, hidden files, symbolic links, inaccessible folders, path validation, watcher event filtering, and file usage ranking before result limits.

Currency tests check source parsing, validation, cache persistence, offline conversion, refresh limits, and copy values across rate changes. UI tests check refresh status, retained results after network failure, and readable help text. Native CI disables live currency requests so a network outage cannot fail the desktop checks.

System command checks search the native catalog, cancel the restart dialog, and verify that Rust rejects power requests without confirmation. Rust tests construct Mac Apple events without sending them and check the Windows flags and Linux method mapping. Browser tests cover explicit confirmation, cancellation, extra Enter presses, results changing behind the dialog, duplicate submission, and action errors. CI does not execute lock, sleep, restart, or shutdown. Test those transitions only in a disposable desktop session after saving work; use the desktop check guide.

Linux test screenshots use `scrot` to capture the focused X11 window. This avoids a WebKit screenshot request that can time out while the page and other driver commands remain responsive. Windows continues to use the WebDriver screenshot API. Screenshot capture errors still fail the check.

To run the native check locally, quit TinyDash first. Install `tauri-driver` 2.0.6 and the platform driver. Windows requires Edge WebDriver matching its WebView2 Runtime; use a terminal without administrator privileges and a separate test user profile with no prior TinyDash data. Linux requires `WebKitWebDriver`, `xclip`, `xdg-utils`, `desktop-file-utils`, `scrot`, and an active X11 session; its data and configuration directories are isolated by the test. This check replaces the current clipboard text, clears saved clipboard history, and records test usage. Then run:

```sh
cargo install tauri-driver --version 2.0.6 --locked
bun run tauri build --no-bundle
bun run test:native
```

On Windows, the test starts TinyDash with a temporary WebView2 profile and a local debug port, then attaches Edge WebDriver. See Microsoft's [WebView2 attach procedure](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webdriver#approach-2-attaching-microsoft-edge-webdriver-to-a-running-webview2-app). The hosted Windows runner has administrator privileges, and [elevated WebView2 hosts ignore environment-based browser flags](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags). The CI wrapper therefore sets a temporary machine policy for `tinydash.exe` and restores it after the test. This wrapper refuses to run outside a disposable GitHub-hosted runner. Local tests use process environment variables. The application build contains no test flags.

The direct Tauri WebDriver supports Windows and Linux. macOS receives build and Rust test checks in CI; its desktop checks remain manual. See the [Tauri WebDriver setup](https://v2.tauri.app/develop/tests/webdriver/).

Use the [desktop check guide](docs/desktop-checks.md) for global shortcuts, focus changes, tray controls, single-instance behavior, and Linux Wayland sessions. The Linux native CI test uses a virtual X11 display. It does not replace these interactive checks.

## Next step

Follow the [public release plan](docs/release-plan.md) for a combined macOS, Windows, and Linux release. Record evidence in [release verification](docs/release-verification.md). The plan covers signing, installation, updates, the download page, and user checks. Keep the MVP tool set fixed while completing this work.
