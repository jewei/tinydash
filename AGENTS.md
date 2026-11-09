# TinyDash — agent guide

TinyDash is a keyboard-first desktop launcher for macOS, Windows, and Linux, written in Go on mygo with native UI. Read this file before you change anything. `docs/architecture.md` explains the design; `docs/rewrite-log.tsv` records each decision of the port from Rust.

## Commands

| Task                                | Command                                                                     |
| ----------------------------------- | --------------------------------------------------------------------------- |
| **Before every commit**             | `bash scripts/verify-go.sh`                                                 |
| Tests of one package                | `go test ./internal/<pkg>`                                                  |
| Model-check the TLA+ models         | `bash scripts/formal-verify.sh` (Java 11+)                                  |
| Drive the real app, with evidence   | the `verify-tinydash` skill (Docker or OrbStack; a Linux container)          |
| Packaged app for this OS            | `go tool mygo build` (into `build/`)                                        |
| Run from source                     | `go run ./cmd/tinydash --data-dir <scratch folder>`                         |

`verify-go.sh` runs vet and staticcheck as macOS, Windows, and Linux see the code, then `go test -race -count=1 ./...`, which includes the import rules and gofmt, then the search speed gate (p95 under 16 ms) without the race detector. It runs the tests on this machine's OS only. CI runs them on all three: after you push, wait for `gh pr checks <branch> --watch` before you call a change verified. Branch protection requires only the `Required checks` job; keep its name. A change to Markdown, `docs/`, or `LICENSE` alone skips the Go jobs in CI.

## Where things are

```
cmd/tinydash/          main: the command line (app.ParseArgs), then app.Main.
internal/
  app/                 Wiring: panel, Settings window, HUD, tray, shortcut and hotkeys,
                       file watcher, Spotlight, clipboard ticker, rates. Start at app.go.
  engine/              Live state: snapshot, single writer, slots, monitor, actions.go
                       (every action, checked before it runs), settingsfile.go.
  search/              Snapshot, Search, Resolve, the order of All, answers(), personal lists.
  core/                Result, Action and Handler, Source traits, Category, SystemCommand.
  feature/<name>/      One pure feature each (apps, files, clipboard, library, calc, ...).
  match/               Scoring (a port of nucleo).
  weburl/              The one WHATWG URL parser.
  zoneinfo/            Time zone rules built into the binary; never the disk's.
  settings/            settings.json model, load, Normalize, Tauri shortcut translation.
  store/               All SQL and the migrations.
  platform/            OS work that mygo lacks. The only package with _darwin/_windows/_linux files.
  ui/kit/              Keys, palette, fonts shared by both views.
  ui/launcher/         Launcher view and model; keymap.go holds every key.
  ui/settings/         Settings view and model.
  archtest/            The import rules as data (rules.go).
  golden/              Test support for the frozen Rust answers in testdata/golden.
formal/                TLA+ models.
testdata/golden/       The Rust app's answers, frozen. Read its README.md.
.claude/skills/        verify-tinydash, formal-verify-clipboard.
```

The package table in `docs/architecture.md` lists what each package owns and may import.

## Rules

Rules marked _(test)_ fail `go test` when broken.

1. **Logic lives in the pure packages.** `core`, `match`, `weburl`, `zoneinfo`, `feature/*`, `settings`, and `search` take disk, network, clock, and randomness as arguments (`fs.FS`, `io.Reader`, `time.Time`). They import no `os`, `net`, `syscall`, `unsafe`, or random source. They call no `time.Now`, and no standard function that reads the disk, such as `time.LoadLocation` (use `zoneinfo.Load`) or `filepath.Glob`. _(test: `internal/archtest`)_
2. **Each package imports only its layer.** The layers and the allowlisted modules are data in `internal/archtest/rules.go`. Change them there, in their own commit, with the reason. _(test)_
3. **OS code lives only in `internal/platform`.** Only it has OS file suffixes or OS build tags. Elsewhere, only a file named `labels.go` reads `runtime.GOOS`, to pick a label or a default. Add a platform function to all three OS files. _(test)_
4. **The compiler asks for each action.** `core.Handler` has one method per action, so a new action does not compile until `engine` handles it. `engine.Actions` checks each action again before it runs it. _(test: `internal/core`)_
5. **All SQL lives in `internal/store`.** Change the schema by appending a migration. Never edit a migration that has shipped. _(test: `TestMigrationsNeverChange`)_
6. **The user's data stays compatible.** Keep the `settings.json` keys, the `tinydash.db` schema, and the result ID prefixes. Unknown settings keys are written back unchanged. _(test: `internal/settings`, `internal/store`, `internal/core`)_
7. **Ported answers match the Rust app.** `testdata/golden` is frozen. A Go result that differs must be listed, with its reason, in `testdata/golden/divergences/<feature>.tsv`. Add new cases to the feature's own tests, not to the corpus. _(test)_
8. **Bound what you keep.** Every list, cache, and stored item has a limit, as a named constant in its package (`search.MaxPins`, `clipboard.MaxTextBytes`).
9. **No new dependency without need.** Prefer the standard library and mygo. A new module goes into `archtest.external` first. _(test)_
10. **No dead code.** No commented-out code, no TODOs, no unused symbols, no speculative abstractions. _(staticcheck, as each OS sees the code)_
11. **Test what you change.** Tests sit next to the code. Write paths in tests as slash paths through `golden.Abs` (`/Users/me` becomes `C:\Users\me` on Windows): a literal `/root` is not absolute on Windows. Drive views with mygo's `ui.NewTester` and move time with `Advance`.
12. **Concurrency that tests reach least has a model.** Before you change clipboard capture, Clear, or history on/off, use the `formal-verify-clipboard` skill.
13. **Never touch the user's real TinyDash data.** Run the app only with `--data-dir` and a scratch folder, or in the `verify-tinydash` container.
14. **Never drive the desktop with synthetic input.** No osascript, cliclick, xdotool, or CGEvent on the user's machine, and no reads or writes of the user's clipboard. Test views with `ui.NewTester`. Drive the real app only in the `verify-tinydash` container.

