# Changelog

What changed in each release of TinyDash, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). Downloads are on the [Releases page](https://github.com/jewei/tinydash/releases).

## [Unreleased]

### Added

- The calculator reads percentages as people say them: `$30 + 20%`, `20% off $50`, `20 is what % of 200`, `50 to 75 is what %`, and `20 is 10% of what`.
- The calculator reads `plus`, `minus`, `times`, `multiplied by`, and `divided by`, and amounts such as `€20`.

### Changed

- `30 + 20%` now gives 36, which adds 20% of 30, instead of 30.2. `200 - 10%` gives 180.

## [0.7.0] - 2026-10-19

### Added

- Settings > Privacy lists each connection TinyDash makes to the internet: what it sends, how often, and whether it is on now.
- Type `?` to list every keyword, such as `pw`, `time in`, `g`, and your own snippet and quicklink keywords, with an example of each. Enter types the keyword for you.
- With the search field empty, ↑ brings back your recent searches. They are kept until TinyDash quits, and never saved.
- Give an app, file, snippet, quicklink, emoji, or command a hotkey from its actions (Set Hotkey). The hotkey runs it from any app. Settings > Search lists hotkeys, with Remove.
- Each release file comes with a signed record of the commit and workflow that built it. Check a download with `gh attestation verify FILE --repo jewei/tinydash`.

## [0.6.2] - 2026-10-18

### Added

- On macOS, install TinyDash with Homebrew: `brew install jewei/tap/tinydash`.
- Linux releases add an `.rpm` package for Fedora and openSUSE, and an AppImage that runs on most distributions.

### Fixed

- On macOS, a click on the tray icon while the launcher is open no longer makes Escape forget the app you came from.
- A hidden result no longer makes a list of results shorter, in a search or on an empty tab. The next result takes its place.
- On macOS, after an update replaces an app and changes its icon, the launcher shows the new icon without a restart of TinyDash. Updates that change files inside the app, such as some installer packages, can still show the old icon, because macOS keeps it.
- Exchange rates and the weather saved while the computer clock was set ahead now update once the clock is correct.
- On Linux, TinyDash now notices newly installed apps when `XDG_DATA_DIRS` or `XDG_DATA_HOME` is set but empty.
- A clip copied again within the same second as the clip before it moves to the top of the clipboard history.
- In Settings > General, Center keeps keyboard focus after you use it, so Tab moves on from there instead of from the top of the page.
- In Settings > Widgets, a clock city that is already in the list with other capital letters, such as `tokyo` after `Tokyo`, now shows "already in the list" instead of disappearing without a message.
- A hidden result can no longer be pinned, and a pin left on a hidden result gives up its place when the pin list is full. Show the result again in Settings > Search to pin it.
- Hide works only on a result that still exists, so a deleted snippet or quicklink no longer stays in the hidden list in Settings.
- If the note cannot save when you start a search, its text is no longer lost. The note shows it again when the widgets come back, and tries to save it again.
- On macOS, Also search with Spotlight now finds files for a search that starts with a dash, such as `-draft`.
- If TinyDash cannot read the saved note, the Notepad says why, and the other widgets still show. Before, the whole widget pane showed only an error.
- A Unix time now says "1 hour ago" or "In 1 year" instead of "60 minutes ago" or "In 12 months" when it is close to the next unit.
- The full JSON view of a clipboard card closes when the launcher hides, so the widgets show again the next time it opens.

## [0.6.1] - 2026-10-17

### Fixed

- `=5+3` now gives the calculator answer, as `5+3` does, instead of a web search.
- Copying a result from the calculator history moves it to the top, and Mod+Backspace deletes a history row.
- On Linux, Flatpak and Snap apps no longer all match a search for "flatpak" or "env".
- A calculation copied again within the same second moves to the top of the history.
- After the computer changes its time zone, the clock widget and the weather's update time show the new local time without a restart of TinyDash.
- In the Snippets tab, a keyword followed by text, such as `jira ABC-12` or `hi Sam`, now gives its result, as it does in All.
- On Windows, a copy from an app that marks it for clipboard history is less likely to go missing from the history when another app reads the clipboard at the same moment.
- Mod+J closes the full JSON view before it moves to the note, so you see what you type.
- A click on an empty part of the tab bar or footer no longer pins the launcher to that screen. Only a drag that moves the launcher saves its spot.
- If Settings cannot reload the snippet or quicklink list after a save, the open item stays as it is, so the next Save does not add a copy.
- On macOS, the Files tab now shows Spotlight results when Spotlight takes more than a second to answer. Before, such a search showed no Spotlight results.
- On Windows, a Files search with `/`, such as `docs/readme`, now finds files in that folder, also when folder names have accents or non-Latin letters.
- On macOS, Apps finds Keychain Access, Archive Utility, and the other Apple utilities in `/System/Library/CoreServices/Applications`.
- The calculator no longer shows wrong answers for `floor`, `ceil`, or `round` of numbers above 2^53 (about 9 quadrillion), or for `sinh` and `cosh` results above 2^64. It now shows no answer for these. It also shows no answer for some correct rounding results with 14 or more digits, such as `round(10000) to binary`.
- A snippet or quicklink cannot take a keyword that another one already uses, so a keyword no longer stops working without a warning.
- A quicklink keyword that is also a web keyword, such as `w`, now comes before the web search. So does the keyword of a snippet with `{query}`.
- On Windows, the "Copied" message shows centered on the right screen when your screens use different scales.
- Date math accepts a minus sign next to its number, as in `today -3 days` or `today-3d`, as it already did for a plus sign.
- Clicking Clear History while the actions menu is open now puts focus on Cancel in the confirm dialog, so Enter and screen readers work there.
- In Settings > Search, Show Again on a hidden result keeps keyboard focus in the list, on the next result.
- Clipboard history skips an image over 16 megapixels before it decodes it, so copying a very large image uses much less memory.
- A setting saved by a newer version, such as a new search engine or emoji language, no longer resets all your settings in an older version. TinyDash skips only the values it cannot read and uses defaults for them, the launcher names the settings that had such values, and a copy of the old file is kept.
- Changing the weather city and then back again quickly no longer shows the weather of the other city, or leaves the widget loading.
- A date such as `2026-10-17` answers with its weekday first, instead of a subtraction. `3pm` and `10am` no longer give a length answer.
- A currency result below 1, such as `1 JPY to USD`, shows 3 significant digits instead of `0.01`. An exact result, such as `100 USD to MYR in exact`, shows its fraction correctly.
- Date math such as `tomorrow + 1 day` or `next friday + 2 weeks` shows the date of `tomorrow` or `next friday` under the answer, not today's date.
- On Linux, the launcher opens in the center of the screen again when its saved spot was on a monitor that is gone, and it centers on the screen with the pointer when the display is scaled.
- On Windows, the launcher opens in the center of the screen with the pointer when that screen has another display scale than the one it was last on.
- On the Files tab, a selected Spotlight result stays selected when the results refresh in the background, so Enter opens the file you chose.
- Settings now saves a quicklink that needs typed text to work, such as `https://{query}`, instead of calling it invalid.
- Enter pressed while results load no longer runs a second result after you click one, and no longer changes the confirmation that your click opened.
- Files that Spotlight finds in the Files tab now show their size and modified date in the preview pane.
- When the widgets cannot load, the launcher says why, and Escape and Mod+K still work.
- A settings file that TinyDash cannot read, such as one saved in a non-UTF-8 encoding or locked by another app, no longer gets replaced with default settings. A file with a bad encoding moves to `settings.invalid.json`. A file that cannot be moved or a locked file stays as it is, and TinyDash does not save setting changes until you reopen it.
- On macOS, a password copied in Passwords or Keychain Access stays out of clipboard history when you switch to another app right after the copy.
- A file you hid no longer comes back in the Files tab from Spotlight results.

## [0.6.0] - 2026-10-14

### Added

- Give any app, indexed file, snippet, quicklink, emoji, or command an alias, such as `vsc` (Add Alias in its actions). Typing the alias puts that result first, after any instant answer. Settings > Search lists your aliases.
- A snippet with a keyword and `{query}` takes the text typed after the keyword, as quicklinks do: `hi Sam` copies `Hello Sam,`.
- After the web search, All offers up to three quicklinks that take `{query}`, filled with what you typed.
- New system commands: Open Trash, Sleep Displays, Toggle Dark Mode, and Make Clipboard Plain Text, which removes the formatting from copied text.
- Copied files in clipboard history have Copy as Plain Text, which copies their paths.
- Quicklinks can use `{clipboard}`, `{date}`, `{time}`, and `{datetime}`, as snippets do.
- Calculator history: copying an answer keeps it. Type `=` to see past results, or `= cm` to find one. Clear Calculator History (System) deletes them.

### Changed

- A row that carries typed text, such as `Jira: ABC-12`, no longer has Hide from Results, so Mod+Enter cannot hide the quicklink by accident. Hide it from its own row.
- In a snippet, `{query}` is now a placeholder: run from the snippet's own row, it is filled with nothing.
- A path quicklink takes a name as its text: `.` and `..` are refused, and its root, such as `~`, a drive, or a network share, must be written in the quicklink, not filled in.

### Fixed

- On macOS, a copy made in Passwords or Keychain Access just before you open the launcher no longer goes into clipboard history.
- When Settings cannot read your hidden results, it no longer also says there are none.

## [0.5.0] - 2026-10-08

### Added

- On macOS, the Files tab can also search your whole home folder with Spotlight (Settings > Files, off by default).
- Right-click a result to see its actions, as Mod+K shows them.
- Hide a result from search for good (Hide from Results in its actions). Settings > Search shows hidden results again.
- A short "Copied" message confirms a copy after the launcher hides.

### Changed

- Currency rates are off until you turn them on. A currency query asks first, and turning rates off deletes the saved table.

### Fixed

- A note that failed to save is saved again when you leave it, instead of waiting for more typing.
- Copied images in clipboard history are no longer kept in the window's cache after you delete them.
- On Linux, a clipboard card never shows text copied during its secret check.
- An older weather download no longer interrupts a newer one for the same city.
- The launcher warns when your saved data cannot be made private to your user account.

## [0.4.1] - 2026-10-08

### Changed

- App results say what an app is for, such as "Web browser", instead of its folder (macOS and Linux). The details still show the path.

### Fixed

- On macOS, the launcher has one thin edge instead of a thick double line.

## [0.4.0] - 2026-10-07

### Added

- Instant answers for a typed color (`#2F6F5E`, `rgb(…)`, `hsl(…)`), with RGB, HSL, contrast on white, and a swatch.
- Instant answers for a typed Unix time in seconds or milliseconds, with local time, UTC, and how long ago it was.
- Instant answers for file permissions: `chmod 755` gives `rwxr-xr-x`, and `rwxr-xr-x` gives `755`.

## [0.3.1] - 2026-10-07

### Fixed

- Clipboard card and focus timer text stays inside its card.
- On macOS, every focus timer notification shows, not only the first. The first one asks to allow notifications.

## [0.3.0] - 2026-10-07

### Added

- A widget pane next to an empty All search, with a switch for each widget in Settings > Widgets:
  - Clocks: local time and up to three cities.
  - Disk space, with a warning below 10% free.
  - Notepad: one scratch note (Mod+J).
  - Focus timer: Pomodoro sessions and breaks, with system notifications (Mod+P).
  - Weather for one city, from Open-Meteo.
  - Clipboard cards for a copied color, Unix time, or JSON (Mod+Shift+Enter).

### Fixed

- Reading the clipboard from two threads at once could crash TinyDash on macOS.

## [0.2.1] - 2026-10-07

### Added

- TinyDash updates itself on macOS and Windows when you choose Install and Restart. Turn the check off in Settings > General.
- Show, hide, and order the tabs, and keep a source such as emoji out of All.
- Drag the launcher by its tab bar or footer; Center Launcher (Mod+K) puts it back.
- Clear History on the Clipboard tab.

### Fixed

- Long copied text no longer runs under the preview buttons.
- Pinned clipboard entries stay off the start screen.

## [0.2.0] - 2026-10-06

A rewrite: the same keyboard-first launcher, with much less code.

### Added

- Light and dark themes that follow the system.
- Clipboard history for images and copied files, besides text.
- Snippets and quicklinks.

### Changed

- Clipboard history is off until you turn it on.
- Pins, usage ranking, and clipboard history from 0.1 are not carried over. Compatible settings are.

### Removed

- Paste into the previous app, the built-in utilities (process killer, color picker, keep awake, media, window controls), share and drag, and the auto-updater (back in 0.2.1).
- The Sage, Rose, and Ink themes, the Compact layout, and the macOS glass effect.

## [0.1.3] - 2026-09-25

The first public release: search apps, files, folders, and clipboard history; calculate, convert units, currencies, dates, and times; find emoji; generate passwords; clean URLs; and search the web.

[Unreleased]: https://github.com/jewei/tinydash/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/jewei/tinydash/compare/v0.6.2...v0.7.0
[0.6.2]: https://github.com/jewei/tinydash/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/jewei/tinydash/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/jewei/tinydash/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jewei/tinydash/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/jewei/tinydash/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/jewei/tinydash/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/jewei/tinydash/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/jewei/tinydash/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/jewei/tinydash/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jewei/tinydash/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/jewei/tinydash/releases/tag/v0.1.3
