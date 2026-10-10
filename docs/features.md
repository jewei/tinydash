# Features

What TinyDash does, for users and for anyone changing it. Keep this page true: update it in the same change as the behavior.

## Open and navigate

Press **Control+Shift+Space** (change it in Settings > General) from any app. On macOS, a letter or sign in the shortcut is stored by its key position, so on a layout other than US, Settings may name a different key than the one pressed. The macOS recorder ignores the ISO key left of 1 and the Japanese keyboard keys (¥, ろ, 英数, かな, and numpad comma), which cannot be registered there, and Caps Lock, which changes typing. As on Windows and Linux, it also ignores the volume and media keys. On Windows and Linux, the recorder takes letters, digits, F1–F24, Space, Enter, Tab, Backspace, Delete, Insert, Home, End, Page Up, Page Down, the arrows, Print Screen, Scroll Lock, and the numpad digits and + − * / . keys. It ignores signs, which each layout puts on different keys; keys that Windows or Linux would register as another key, such as numpad Enter, Num Lock, and Play/Pause; Pause, which Windows turns into Break when Control is held; the volume and track keys; numpad digits and the decimal key while Num Lock is off; and on Windows, which reads Control+Alt as AltGr, a Control+Alt press without Shift that types something other than its own letter or digit, such as German Control+Alt+7 ("{") or French Control+Alt+1 ("&"). The modifier shows as Win on Windows and Super on Linux. On Linux with X11 and more than one keyboard layout, a letter or digit goes to the first key that types it in any of the layouts, which may be a different key. On Wayland, bind a desktop shortcut to `tinydash`; running it again toggles the launcher.

| Keys            | Action                                                                                                                         |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Type            | Search                                                                                                                         |
| ↑ / ↓           | Select a result                                                                                                                |
| Enter           | Run the selected result's main action                                                                                          |
| Mod+Enter       | Run its second action: Show in Finder for apps and files, Pin for clipboard entries, snippets, quicklinks, emoji, and commands |
| Mod+1 … Mod+9   | Run the main action of that row                                                                                                |
| Mod+K           | All actions for the result, plus Refresh, Center Launcher, Settings, and Quit                                                  |
| Mod+Backspace   | Delete the selected clipboard entry                                                                                            |
| Tab / Shift+Tab | Next / previous category                                                                                                       |
| Mod+,           | Open Settings                                                                                                                  |
| Mod+J           | Edit the widget pane's note, and back                                                                                          |
| Mod+P           | Start, pause, or resume the focus timer                                                                                        |
| Mod+Shift+Enter | Run the clipboard card's main action, such as Copy HEX                                                                         |
| Escape          | Close the full JSON view, dismiss a warning, or hide and return to the previous app                                            |

Mod is Command on macOS and Control elsewhere. Clicking a row runs it; right-clicking it shows its actions, as Mod+K does. After a copy, the launcher hides and a short "Copied" message shows near the bottom of the screen; it never takes focus from the app you return to. The launcher opens with an empty query, in the center of the screen with the pointer, and hides when another app gets focus (optional). Drag the empty parts of the tab bar or the footer to move it: it then opens there, until that spot is no longer on a screen. Center Launcher in the actions menu, or Center in Settings > General, puts it back.

Command line: `tinydash` (show, or toggle if running), `--settings`, `--background` (used at login), `--mode <category>`.

## Categories

**All** mixes everything. An empty All shows pins (except pinned clipboard entries), then up to eight things you use often (not clipboard text or system commands). Each other tab searches one source. Its empty view, listed below, shows pins first and stops at 100 results. Settings > General shows, hides, and orders the tabs after All; a hidden tab's results still show in All, and Tab skips it. Each tab also has an "in All" switch: turned off, its search results and suggestions leave All (its pins stay on the empty All view, a snippet or quicklink keyword still answers there, and its tab still finds them), so a source such as emoji cannot fill the list. A tab is never both hidden and out of All: turning one switch off while the other is off turns the other on. A tab added in a later version joins the end of your list, shown. Opening a hidden tab on purpose (`--mode clipboard`) shows it while it is open.

| Category  | Finds                                                                                                                                                                                             | Empty view                  |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| Apps      | Installed apps by name, bundle name, or executable; each says what it is for (macOS, Linux) or where it is                                                                                        | Apps, most used first       |
| Files     | Names of files and folders in the indexed folders                                                                                                                                                 | Files you opened before     |
| Clipboard | Text, images, and file lists you copied                                                                                                                                                           | Newest first                |
| Snippets  | Saved snippets and quicklinks by name or keyword                                                                                                                                                  | Snippets and quicklinks     |
| Emoji     | Name, :shortcode:, or keywords in chosen languages                                                                                                                                                | Most used, then the catalog |
| System    | Lock, Sleep, Restart, Shut Down, Log Out, Empty Trash, Open Trash, Sleep Displays, Toggle Dark Mode, Make Clipboard Plain Text, Clear Clipboard History, System Settings, TinyDash Settings, Quit | All commands                |

