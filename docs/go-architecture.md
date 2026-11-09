# TinyDash on mygo: architecture

The design of the Go port. It replaces `docs/architecture.md` when the Rust code goes. Three independent design sketches agreed on most of it; the choices where they differed are listed under "Decided".

## Packages

Module `github.com/jewei/tinydash`. Arrows are the only imports allowed; `internal/archtest` fails the build on any other.

| Package                                        | Owns                                                                                                                                                                              | May import                                                                |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `cmd/tinydash`                                 | `main`: single-instance lock, CLI, `app.Run`                                                                                                                                      | `app`                                                                     |
| `internal/core`                                | `Result`, `Kind`, `Icon`, `Action` (sealed), `Source` and its traits table, `ID`, `Category`, personal lists (usage, pins, hidden, aliases, hotkeys)                              | stdlib only                                                               |
| `internal/match`                               | Normalize, name, word, and alias scoring                                                                                                                                          | `core`, stdlib                                                            |
| `internal/weburl`                              | WHATWG URL parse, write, and check, shared by `cleanurl` and `library`                                                                                                            | stdlib minus OS, network, and clock                                       |
| `internal/feature/<name>`                      | One pure feature each: `apps`, `files`, `clipboard`, `library`, `emoji`, `system`, `calc`, `currency`, `datetime`, `password`, `cleanurl`, `color`, `unixtime`, `web`, `keywords` | `core`, `match`, `weburl`, stdlib minus OS, network, and clock            |
| `internal/search`                              | `Snapshot`, `Search`, `Browse`, `Resolve`, the source table and the answer list                                                                                                   | `core`, `match`, `feature/*`, `settings`                                  |
| `internal/settings`                            | `settings.json` model, tolerant load, `Normalize`, save, unknown-key passthrough, Tauri accelerator translation                                                                   | `core`, stdlib                                                            |
| `internal/store`                               | All SQL and the append-only migrations                                                                                                                                            | `core`, `feature/*` (row types), `mygo/plugins/sqlite`                    |
| `internal/platform`                            | Only the OS work that mygo lacks. The only package with GOOS files                                                                                                                | `core`, purego, `x/sys`, `mygo`                                           |
| `internal/engine`                              | Live state: atomic snapshot, the single writer, refresh slots, the clipboard monitor, action checks and execution                                                                 | everything above except `platform`, which it reaches through an interface |
| `internal/ui/launcher`, `internal/ui/settings` | Native views and their models                                                                                                                                                     | `core`, `search`, `settings`, `mygo/ui`, the engine's read API            |
| `internal/app`                                 | Wiring: panel, settings window, tray, global shortcut, hotkeys, updater, watchers                                                                                                 | all                                                                       |

## Rules the tests enforce

1. `feature/*`, `match`, `weburl`, and `core` import no `os`, `os/exec`, `net/...`, `syscall`, `unsafe`, or mygo package, and do not call `time.Now`. Disk, network, clock, and randomness come in as arguments: `fs.FS`, `io.Reader`, `time.Time`.
2. Only `store` imports the sqlite plugin.
3. Only `platform` has `_darwin.go`, `_windows.go`, or `_linux.go` files or OS build tags.
4. `ui/*` never imports `store`, `platform`, `engine` internals, or the mygo root package.
5. Only `cmd/tinydash` imports `app`.
6. `go.mod` requires only allowlisted modules.
7. Every shipped migration keeps its SHA-256.
8. Every `Source` has a traits row and a frozen ID prefix.

## Data flow and concurrency

- **Snapshot.** `engine` holds `atomic.Pointer[search.Snapshot]`. Readers load it and never lock. A snapshot is never changed after it is published.
- **Single writer.** One goroutine makes every change: actions, settings, saved clips, swapped-in indexes. A caller sends `func(*next) error`. The writer checks limits, writes the store, publishes a shallow copy with `Gen+1`, and calls `OnChange`. The main thread never waits on the writer.
- **Search** runs synchronously on the main thread when the query, the category, or `Gen` changes. A benchmark test holds it under 8 ms at p95 with 5,000 apps and 50,000 files. If it misses, `Search` fans out inside; the API stays.
- **Refresh slots.** Each scanned source has one goroutine that ranges over a channel with capacity 1. `Request` is a non-blocking send, so requests coalesce, one scan runs at a time, and a request during a scan causes one more scan.
- **Clipboard monitor.** A 500 ms tick reads the OS change counter, reads the content only when it changed, and sends `saveClip(epoch, content)` to the writer. Clear and history-off bump the epoch, and the writer drops a save with an old epoch.
- **View updates.** `OnChange` calls `panel.Update(launcher.Stale)`, which searches again and keeps the selection by ID.

TLA+ models (with `/create-formal-verification-skill`) cover the two places where order of events decides correctness: refresh slots with `Ready`, and clipboard capture against Clear and history-off.

## Actions

`core.Action` is a sealed interface with `Dispatch(Handler) error` and `CountsAsUse() bool`. `Handler` has one method per action type, so a new action does not compile until `engine` handles it. `engine` checks every action again before it runs it.

## UI

Views read a model and call methods on it. Key bindings are one table in `keymap.go`; the commands they map to are plain methods with table tests. The actions menu, the alias dialog, and the hotkey dialog are overlays drawn in the panel, so the nonactivating panel keeps the keyboard. Tests drive views with `ui.NewTester` against a fixture snapshot and a fake platform, and move time with `Advance`, never `Sleep`.

## Compatibility with the Rust app

- Same data folders (the Tauri identifier `dev.tinydash.launcher`), same `tinydash.db` schema, same `settings.json` keys. Keys of dropped features pass through unchanged, so a downgrade loses nothing.
- Stored shortcuts stay in Tauri syntax and are translated at the edge.
- `testdata/golden` pins the Rust app's answers; differences are listed in `divergences/<feature>.tsv`.

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
