# Development

## Set up

1. Install Go 1.27.3 or later (`go.mod` sets the version).
2. On Linux, install the libraries a run needs: `sudo apt install libgtk-3-0 libayatana-appindicator3-1`. The window needs GTK (`libgtk-3-0`; Ubuntu 24.04 provides it as `libgtk-3-0t64`). The tray icon needs `libayatana-appindicator3-1`. Without that library TinyDash still starts, and the launcher says the install command. Compiling and packaging do not need the header packages `libgtk-3-dev` and `libayatana-appindicator3-dev`. CI installs the same two runtime libraries.
3. Nothing else. `go.mod` pins the tools, and `go tool` runs them: `go tool mygo` (the mygo CLI) and `go tool staticcheck`. TinyDash needs no cgo.

For the formal checks, install Java 11 or later. For the `verify-tinydash` skill, install Docker or OrbStack.

## Daily commands

| Command                                        | What it does                                                                                                                                 |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `bash scripts/verify-go.sh`                    | Every gate: `go vet` and staticcheck as macOS, Windows, and Linux see the code, `go test -race -count=1 ./...`, then the search speed gate without the race detector. Run it before every commit. |
| `go test ./internal/<pkg>`                     | The tests of one package.                                                                                                                    |
| `bash scripts/formal-verify.sh`                | Model-checks the TLA+ models in `formal/` (see "Formal checks").                                                                            |
| `go build ./cmd/tinydash`                      | Builds the binary for this OS.                                                                                                               |
| `go run ./cmd/tinydash --data-dir <folder>`    | Runs the app from source with its data in `<folder>` (see below).                                                                           |
| `go tool mygo build`                           | Builds the packaged app into `build/` (see "Packaging").                                                                                    |

The tests in `go test ./...` include the import rules and the gofmt check (`internal/archtest`). Logs go to stderr.

## Run the app without touching your own TinyDash

`--data-dir <folder>` keeps `settings.json` and `tinydash.db` in that folder, so a run from source never reads or writes your settings and history. Use a new, empty folder, and delete it to start fresh:

```sh
go run ./cmd/tinydash --data-dir "$(mktemp -d)"
```

`--hidden` starts without showing the launcher. Quit an installed TinyDash first: both would use the same launcher shortcut. A second start of the same build hands its options to the running one (with none, it toggles the launcher) and quits. `--settings`, `--mode <category>`, and `--background` (the same as `--hidden`) are the Rust app's options.

Without `--data-dir`, TinyDash uses the folders of the Rust app, which hold the user's real data. Never run it that way for development.

- **macOS:** `~/Library/Application Support/dev.tinydash.launcher` holds both `settings.json` and `tinydash.db`.
- **Linux:** `settings.json` is in `~/.config/dev.tinydash.launcher`. `tinydash.db` is in `~/.local/share/dev.tinydash.launcher`. An absolute `XDG_CONFIG_HOME` replaces `~/.config`, and an absolute `XDG_DATA_HOME` replaces `~/.local/share`. A relative `XDG_DATA_HOME` is ignored. A relative `XDG_CONFIG_HOME` is refused, and TinyDash does not start.
- **Windows:** `settings.json` is in `%AppData%\dev.tinydash.launcher`. `tinydash.db` is in `%LocalAppData%\dev.tinydash.launcher`.

## Tests

- Tests sit next to the code, in the same package. Pure packages get disk, clock, and randomness as arguments, so their tests need no OS.
- Write paths in tests as slash paths through `golden.Abs`. It turns `/Users/me` into `C:\Users\me` on Windows.
- Views are tested headless with mygo's `ui.NewTester` and a fake backend. Tests move time with `Advance`, never `Sleep`.
- The tests run on this machine's OS only. CI (`.github/workflows/ci.yml`) runs `verify-go.sh` on macOS, Windows, and Linux. After you push, wait for `gh pr checks <branch> --watch`.
- The Coverage workflow runs the tests on Linux with coverage, and writes the share of code that they run to its job summary. It fails when a test fails, and its log shows the test output. Branch protection does not require it. To see it locally: `go test -coverprofile=coverage.out ./...`, then `go tool cover -func=coverage.out`.
- Search has a budget of 8 ms at p95, with 5,000 apps and 50,000 files. `TestSearchStaysWithinBudget` fails above 16 ms, which leaves room for slower CI machines; `verify-go.sh` runs it without the race detector. To see the numbers: `go test ./internal/search -run '^$' -bench .`.

Automated tests do not cover these OS effects. Check them by hand on a real desktop, with throwaway data, when you change them: the global shortcut and hotkeys, the panel focus, the tray icon, open at login, the system commands, Spotlight results, clipboard capture of images and files, and the Up Next widget.

On macOS, Up Next reads and asks for calendars and reminders only when TinyDash is its own responsible process, and only when its Info.plist has the usage strings. macOS checks the access and the usage strings of the responsible process, and a program that a terminal starts belongs to the terminal. So `go run`, started from a terminal, never asks: the widget says that another app opened TinyDash. To test Up Next, quit the installed TinyDash, build the app, and open it with `open`, which starts it through launchd:

```sh
go tool mygo build
open build/darwin-arm64/TinyDash.app --args --data-dir "$(mktemp -d)"
```

Turn on Up Next in Settings > Widgets; macOS then asks for access as TinyDash. The local build is ad hoc signed, so macOS may ask again for the installed app.

## Golden corpus

`testdata/golden` holds what the Rust app answered for each query, at a fixed clock, time zone, and set of rates. It also holds fixtures of the Rust app's `settings.json` loads and a `tinydash.db` that the Rust store wrote. The Rust code is gone, so the corpus is frozen: `testdata/golden/README.md` says how to write it again from the last commit that has the Rust code.