Pin any app, file, clipboard entry, snippet, quicklink, emoji, or command from its actions (up to 100 pins). Pins stay at the top of the empty All view and of their category. A pinned clipboard entry is kept rather than launched, so it heads the Clipboard tab and is found by a search, but stays off the empty All view. A pin of an item that is hidden for now (a file outside the indexed folders, or a clipboard entry while history is off) is kept for when the item returns; when the list is full, the oldest such pin makes room for a new one.

Give any app, indexed file, snippet, quicklink, emoji, or command an alias from its actions (Add Alias): one word of up to 32 characters, such as `vsc` for Visual Studio Code. Typing the whole alias puts its result first, after any instant answer and above any name match; typing its start ranks the result like a name that starts with the query. This works in All (while its tab is in All) and in its tab. Each alias names one result. Change Alias shows the current one; saving it empty removes it. A row that carries typed text, such as `Jira: ABC-12`, has no alias action: set it on the item's own row. Settings > Search lists aliases, with Remove (up to 500). An alias stays when its item is gone for now, such as an app being reinstalled, and goes with a deleted snippet or quicklink.

Hide any app, file, snippet, quicklink, emoji, or command from its actions (Hide from Results): search leaves it out for good, and hiding a pinned item unpins it. Settings > Search lists hidden results, with Show Again (up to 500 hidden). Clipboard entries are deleted instead.

Usage ranking: each run of an app, file, emoji, snippet, quicklink, or system command raises it among similar matches (frequency up to 20 uses, recency by day). Copying a path or showing in Finder does not count. TinyDash remembers the 1,000 most recently used items.

## Widgets

An empty All search shows widgets to the right of the results, in place of the details. Typing, or another tab, shows the details again. Settings > Widgets turns each widget on or off; with every widget off, the details stay.

- **Clocks** (on): local time and date, and up to three cities with their time and difference from local time. A city is any name that time zone answers know: `Tokyo`, `kl`, `Europe/London`, `pst`.
- **Disk space** (on): the free space on the disk of your home folder, and how full it is. Below 10% free, it warns. On macOS, free space counts purgeable files, as Finder does.
- **Weather** (off): the weather now in one city (Settings > Widgets), with today's high, low, and chance of rain, in °C or °F. It comes from Open-Meteo, which receives the city name and its location and nothing else. It is downloaded when the launcher opens and the last download is 30 minutes old (after a failure, 10 minutes later), and on Refresh; the last download stays for offline use.
- **Focus timer** (off): Pomodoro sessions, 25 minutes by default, each followed by a 5-minute break, with a 15-minute break after the fourth session (change all four in Settings > Widgets). A system notification says when a session or a break ends; a break starts when you choose. On macOS, the first notification asks you to allow notifications for TinyDash (change it later in System Settings > Notifications). Mod+P starts, pauses, or resumes it, and Mod+K lists Skip Break and Reset Timer. The timer keeps running while the launcher is hidden, but not after TinyDash quits, and turning the widget off resets it.
- **Clipboard cards** (off): when the clipboard holds a color (`#2F6F5E`, `#fff`, `rgb(47, 111, 94)`, `hsl(164, 41%, 31%)`), a Unix time in seconds or milliseconds from 2001 to 2103, or a JSON object or array of up to 1 MB, a card at the top shows it decoded: a color in HEX, RGB, and HSL with its contrast on white and black; a time in local time, UTC, and ISO 8601, and how long ago it was; JSON's size, top-level keys, and start, and the first 2,000 lines in full (View Full; Escape goes back). Each form has a copy button, Mod+Shift+Enter runs the card's main action, and Dismiss hides the card until you copy something new. The text is read each time widgets show, works while clipboard history is off, is never saved, and is skipped when its source marked it secret. On macOS, reading the clipboard may show a system alert that asks to allow pasting; System Settings can then always allow or deny it.
- **Notepad** (off): one scratch note of up to 10,000 characters, saved as you type to `tinydash.db` on this computer. If a save fails, the note says so and saves again when you leave it. Mod+J moves to the note and back; Escape returns to the search field.

## Instant answers (All only)

