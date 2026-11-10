# TinyDash

A small, keyboard-first launcher for macOS, Windows, and Linux. Press a shortcut, type a few letters, and press Enter.

- **Find** apps, files, clipboard history, snippets, quicklinks, emoji, and system commands.
- **Answer** calculations, unit and currency conversions, dates, time zones, passwords, and clean URLs.
- **Stay private:** everything runs on your computer. The packaged app downloads only the daily currency rate table, and only after you turn it on. A run from source also downloads the SQLite library once.

TinyDash is written in Go with [mygo](https://github.com/egoist/mygo) native UI. Versions up to 0.9.0 were a Rust and Tauri app; the Go app reads their settings and data.

Press **Control+Shift+Space** to open TinyDash. See [all features and shortcuts](docs/features.md).

## Compared with the Rust app

The Go app draws its window natively. The Rust app drew a web page in a WebView, which ran four WebKit helper processes beside it. Without them, the Go app starts faster and uses less memory and CPU:

| Measure                          | Go app      | Rust app 0.9.0                  | Change               |
| -------------------------------- | ----------- | ------------------------------- | -------------------- |
| Time to index 131 apps at start  | 0.21–0.24 s | 0.34–0.45 s                     | about 40% faster     |
| Memory (macOS footprint)         | 36 MB       | 93 MB (26 MB + 67 MB WebKit)    | about 60% less       |
| Memory (resident)                | 108 MB      | 181 MB (96 MB + 85 MB WebKit)   | about 40% less       |
| CPU while idle, over 60 s        | 0.15 s      | 0.31 s (0.02 s + 0.29 s WebKit) | about half           |
| Search, p95 per query            | 0.12–5.0 ms | 0.12–4.4 ms                     | up to about 1.9 times slower |
| App size, universal macOS        | 35 MB       | 22 MB                           | larger               |
| Download (.dmg), universal macOS | 11.2 MB     | 8 MB                            | larger               |

Both apps stay within the 8 ms search budget. The search time leaves out drawing: in the Rust app, each keystroke also went to the WebView and back.

How it was measured, on an Apple M2 Mac with macOS 27.2, on 2026-11-10: each app started hidden, three times, with the same settings and data. The Rust app was the installed 0.9.0 release; the Go app was built from this repository. Memory was read 30 s after start, and idle CPU from 10 s to 70 s, with the Rust app's WebKit helpers counted. Search used the same 5,000 apps, 50,000 files, and queries in both apps: `BenchmarkSearch` in `internal/search`, and the same data in a release build of the Rust code.

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
