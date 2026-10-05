# Features

What TinyDash does, for users and for anyone changing it. Keep this page true: update it in the same change as the behavior.

## Open and navigate

Press **Control+Shift+Space** (change it in Settings > General) from any app. On Wayland, bind a desktop shortcut to `tinydash`; running it again toggles the launcher.

| Keys            | Action                                                                                                             |
| --------------- | ------------------------------------------------------------------------------------------------------------------ |
| Type            | Search                                                                                                             |
| ↑ / ↓           | Select a result                                                                                                    |
| Enter           | Run the selected result's main action                                                                              |
| Mod+Enter       | Run its second action: Show in Finder for apps and files, Pin for clipboard entries, snippets, emoji, and commands |
| Mod+1 … Mod+9   | Run the main action of that row                                                                                    |
| Mod+K           | All actions for the result, plus Refresh, Settings, and Quit                                                       |
| Mod+Backspace   | Delete the selected clipboard entry                                                                                |
| Tab / Shift+Tab | Next / previous category                                                                                           |
| Mod+,           | Open Settings                                                                                                      |
| Escape          | Dismiss a warning, or hide and return to the previous app                                                          |

Mod is Command on macOS and Control elsewhere. Clicking a row runs it. The launcher opens on the screen with the pointer, with an empty query, and hides when another app gets focus (optional).

Command line: `tinydash` (show, or toggle if running), `--settings`, `--background` (used at login), `--mode <category>`.

## Categories

**All** mixes everything. An empty All shows pins, then up to eight things you use often (not clipboard text or system commands). Each other tab searches one source. Its empty view, listed below, shows pins first and stops at 100 results.

| Category  | Finds                                                                                                                    | Empty view                  |
| --------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------- |
| Apps      | Installed apps by name, bundle name, or executable                                                                       | Apps, most used first       |
| Files     | Names of files and folders in the indexed folders                                                                        | Files you opened before     |
| Clipboard | Text, images, and file lists you copied                                                                                  | Newest first                |
| Snippets  | Saved snippets and quicklinks by name or keyword                                                                         | Snippets and quicklinks     |
| Emoji     | Name, :shortcode:, or keywords in chosen languages                                                                       | Most used, then the catalog |
| System    | Lock, Sleep, Restart, Shut Down, Log Out, Empty Trash, Clear Clipboard History, System Settings, TinyDash Settings, Quit | All commands                |

Pin any app, file, clipboard entry, snippet, emoji, or command from its actions (up to 100 pins). Pins stay at the top of the empty All view and of their category. A pin of an item that is hidden for now (a file outside the indexed folders, or a clipboard entry while history is off) is kept for when the item returns; when the list is full, the oldest such pin makes room for a new one.

Usage ranking: each run of an app, file, emoji, snippet, quicklink, or system command raises it among similar matches (frequency up to 20 uses, recency by day). Copying a path or showing in Finder does not count. TinyDash remembers the 1,000 most recently used items.

## Instant answers (All only)

| Type       | Examples                                                                                | Enter copies          |
| ---------- | --------------------------------------------------------------------------------------- | --------------------- |
| Calculator | `12 * 8`, `sqrt(144)`, `5 ft to cm`, `8 Mbps to MBps`                                   | Result                |
| Currency   | `100 usd to eur`, `100 USD MYR`                                                         | Result, to 2 decimals |
| Time zones | `now`, `time in tokyo`, `london time`, `10am pacific to kl`, `2026-12-15 9:30 new york` | Time                  |
| Dates      | `today + 3 days`, `next friday + 2 weeks`, `in 10 days`, `2028-02-28 + 1 week`          | `YYYY-MM-DD`          |
| Passwords  | `password`, `pw 32`, `passphrase 8`, `pin 4`                                            | Value, marked secret  |
| Clean URL  | Paste a link with `utm_*`, `fbclid`, … parameters                                       | Clean URL             |
| Web search | `g rust`, `ddg`, `bing`, `brave`, `yt`, `gh`, `w` + text                                | Opens browser         |

Every All search with text ends with a web search on the chosen engine (Settings > Search), unless it starts with a web keyword such as `yt`.