| Type        | Examples                                                                                | Enter copies          |
| ----------- | --------------------------------------------------------------------------------------- | --------------------- |
| Calculator  | `12 * 8`, `sqrt(144)`, `5 ft to cm`, `8 Mbps to MBps`                                   | Result                |
| Currency    | `100 usd to eur`, `100 USD MYR`                                                         | Result, to 2 decimals |
| Time zones  | `now`, `time in tokyo`, `london time`, `10am pacific to kl`, `2026-12-15 9:30 new york` | Time                  |
| Dates       | `today + 3 days`, `next friday + 2 weeks`, `in 10 days`, `2028-02-28 + 1 week`          | `YYYY-MM-DD`          |
| Passwords   | `password`, `pw 32`, `passphrase 8`, `pin 4`                                            | Value, marked secret  |
| Clean URL   | Paste a link with `utm_*`, `fbclid`, … parameters                                       | Clean URL             |
| Color       | `#2F6F5E`, `#fff`, `rgb(47, 111, 94)`, `hsl(164, 41%, 31%)`                             | HEX                   |
| Unix time   | `1791354301`, `1791354301250` (seconds or milliseconds, 2001 to 2103)                   | Local time            |
| Permissions | `chmod 755`, `chmod 4755`, `rwxr-xr-x`, `-rw-r--r--` (a bare `755` needs `chmod`)       | The other form        |
| Web search  | `g rust`, `ddg`, `bing`, `brave`, `yt`, `gh`, `w` + text                                | Opens browser         |

Every All search with text ends with a web search on the chosen engine (Settings > Search), unless it is a web keyword followed by text, such as `yt lofi`. After it come up to three quicklinks whose URL or path has `{query}`, most used first, each filled with the whole text: `Jira: ABC-12`. They do not show after a keyword search (`g rust`, or a snippet or quicklink keyword), when snippets are out of All, for a hidden quicklink, or for a quicklink that already shows because the text matches its name. A path quicklink shows only for text that is a name.

- Currency rates are off until you turn them on (Settings > Search, or Turn On Currency Rates on the answer that a currency query shows while they are off). TinyDash then uses the daily ECB table from Frankfurter, cached for offline use, refreshed when older than 12 hours. Only the table is downloaded. Turning rates off deletes the saved table.
- Time zones accept IANA names (`America/New_York`), city names, countries, and common names (`pacific`, `pst`, `kl`). A wall time skipped by a clock change gives no answer; a repeated one gives two.
- Color and Unix time answers decode like clipboard cards: a color shows its RGB, HSL, and contrast on white, and Mod+K copies each form; a Unix time shows UTC and how long ago it was.
- Passwords: 8–64 characters from letters, digits, and symbols (at least one of each); 3–12 words from the EFF large wordlist, without its four hyphenated words, so the hyphens between words stay clear; 4–12 digits. Uses the OS random source.

## Clipboard history

Off until you turn it on (in the Clipboard tab or Settings). While on, TinyDash saves what you copy, newest first, up to the limit you choose (1–1000; pins do not count). Lowering the limit deletes the oldest entries at once. Turning history off hides saved entries, and their pins, but keeps them until you clear them; turning it on again shows them.

- Text up to 16 KB. Images (up to 32, not counting pins, each at most 8 MB and 16 megapixels) and copied files (up to 64 paths, as references) are separate opt-ins.
- Copies marked secret by password managers are never read. On macOS, copies made while Passwords or Keychain Access is in front are skipped.
- Enter copies the entry back in its original format and returns you to the previous app, ready to paste. Copying an entry again moves it to the top.
- Text is saved as plain text. A list of copied files also has Copy as Plain Text, which copies its paths, one per line.
- The System command Make Clipboard Plain Text puts the clipboard's text back without its formatting, such as fonts and links from a web page or a document, so it pastes as plain text. It works while history is off, and leaves a copy that its app marked secret as it is. The plain copy is marked secret, so clipboard histories, TinyDash's included, do not save it again; a copy already in history stays there.
- Clear History deletes everything except pins, after you confirm. It is in the Clipboard tab (the footer button, or Mod+K), in Settings, and in the System command Clear Clipboard History. Data stays on this computer in `tinydash.db`, unencrypted.
- Linux saves text only, and on Wayland the desktop reports a copy only while TinyDash has focus, so copies made elsewhere are saved when the launcher next opens (the latest one only).

## Snippets and quicklinks

Create them in Settings > Snippets and Settings > Quicklinks.

