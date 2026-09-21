# Build and develop

Install Bun 1.4.2, Rust 1.98.1 or later, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/). macOS needs Xcode command line tools. Windows needs the C++ build tools and WebView2. Linux needs GTK and WebKitGTK development libraries.

## Start the desktop app

```sh
bun install --frozen-lockfile
bun run tauri dev
```

Bun runs the JavaScript tools. Node.js is not required. The package files and lockfiles define the dependency versions. `@types/node` provides declarations for APIs that Bun supports.

The development server uses `127.0.0.1:1420`. Quit an existing TinyDash process before starting another build. Hiding the launcher leaves that process running.

`bun run dev` starts only the browser frontend. Application discovery and other desktop functions need the Rust backend. Browser tests supply their own mock backend.

## Build a package

```sh
bun run tauri build
```

Build on each target operating system. Packages appear in `src-tauri/target/release/bundle/`. For a local macOS app without a DMG, use `bun run tauri build --bundles app`.

Use [verification](verify.md) before submitting a change. Use [release preparation](release.md) for signing and update configuration.

## Keep local work private

Use `.local/research/`, `.local/benchmarks/`, and `.local/plans/` for private work. `designs/` and `.hallmark/` are also ignored. The build and tests must work without these folders.

Run `bun run check:repo` to check for private paths and broken documentation links. This check also runs in `bun run verify` and CI. See [repository checks](verify.md#check-repository-paths-and-history) to check staged files or commit history.
