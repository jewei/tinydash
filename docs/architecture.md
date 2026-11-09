# Architecture

TinyDash is one Go process on [mygo](https://github.com/egoist/mygo) with native UI: a launcher panel that does not take focus from other apps, a Settings window, and a "Copied" message panel. It replaced a Rust and Tauri app with a SolidJS frontend. `docs/rewrite-log.tsv` records each decision of the port.

## Packages

Module `github.com/jewei/tinydash`. The "May import" column is the complete list of allowed imports. `internal/archtest` fails the tests on any other.

| Package                                        | Owns                                                                                                                                                                                     | May import                                                     |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `cmd/tinydash`                                 | `main`: the `--data-dir` and `--hidden` flags, then `app.Main`                                                                                                                           | `app`                                                          |
| `internal/core`                                | `Result`, `Kind`, `Icon`, `Action` (sealed) and `Handler`, `Source` and its traits table, `ID`, `Category`, `SystemCommand`, `Personal`                                                  | stdlib only                                                    |
| `internal/match`                               | Normalization, and name, word, and alias scoring (a port of nucleo)                                                                                                                      | `core`, stdlib                                                 |
| `internal/weburl`                              | WHATWG URL parse, write, and check, shared by `cleanurl` and `library`                                                                                                                   | stdlib minus OS, network, and clock                            |
| `internal/feature/<name>`                      | One pure feature each: `apps`, `files`, `clipboard`, `library`, `emoji`, `system`, `calc`, `currency`, `datetime`, `password`, `cleanurl`, `color`, `unixtime`, `web`                   | `core`, `match`, `weburl`, stdlib minus OS, network, and clock |
| `internal/settings`                            | The `settings.json` model, tolerant load, `Normalize`, unknown-key passthrough, Tauri accelerator translation                                                                            | `core`, stdlib                                                 |
| `internal/search`                              | `Snapshot`, `Search`, `Resolve`, the source order of All, the answer list, the personal lists (usage, pins, hidden, aliases, hotkeys)                                                    | `core`, `match`, `feature/*`, `settings`                       |
| `internal/store`                               | All SQL and the append-only migrations                                                                                                                                                   | `core`, `feature/*` (row types), mygo `plugins/sqlite`         |
| `internal/platform`                            | Only the OS work that mygo lacks: data folders, clipboard change counter, secret copies on macOS, disk space, hidden files, Spotlight, system commands. The only package with GOOS files | `core`, purego, `x/sys`, `mygo`                                |
| `internal/engine`                              | Live state: the atomic snapshot, the single writer, refresh slots, first load, the clipboard monitor, the `settings.json` file, action checks and execution                              | `core`, `match`, `feature/*`, `search`, `settings`, `store`    |
| `internal/ui/kit`                              | What both views share: key recording and display, the Canvas palette, the embedded Figtree and Caprasimo fonts                                                                           | `core`, `settings`, `mygo/ui`                                  |
| `internal/ui/launcher`, `internal/ui/settings` | Native views and their models. Each reaches the app through its own `Backend` interface                                                                                                 | `core`, `search`, `settings`, `engine`, `ui/kit`, `mygo/ui`    |
| `internal/app`                                 | Wiring: data folders, the engine, the panel, the Settings window, the HUD, the tray icon, the global shortcut and hotkeys, the file watcher, Spotlight, the clipboard ticker, rates      | everything in `internal/`, mygo, `mygo/ui`, `mygo/transfer`    |
| `internal/golden`                              | Test support: compares Go answers with the frozen corpus in `testdata/golden`. The app never imports it                                                                                 | `core`                                                         |
| `internal/archtest`                            | The import rules as data (`rules.go`), checked by `go test`                                                                                                                              | stdlib                                                         |

Other folders:

- `formal/`: TLA+ models. `scripts/formal-verify.sh` checks them.
- `testdata/golden/`: the Rust app's answers, frozen. See "Compatibility with the Rust app".
- `.claude/skills/verify-tinydash/`: builds the app with `-tags e2e` and drives it in a Linux container. `internal/app/automation_e2e.go` links mygo's automation plugin only in that build.
- `.claude/skills/formal-verify-clipboard/`: how to change clipboard capture safely.
- `mygo.json` and `resources/icon.png`: packaging for `go tool mygo build`.

## Rules the tests enforce

1. `core`, `match`, `weburl`, `feature/*`, `settings`, and `search` are pure. They import no `os`, `os/exec`, `os/signal`, `net`, `syscall`, `unsafe`, `plugin`, `crypto/rand`, or `math/rand`, and do not call `time.Now`, `time.Sleep`, or the other clock functions. Disk, network, clock, and randomness come in as arguments: `fs.FS`, `io.Reader`, `time.Time`. (`internal/archtest`)
2. Each package imports only its layer's packages, and only allowlisted modules outside the standard library. Only `store` imports the SQLite plugin. Only `platform` imports purego and `x/sys`. (`internal/archtest`)
3. Only `platform` has `_darwin.go`, `_windows.go`, or `_linux.go` files or OS build tags. Outside `platform`, only a file named `labels.go` reads `runtime.GOOS`, to pick a label or a default. (`internal/archtest`)
4. Every Go file is gofmt-formatted. (`internal/archtest`)
5. Every shipped migration keeps its SHA-256. (`TestMigrationsNeverChange` in `internal/store`)
6. Every `Source` has a traits row and a frozen ID prefix. (`TestSourcePrefixesNeverChange` and `TestSourceTraits` in `internal/core`)
7. Every action has its own `Handler` method and the Rust rule for "counts as use". (`TestEveryActionHasItsOwnHandlerMethodAndUseRule` in `internal/core`)
8. Each ported answer matches the frozen Rust answer, or is listed as a divergence. (`internal/golden` and the golden test of each feature)

## Data flow and concurrency

- **Snapshot.** `engine` holds `atomic.Pointer[search.Snapshot]`. Readers load it and never lock. A published snapshot never changes.
- **Single writer.** One goroutine (`Engine.Run`) makes every change: actions, settings, saved clips, swapped-in indexes, and rates. A caller sends `func(next *search.Snapshot, db *store.Store) error` through `Engine.Change`. The writer applies it to a shallow copy, writes the store, publishes the copy, and calls `onChange`. The main thread never waits on the writer.
- **Search** runs synchronously on the main thread when the query, the tab, or the data changes. `internal/search/search_bench_test.go` measures it with 5,000 apps and 50,000 files against a budget of 8 ms at p95.
- **Refresh slots.** Each scanned source (apps, files) has one `engine.Slot`: a goroutine that ranges over a channel with capacity 1. `Request` is a non-blocking send. So requests coalesce, one scan runs at a time, and a request during a scan causes one more scan.
- **Clipboard monitor.** A 500 ms tick reads the OS change counter. It reads the content only when the counter changed and history is on, and sends `SaveClip(epoch, content)` to the writer. Clear and history-off bump the epoch, and the writer drops a save with an old epoch. `formal/ClipboardHistory.tla` checks this order against every interleaving with Clear and history on/off.
- **View updates.** `onChange` calls `panel.Update`, which runs `launcher.Model.Stale` on the main thread: it searches again, keeps the selection by ID, and loads the widgets again. An open Settings window reads the data again too.

## Actions

`core.Action` is a sealed interface with `Dispatch(Handler) error`, `CountsAsUse() bool`, and `Name() string`. `Handler` has one method per action type, so a new action does not compile until `engine` handles it. `engine.Actions` checks every action again against the published snapshot before it runs it. It launches only indexed apps. It opens and reveals only indexed apps and files, and the paths that a Spotlight search returned (`files.FoundPaths`). It opens only web addresses, and reads snippets and quicklinks from its own library. A hotkey runs the main action of its result through `Actions.RunHotkey`. The app writes a hotkey's error to the log.

## Background work

| Work               | Trigger                                                                                                                                  | Where                                            |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| App and file scans | Startup; the launcher shows and the index is 15 minutes old (files also when marked dirty); Refresh; a change of the folder settings (files) | `internal/app/app.go`, `engine/rebuild.go`       |
| Dirty marking      | mygo `WatchFiles` events under the indexed folders (with subfolders on macOS and Windows, one level on Linux)                           | `internal/app/watch.go`                          |
| Clipboard capture  | Every 500 ms. The content is read only when the change counter moved; Linux has no counter                                               | `internal/app/clipboard.go`, `engine/monitor.go` |
| Exchange rates     | The launcher shows and the rates are 12 hours old; turning rates on; Refresh                                                             | `internal/app/app.go`, `feature/currency`        |
| Spotlight results  | Each Files search of two or more characters while the setting is on (macOS), through `mdfind` on a pseudo-terminal                       | `internal/app/spotlight.go`, `platform`          |

## UI

Views read a model and call methods on it. The launcher's keys are one table in `internal/ui/launcher/keymap.go`. The commands they map to are plain methods with table tests. The actions menu, the alias dialog, the hotkey dialog, and confirmations are overlays drawn in the panel, so the nonactivating panel keeps the keyboard. Tests drive views with mygo's `ui.NewTester` against a fixture snapshot and a fake backend, and move time with `Advance`, never `Sleep`.

The widget pane (clocks and disk space) shows beside the results of an empty All search. The HUD is a second panel. It shows "Copied" with `ShowInactive`, so it never takes focus, and the pointer passes through it.

## Storage

- **Folders.** `platform.AppDirs("dev.tinydash.launcher")` gives the folders of the Tauri app: `settings.json` in the config folder (`os.UserConfigDir`), and `tinydash.db` in the local data folder. `--data-dir` puts both in one folder of your choice.
- **Settings.** `internal/settings` reads `settings.json`, and `engine/settingsfile.go` writes it. Missing fields take defaults. Keys that this version does not model, such as those of removed features, are kept and written back. A value that cannot be read takes its default, and the file is first copied to `settings.invalid.json`. A damaged file (bad JSON, or not a JSON object, such as `[]`) is moved to `settings.invalid.json`. A file that cannot be read for another reason stays in place, and saves are refused until the next launch. A save writes a synced temporary file, then renames it.
- **Data.** `tinydash.db`, SQLite through mygo's `plugins/sqlite`, without cgo. `go tool mygo build` bundles the SQLite library. A run from source downloads it once into the user cache and checks its SHA-256. Tables: `usage` (the 1,000 most recently used IDs), `pins`, `clipboard`, `library`, `hidden`, `aliases`, `hotkeys`, `calculations`, `cache` (exchange rates), and `note` (the text of the removed Notepad, kept untouched). The file is owner-only on Unix. Migrations are append-only and refuse a database from a newer version.

## Compatibility with the Rust app

- Same data folders, same `tinydash.db` schema, and same `settings.json` keys. A downgrade to the Rust app loses nothing.
- Stored shortcuts stay in Tauri syntax. `settings.Accelerator` translates them to mygo accelerators when they are registered.
- `testdata/golden` holds the Rust app's answers, frozen when the Rust code was removed. Differences are listed in `testdata/golden/divergences/<feature>.tsv`. `golden.CheckRanked` compares ranked lists up to the order of equal scores. In Go, equal scores keep their input order.
- Windows app IDs changed: mygo lists apps as `shell:AppsFolder\<ID>`, and the Rust app used Start menu shortcut paths. The pins, usage, aliases, and hotkeys of Windows apps reset once.

## Decided

| Question                       | Choice                                                  | Rejected, and why                                                                                                                 |
| ------------------------------ | ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Action exhaustiveness          | `Handler` interface: the compiler asks                  | AST test or the `exhaustive` linter: a test or a tool where the compiler suffices                                                 |
| State ownership                | Atomic snapshot plus one writer goroutine               | `RWMutex` per index as in Rust: ordering mutexes and check-then-write races; an actor that also serves reads: a hop per keystroke |
| Search thread                  | Synchronous, with a benchmark gate                      | Latest-wins goroutine: stale-reply logic and async tests, for a cost not yet measured                                             |
| Refresh coalescing             | Channel of capacity 1 per slot                          | Busy and requested atomics: three flags whose combinations need a model to trust                                                  |
| Source and answer registration | Explicit tables                                         | `init()` self-registration: hides the order that the goldens pin                                                                  |
| Import rules                   | Stdlib test over `go list -json` and `go/parser`        | `depguard`: another tool for the same check                                                                                       |
| SQLite                         | mygo `plugins/sqlite` (purego, SHA-256-checked library) | `mattn/go-sqlite3` needs cgo; `modernc.org/sqlite` is a large new dependency                                                      |
| UI                             | mygo native UI                                          | Webview + Solid: keeps a JS toolchain and an IPC layer                                                                            |
| Ties in ranking                | Equal scores keep their input order (stable sort)       | A port of Rust's unstable selection: its tie order was an artifact, and the port cost 230 lines                                   |
| Windows app list               | mygo's `InstalledApps`, with a one-time reset           | A port of the Rust Start menu scan: a second Windows app scan only to keep the old IDs                                            |

## Security

- No user text reaches a shell. External programs run with fixed arguments; only `mdfind` gets the typed name, as one argument after `-name`.
- Clipboard history is opt-in. It skips copies marked secret, and on macOS, copies made in Passwords or Keychain Access. It is never sent anywhere. While history is off, the monitor reads no content.
- The only network request of TinyDash is for the ECB rate table from Frankfurter, while currency rates are on (off by default).
- `tinydash.db` is owner-only on Unix, because it can hold private clipboard text.
