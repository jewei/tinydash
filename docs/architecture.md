# Architecture

## Overview

```
 keyboard ─▶ Launcher (Solid) ──invoke──▶ commands.rs ──▶ search::search(Snapshot) ─▶ features/*
                 ▲                              │                                      (read-only)
                 │                              └──▶ actions::run(Action) ─▶ platform/*, store.rs,
                 └───────── events ◀────────────────── refresh.rs, monitor.rs        system_clipboard.rs
```

TinyDash is one Rust process with two webview windows: the **launcher**, created at startup and hidden, and **Settings**, created on demand. The Rust side owns all data and all behavior. The windows render what Rust sends and send back what the user chose.

## Search

1. The launcher calls `search(query, category)` for every keystroke. `launcher/latest.ts` keeps one request in flight; a newer query replaces the waiting one, and a stale reply is never shown.
2. `commands::search` takes a `Snapshot`: `Arc` handles to the current indexes, usage, pins, and settings. No lock is held while it matches.
3. `search::search` asks each feature for scored hits:
   - **All** shows instant answers (calculator, dates and times, passwords, clean URLs, web keywords, quicklink keywords), then name matches from every source the user keeps in All (`Settings::in_all`), then fuzzy matches, then a web search on the chosen engine unless a web keyword answered. Inside a tier the source order is fixed (apps, system, snippets, files, clipboard, emoji), so an app named like the query beats an emoji shortcode.
   - A **category** shows only its source, by score. An empty query lists pins first, then the category's own order.
4. `search/matcher.rs` scores names with nucleo: exact (10,000) > prefix (3,000) > word prefix (500) > fuzzy (below 500, as is a path match); an alias loses 100 and an emoji keyword 300. Paths match only when every query word appears in them. `search/usage.rs` adds up to 1,000 for frequency and recency, so usage reorders close matches but never lifts a weak match above a strong one.

## Results and actions

A `SearchResult` carries a stable `id` (`app:/Applications/Safari.app`, `clip:42`, `emoji:🚀`) and a list of `ResultAction`s. The first action runs on Enter, the second on Mod+Enter. The frontend sends the chosen `Action` back to `run_action`, plus the result ID when it was the main action, which counts as use.

`actions.rs` checks every action against Rust's own state. It launches only indexed apps, opens and reveals only indexed apps and files, opens only http(s) URLs (a quicklink the user saved may also open a mailto URL or an absolute or `~` path), pins only items that still exist (at most 100), and reads snippets and quicklinks from its own library; a quicklink's query cannot add a path separator. Previews load for indexed apps and files, saved clipboard entries (hidden while history is off), and snippets. Copies from the password generator are marked secret so clipboard managers skip them.

## State and background work

`State` (in `state.rs`) holds each index as a `Shared<T>`: readers clone an `Arc`; writers build a new value and swap it in, so a search never waits for a writer. Writers that must not interleave also take a `Mutex` in `State` (`settings_change`, `reloading`, `limited_change`) and may hold it across database or OS work.

| Work               | Trigger                                                                                                                                       | Where                           |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| App and file scans | Startup; launcher opens and the index is dirty or 15 minutes old; Refresh; a change to the file folder settings (files only)                  | `refresh.rs`                    |
| Dirty marking      | File system events under app folders or indexed folders                                                                                       | `watcher.rs`                    |
| Clipboard capture  | OS change counter changes (checked every 500 ms; GTK events on Linux)                                                                         | `monitor.rs`                    |
| Exchange rates     | Startup and launcher opens when rates are 12 hours old (retry after 1 hour); turning rates on; Refresh                                        | `refresh.rs`, `currency.rs`     |
| Weather            | Startup and launcher opens when the weather is 30 minutes old (retry after 10 minutes); a new city; turning it on; Refresh                    | `refresh.rs`, `weather.rs`      |
| Focus timer        | A thread wakes when the running phase ends (and at least every 30 seconds, as waits stop during sleep), notifies, and emits `widgets:changed` | `timer.rs`, `features/focus.rs` |
| Update check       | Launcher opens and the last check is 6 hours old (macOS and Windows release builds, when the setting is on); Check for Updates                | `updates.rs`                    |