- **Placeholders** work in both: `{query}` (the text you type after the keyword), `{clipboard}` (the text on the clipboard), `{date}` (`2026-10-11`), `{time}` (`09:05`), and `{datetime}`. They are filled in one pass, so a value that looks like a placeholder, such as typed text `{clipboard}`, stays as it is. Other text in braces stays too.
- **Snippet:** text. Enter fills the placeholders and copies the text. A snippet with a keyword and `{query}` takes the text typed after its keyword: with keyword `hi` and text `Hello {query},`, typing `hi Sam` copies `Hello Sam,`. Text that includes `{clipboard}` is copied as secret, so clipboard histories skip it, in case the clipboard held a password.
- **Quicklink:** an http, https, or mailto URL, or an absolute or `~` path. Placeholder values are encoded for a URL: with keyword `jira` and URL `https://jira.example.com/browse/{query}`, typing `jira ABC-12` opens that issue. In a path, a value must be a name, not a path. `{clipboard}` is empty when the copy is secret: marked so by its app, as password managers do, or, on macOS, made in Passwords or Keychain Access.
- Limits: 500 snippets and quicklinks in all. A name has up to 100 characters, the text up to 32 KB, and the optional keyword is one word of up to 32 characters.

## Files

TinyDash indexes the names of files and folders in Desktop, Documents, and Downloads (change in Settings > Files, as full paths such as `~/Projects`; up to 50 folders and 50 names to skip), up to 50,000 entries. It skips hidden entries (names that start with a dot, and items that macOS or Windows marks hidden, such as `~/Library` and `desktop.ini`), symbolic links, and folders named in the skip list (`node_modules`, `target` by default). It never reads file contents. On macOS, packages such as apps and photo libraries are listed as one item, without their contents. Changes are picked up the next time the launcher opens. On Linux, only changes directly inside the chosen folders are noticed at once (each watched folder uses one inotify watch from a per-user limit); deeper changes appear at the next full rescan, which starts when the launcher opens and the index is more than 15 minutes old. One word matches names; two or more words also match paths, so `project readme` finds `project/README.md`.

On macOS, **Also search with Spotlight** (Settings > Files, off by default) makes the Files tab also find names anywhere in your home folder, from the Spotlight index, below the results from your folders. It starts at two characters, skips hidden files and the folder names to skip, and adds up to 100 results. Spotlight finds only what it has indexed, and follows its own privacy settings. These results open, show in Finder, and copy their path; they cannot be pinned.

## Settings

Changes save at once. If a change cannot apply (for example, the shortcut is taken), Settings shows why and keeps the previous value.

| Section              | Settings                                                                                                         |
| -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| General              | Shortcut, appearance (system, light, dark), hide on focus loss, open at login, tray/menu bar icon, updates, tabs |
| Widgets              | Each widget and clipboard cards on or off, clock cities, weather city and unit, focus timer lengths              |
| Clipboard            | History on/off, entries to keep, save images and copied files (not on Linux), clear history                      |
| Files                | Folders to index, folder names to skip, Spotlight (macOS)                                                        |
| Search               | Web search engine, currency rates on/off, emoji skin tone, emoji keyword languages, aliases, hidden results      |
| Snippets, Quicklinks | Create, edit, delete                                                                                             |
| About                | Version, Check for Updates, settings and data folders, credits                                                   |

## Updates

On macOS and Windows, TinyDash looks for a new version when the launcher opens, at most every six hours (turn this off in Settings > General). It downloads only a small feed from GitHub. A new version shows as a bar in the launcher (Install and Restart, or Later) and in the actions menu (Mod+K: Install TinyDash … and Restart, or Hide Update Notice); a later check that finds none removes it. Nothing installs until you choose, and TinyDash refuses an update whose signature does not match its own key. Settings > About shows a version already found, with Install and Restart, and can also check at once. On Linux, and in builds made on your own computer, TinyDash does not update itself: download new versions from the Releases page.

## Platform notes

- **macOS:** app and file icons, and returning focus to the previous app after Escape or a copy. Restart, Shut Down, Log Out, Empty Trash, and Toggle Dark Mode ask for Automation permission the first time. TinyDash has no Dock icon; the menu bar icon is off by default, so opening TinyDash again from Finder or Spotlight shows the launcher.
- **Windows:** the launcher uses native rounded corners. Sleep Displays is named Turn Off Displays, and Open Trash and Empty Trash name the Recycle Bin. Toggle Dark Mode switches both apps and Windows, as Settings > Personalization > Colors does; a custom mix becomes one mode, the opposite of the apps' mode. The app list comes from Start menu shortcuts (`.lnk`, `.url`, `.exe`, `.appref-ms`); Store apps without a shortcut, such as Calculator, are not listed. Results use generic icons.
- **Linux:** apps come from desktop entries. Clipboard history saves text only. Global shortcuts need X11; on Wayland, bind a desktop shortcut to `tinydash`. Log Out and System Settings support GNOME, KDE, and Xfce. Toggle Dark Mode supports GNOME. Turn Off Displays (Sleep Displays) works on X11 and on KDE Plasma with Wayland.