- Currency uses the daily ECB table from Frankfurter, cached for offline use, refreshed when older than 12 hours. Only the table is downloaded.
- Time zones accept IANA names (`America/New_York`), city names, countries, and common names (`pacific`, `pst`, `kl`). A wall time skipped by a clock change gives no answer; a repeated one gives two.
- Passwords: 8–64 characters from letters, digits, and symbols (at least one of each); 3–12 words from the EFF large wordlist, without its four hyphenated words, so the hyphens between words stay clear; 4–12 digits. Uses the OS random source.

## Clipboard history

Off until you turn it on (in the Clipboard tab or Settings). While on, TinyDash saves what you copy, newest first, up to the limit you choose (1–1000; pins do not count). Lowering the limit deletes the oldest entries at once. Turning history off hides saved entries, and their pins, but keeps them until you clear them; turning it on again shows them.

- Text up to 16 KB. Images (up to 32, not counting pins, each at most 8 MB and 16 megapixels) and copied files (up to 64 paths, as references) are separate opt-ins.
- Copies marked secret by password managers are never read. On macOS, copies made while Passwords or Keychain Access is in front are skipped.
- Enter copies the entry back in its original format and returns you to the previous app, ready to paste. Copying an entry again moves it to the top.
- Clear History (in Settings, or the System command Clear Clipboard History) deletes everything except pins. Data stays on this computer in `tinydash.db`, unencrypted.
- Linux saves text only, and on Wayland the desktop reports a copy only while TinyDash has focus, so copies made elsewhere are saved when the launcher next opens (the latest one only).

## Snippets and quicklinks

Create them in Settings > Snippets and Settings > Quicklinks.

- **Snippet:** text with optional `{date}`, `{time}`, `{datetime}`, and `{clipboard}`. Enter fills the placeholders and copies the text. Text that includes `{clipboard}` is copied as secret, so clipboard histories skip it, in case the clipboard held a password.
- **Quicklink:** an http, https, or mailto URL, or an absolute or `~` path. `{query}` takes the text you type after the keyword, encoded: with keyword `jira` and URL `https://jira.example.com/browse/{query}`, typing `jira ABC-12` opens that issue.
- Limits: 500 snippets and quicklinks in all. A name has up to 100 characters, the text up to 32 KB, and the optional keyword is one word of up to 32 characters.

## Files

TinyDash indexes the names of files and folders in Desktop, Documents, and Downloads (change in Settings > Files; up to 50 folders and 50 names to skip), up to 50,000 entries. It skips hidden entries (names that start with a dot and, on Windows, files and folders marked hidden, such as `desktop.ini`), symbolic links, and folders named in the skip list (`node_modules`, `target` by default). It never reads file contents. On macOS, packages such as apps and photo libraries are listed as one item, without their contents. Changes are picked up the next time the launcher opens. On Linux, only changes directly inside the chosen folders are noticed at once (each watched folder uses one inotify watch from a per-user limit); deeper changes appear at the next full rescan, which starts when the launcher opens and the index is more than 15 minutes old. One word matches names; two or more words also match paths, so `project readme` finds `project/README.md`.

## Settings

Changes save at once. If a change cannot apply (for example, the shortcut is taken), Settings shows why and keeps the previous value.

| Section              | Settings                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------ |
| General              | Shortcut, appearance (system, light, dark), hide on focus loss, open at login, tray/menu bar icon            |
| Clipboard            | History on/off, entries to keep, save images and copied files (not on Linux), clear history                  |
| Files                | Folders to index, folder names to skip                                                                       |
| Search               | Web search engine, currency rates on/off, emoji skin tone, emoji keyword languages (Chinese, Malay, Spanish) |
| Snippets, Quicklinks | Create, edit, delete                                                                                         |
| About                | Version, settings and data folders, credits                                                                  |

## Platform notes

- **macOS:** app and file icons, and returning focus to the previous app after Escape or a copy. Restart, Shut Down, Log Out, and Empty Trash ask for Automation permission the first time. TinyDash has no Dock icon; the menu bar icon is off by default, so opening TinyDash again from Finder or Spotlight shows the launcher.
- **Windows:** the launcher uses native rounded corners. The app list comes from Start menu shortcuts (`.lnk`, `.url`, `.exe`, `.appref-ms`); Store apps without a shortcut, such as Calculator, are not listed. Results use generic icons.
- **Linux:** apps come from desktop entries. Clipboard history saves text only. Global shortcuts need X11; on Wayland, bind a desktop shortcut to `tinydash`. Log Out and System Settings support GNOME, KDE, and Xfce.
