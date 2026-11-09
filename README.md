# TinyDash

A small, keyboard-first launcher for macOS, Windows, and Linux. Press a shortcut, type a few letters, and press Enter.

- **Find** apps, files, clipboard history, snippets, quicklinks, emoji, and system commands.
- **Answer** calculations, unit and currency conversions, dates, time zones, passwords, and clean URLs.
- **Stay private:** everything runs on your computer. The packaged app downloads only the daily currency rate table, and only after you turn it on. A run from source also downloads the SQLite library once.

TinyDash is written in Go with [mygo](https://github.com/egoist/mygo) native UI. Versions up to 0.9.0 were a Rust and Tauri app; the Go app reads their settings and data.

Press **Control+Shift+Space** to open TinyDash. See [all features and shortcuts](docs/features.md).

## Install

The Go app has no release yet. The downloads on the [Releases page](https://github.com/jewei/tinydash/releases) and the Homebrew cask are the earlier Rust app. To use the Go app now, build it from source.

## Build from source

Install Go 1.27.3 or later. On Linux, also install `libgtk-3-dev` and `libayatana-appindicator3-dev`. Then:

```sh
go tool mygo build                                # the packaged app, into build/
go run ./cmd/tinydash --data-dir "$(mktemp -d)"    # try it with empty, throwaway data
```

On macOS, `go tool mygo build` makes `build/darwin-arm64/TinyDash.app` and a disk image. Without `--data-dir`, TinyDash uses your real settings and data.

## Documentation

- [Features](docs/features.md): what TinyDash does, its limits, and what changed from the Rust app.
- [Architecture](docs/architecture.md): how it works and why.
- [Development](docs/development.md): setup, checks, tests, and packaging.
- [AGENTS.md](AGENTS.md): rules for contributors, human or AI.

## License

MIT (see [LICENSE](LICENSE)). The bundled fonts, Figtree and Caprasimo, use the SIL Open Font License; see `internal/ui/kit/fonts`. The emoji keyword data uses the Unicode License; see `internal/feature/emoji/data/LICENSE`.