When data changes, Rust emits `results:stale` and the launcher searches again, keeping its selection.

## Storage

- **Settings:** `settings.json` in the app config folder (`settings.rs`). Missing fields take defaults. A damaged file is renamed to `settings.invalid.json` and defaults are used.
- **Data:** `tinydash.db`, SQLite in the app's local data folder (`store.rs`; on Windows `%LOCALAPPDATA%`, so roaming profiles do not copy it). Tables: `usage` (the 1,000 most recently used IDs), `pins`, `clipboard`, `library`, `note` (the scratch note), `cache` (exchange rates and the weather). The file is owner-only on Unix and uses `secure_delete`. Migrations are append-only and refuse a database from a newer version.

## IPC contract

Every command is listed in `lib.rs` and defined in `commands.rs`. Types that cross IPC derive `ts_rs::TS`; `cargo test` writes them to `src/generated`, and CI fails if they are stale. `src/lib/ipc.ts` is the only frontend module that calls `invoke` or `listen`.

Events: `launcher:shown` (reset the query, optional category), `results:stale` (search again), `settings:changed` (apply theme and settings in every window), `update:changed` (the newer version an update check found, or none), `widgets:changed` (a widget changed on its own, such as the focus timer; the widget pane loads again). An event sent before a page listens is lost, so `launcher_init` also returns the category of the latest show, for example `--mode clipboard` at startup, and the update found so far; `about` also returns that update, for the Settings window.

Images use custom protocols rather than IPC: `icon://` serves system icons (macOS; it returns only an icon image, never file contents) and `clip://` serves saved clipboard images by ID.

## Platform layer

`platform/mod.rs` lists the functions each OS provides: app discovery and launch, icons, system commands, clipboard change detection and reads (one thread at a time), disk space, launcher window setup, and returning focus. Only this folder decides behavior by OS; elsewhere `cfg!` picks only a label or a default, and a test may use `#[cfg(unix)]` only when another OS cannot set up its case (AGENTS.md rule 6).

| Concern          | macOS                                 | Windows                      | Linux                                                                                                         |
| ---------------- | ------------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Apps             | `.app` bundles in app folders         | Start menu shortcuts         | GIO desktop entries                                                                                           |
| Clipboard change | `NSPasteboard.changeCount`            | `GetClipboardSequenceNumber` | GTK `owner-change`; the secret check and the text read run in one GTK request chain, so Linux saves text only |
| Secret copies    | nspasteboard.org types, Passwords app | History-exclusion formats    | `x-kde-passwordManagerHint`                                                                                   |
| System commands  | login framework, `pmset`, AppleScript | Win32 and `shutdown.exe`     | `loginctl`, `systemctl`, desktop tools                                                                        |

## Security

- Both windows load only bundled code under a strict CSP. They get the Tauri event permission and TinyDash's own commands, nothing else.
- External programs run only with fixed arguments; user text never reaches a shell.
- Clipboard history is opt-in, skips marked secrets, and is never sent anywhere. Clipboard cards read the text when the widget pane loads, skip marked secrets the same way, and keep nothing. Network requests: the ECB rate table, the weather (when the widget is on: the city name to Open-Meteo's geocoding service, then its coordinates to the forecast service), and on macOS and Windows the update feed on GitHub (when Check for updates is on) and an update the user chooses to install.

## Decisions

- **Stable dependencies.** Tauri 2, Solid 1.9, Vite+ 1.0. Preview releases needed patches and vendored crates.
- **Lazy freshness instead of live rescans.** Watchers only mark indexes dirty; work happens when the launcher opens. Idle cost is zero, and results are current when they are seen.
- **Settings apply at once.** No drafts, no Save button: each change is validated and saved, or rejected with the reason.
- **No plugin system.** Features are plain modules; add one when a real need appears.
