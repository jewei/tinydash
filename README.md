# TinyDash

A desktop launcher for macOS, Windows, and Linux, built with Rust, Tauri, and SolidJS.

TinyDash source is available under the [MIT license](LICENSE). The first release
will be free to download and use.

Search applications, filenames, and clipboard history. Calculate values, convert units and currencies, find emoji, generate passwords, convert dates and times, clean URLs, and open web searches.

## Start

TinyDash is in development. Use a build from [GitHub Actions](https://github.com/jewei/tinydash/actions/workflows/check.yml), or build it locally. Read the [installation guide](docs/how-to/install.md) for package types and signing limits.

To build from source, install Bun 1.4.2, Rust 1.98.1 or later, and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
bun install --frozen-lockfile
bun run tauri dev
```

Press **Control + Shift + Space** to show or hide the launcher. On native Wayland, assign a desktop shortcut to `tinydash`.

## Documentation

- [First session](docs/tutorials/first-session.md)
- [All features](docs/reference/features/README.md)
- [Keyboard shortcuts](docs/reference/keyboard-shortcuts.md)
- [Build and develop](docs/how-to/build.md)
- [Verify a change](docs/how-to/verify.md)
- [Architecture](docs/explanation/architecture.md)
- [Documentation index](docs/README.md)

## Development checks

```sh
bun run verify
bun run verify:full
```

The fast check runs repository checks, Prettier and Rust formatting checks, type checks, and browser smoke tests. The full check also runs all browser tests, Rust tests, Clippy, and the frontend build. Native desktop checks use a separate command and test session.

## Third-party material

Dependencies keep their own licenses. The bundled [Figtree](public/fonts/Figtree-OFL.txt)
and [Caprasimo](public/fonts/Caprasimo-OFL.txt) fonts use the SIL Open Font License.
The [EFF word list](src-tauri/src/providers/tools/data/README.md) uses Creative
Commons Attribution 4.0.
