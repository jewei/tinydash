# Changelog

What changed in each release of TinyDash, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). Downloads are on the [Releases page](https://github.com/jewei/tinydash/releases).

## [Unreleased]

### Added

- Right-click a result to see its actions, as Mod+K shows them.

### Changed

- Currency rates are off until you turn them on. A currency query asks first, and turning rates off deletes the saved table.

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

[Unreleased]: https://github.com/jewei/tinydash/compare/v0.4.1...HEAD
[0.4.1]: https://github.com/jewei/tinydash/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/jewei/tinydash/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/jewei/tinydash/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/jewei/tinydash/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/jewei/tinydash/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jewei/tinydash/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/jewei/tinydash/releases/tag/v0.1.3