A Go answer that differs from the corpus fails its test, unless `testdata/golden/divergences/<feature>.tsv` lists the query with the reason. Ranked lists compare up to the order of equal scores (`golden.CheckRanked`).

## Formal checks

`formal/ClipboardHistory.tla` models clipboard capture against Clear and history on/off. `bash scripts/formal-verify.sh` downloads TLC 1.7.4 once into `~/.cache/tinydash` (or `$XDG_CACHE_HOME/tinydash`), checks its SHA-256, and runs each model. Each config is `<Spec>.<variant>.cfg`. A config whose name holds `.broken` models a known bug: it must fail on the invariant that its `\* must break: <Invariant>` line names, which shows that the check can catch the bug. Every other config must pass. CI runs it on Linux. Before you change clipboard capture, Clear, or history on/off, use the `formal-verify-clipboard` skill.

## Drive the real app: the verify-tinydash skill

The `verify-tinydash` skill (`.claude/skills/verify-tinydash/`) builds TinyDash with `-tags e2e` for Linux, starts it in a Docker container on a virtual X display with seeded data, and drives it through mygo's automation socket. It saves evidence (window captures, element trees, `tinydash.db` rows, the X clipboard) in `.local/verify/runs/`. It never touches your desktop, your clipboard, or your TinyDash data. Read its `SKILL.md` before a run.

It needs Docker or OrbStack. The first launch downloads mygo's SQLite library from GitHub.

## Packaging

`go tool mygo build` reads `mygo.json` (name, identifier `dev.tinydash.launcher`, version, `LSUIElement` so that macOS shows no Dock icon, the usage strings that Up Next needs to ask for calendars and reminders, and `Entitlements.plist`, which lets the hardened runtime of the signed release read them) and `resources/icon.png`. On macOS it writes `build/darwin-arm64/TinyDash.app`, ad hoc signed, and `tinydash-<version>-arm64.dmg`, and bundles mygo's SQLite library in the app. `-platform` picks other targets, such as `darwin/universal,windows/amd64,linux/amd64`. Signing and disk images need macOS. `go tool mygo build -h` lists every flag.

On Linux, `mygo.json` sets `linux.webview` to false, so the `.deb` and the Arch `PKGBUILD` depend on GTK and not on WebKitGTK: every window draws with the native UI and never loads WebKit. `linux.depends` adds `libayatana-appindicator3-1` (Arch: `libayatana-appindicator`), which the tray icon loads. The tray is on by default, so the package depends on the library rather than recommending it. The app still starts if the library is removed later, and says how to install it. The desktop entry's `StartupWMClass` is the executable name, `tinydash`. TinyDash sets the window class to that same string after GTK starts, because GTK would otherwise capitalize it to `Tinydash` and a dock would not match the window to the icon.

The version is in `mygo.json`.

`app-icon.svg` is the source of `resources/icon.png`; the PNG cannot be scaled up without loss. To change the icon, edit the SVG, then render it at 1024 × 1024 px, for example with `rsvg-convert -w 1024 -h 1024 app-icon.svg -o resources/icon.png`.

## Dependencies

- `go.mod` lists the modules. `internal/archtest/rules.go` allowlists each module and the packages that may import it. A new module fails the tests until it is added there, also when only a test imports it.
- Dependabot opens one weekly PR for all Go modules and one for GitHub Actions.
- The Audit workflow runs `govulncheck` every Monday (or from Actions > Audit > Run workflow) and fails on a vulnerable code path.
- CodeQL scans the Go and workflow code on each pull request, on `main`, and every Monday. OpenSSF Scorecard grades the repository setup on `main` and every Monday. Both report to Security > Code scanning and never block a merge.

## Bundled data

- Emoji: `internal/feature/emoji/catalog.go`, generated from the Rust `emojis` crate 0.9.1 (Unicode 18.0), and the keyword files in `internal/feature/emoji/data/` (Unicode License, in `data/LICENSE`). Both are frozen: the generator was removed with the Rust app.
- Passwords: the EFF large wordlist, in `internal/feature/password/`.
- Time zones: `internal/zoneinfo/zoneinfo.zip`, a copy of `$(go env GOROOT)/lib/time/zoneinfo.zip` from Go 1.27.3 (IANA 2026c). To update the rules, copy that file from a newer Go and change the release in the comment in `zoneinfo.go`. Then run the tests: the datetime golden test shows any answer that the new rules change.
- Fonts: Figtree and Caprasimo in `internal/ui/kit/fonts/`, under the SIL Open Font License.

## Release

1. In a pull request, set `version` in `mygo.json`, and move the `[Unreleased]` notes in `CHANGELOG.md` under `## [<version>] - <date>`, with its compare link. Merge it, and wait for CI on `main`.
2. Run the Release workflow on `main`: `gh workflow run release.yml -f version=<version>`. It makes a draft by default (`-f draft=false` publishes at once). It checks the version, the changelog section, and CI. It builds macOS (universal, signed with the Developer ID, notarized), Windows (installer), and Linux (`.deb`, `.tar.gz` with `install.sh`), records each file's build provenance, and makes the release with the changelog section as its notes and `SHA256SUMS`.
3. Try the draft's downloads, then publish it on GitHub.
4. Publishing starts the Homebrew workflow (`homebrew.yml`), which points the cask in `jewei/homebrew-tap` at the new `.dmg`. To run it again: `gh workflow run homebrew.yml -f version=<version>`.

Signing uses the `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` repository secrets; the cask uses `HOMEBREW_TAP_TOKEN`. Local builds stay ad hoc signed: only the workflow adds `macos.notarize` to `mygo.json`. The app has no update feed, so there is no updater key.