## Add a feature

Each step names what fails when it is missing. Steps without a check need care.

- **New search source:**
  1. Add a `Source` constant before `sourceCount` and its row in `sourceTraits` (`internal/core/source.go`). Add its prefix to `TestSourcePrefixesNeverChange`, which fails until then.
  2. Write `internal/feature/<name>` with `Search`, `Browse`, and `Get`, as `feature/apps` does.
  3. Add its index to `search.Snapshot`. Wire it into `find`, `browse`, `resolve`, and the `sources` list of `all` in `internal/search/search.go`.
  4. For a new tab, add a `Category` to `core.Categories()` and its label to `categoryLabels` in `ui/launcher/glyphs.go` and `ui/settings/view.go` (no test checks the labels). `Normalize` adds the tab to saved settings.
  5. A source that a scan fills gets an `engine.Slot` in `internal/app/app.go`, requested at startup, in `Show` when old, and in `appShell.Refresh`.
- **New instant answer:** write `Answer(query, …) (core.Result, bool)` in a feature package. Call it from `answers()` in `internal/search/search.go`; its place in that list is its place in All. A new `core.Kind` needs a row in `kinds` in `ui/launcher/glyphs.go`. A new keyword gets a row in `builtIns` in `feature/library/keywords.go`, whose example must get an answer _(test)_.
- **New action:** add the type, `Dispatch`, `CountsAsUse`, and `Name` in `internal/core/action.go`, and a method to `Handler`. The build fails until `engine`'s `handler` and the test `recorder` have the method. Add it to `actions` and `countsAsUse` in `core_test.go`, which count the `Handler` methods _(test)_.
- **New setting:** add a field with its JSON key to `settings.Settings`, its default to `Default`, and a clamp to `Normalize`. Add a reader to `fields` in `internal/settings/load.go`; `TestDefaultsLoadBackUnchanged` fails without it unless the default is the zero value. A setting that changes the OS (shortcut, login item, tray) applies in `applyToSystem` in `internal/app/settings.go`, which the engine calls again to roll back a failure. Add its control in `internal/ui/settings/view.go`.
- **New system command:** add a `SystemCommand` and list it in `SystemCommands()`. Add its row in `feature/system/system.go` _(test)_ and its plan in each `internal/platform/system_*.go` _(test on each OS; CI runs all three)_.
- **New widget:** add its switch as a setting (above). Add its data to `launcher.Widgets` (`ui/launcher/backend.go`), fill it in `loadWidgets` (`internal/app/widgets.go`) with pure logic in `feature/` and OS reads in `platform/`. Draw it in `ui/launcher/widgets.go` and add it to `showsWidgets` in `ui/launcher/model.go`. Add its toggle to the Widgets section of `ui/settings/view.go`.

Then update `docs/features.md`, add the change under Unreleased in `CHANGELOG.md`, and run `bash scripts/verify-go.sh`.

## Style

- Names say what a thing is or does: `engine.Slot`, `Actions.RunHotkey`, `blurWasThisPress`. No abbreviations except common ones (`id`, `url`).
- Comments explain why, not what. An `unsafe` pointer cast has a `// SAFETY:` comment that says why it holds.
- User-facing text: short sentences, active voice, no jargon. Errors say what happened and what to do. They are sentences, so staticcheck's ST1005 is off (`staticcheck.conf`).
- gofmt, `go vet`, and staticcheck, through `verify-go.sh`.

## Commits and pull requests

- Conventional commits: `feat(search): …`, `fix(clipboard): …`, `docs: …`. One logical change per commit.
- Chain the gate and the commit with `&&`, never `;` or a pipe that hides the exit code.
- A change users notice gets a line under Unreleased in `CHANGELOG.md`, in plain words.
- The repository has no pre-commit hook. If a commit fails in an old Vite hook, `core.hooksPath` still points at it: tell the user, and leave the git config as it is.
- A PR explains the change, lists the commands you ran with their results, and includes screenshots for UI changes.

Keep private notes, plans, and evidence in `.local/` (ignored by git).
