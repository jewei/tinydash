# Repository Guidelines

## Project structure and modules

TinyDash is a desktop launcher built with SolidJS, TypeScript, Rust, and Tauri 2.

- `src/` contains frontend views and components. Styles use `tokens.css` and `src/styles/`.
- `src-tauri/src/` contains `launcher/`, search `providers/`, `ranking/`, SQLite `db/`, and OS-specific `platform/` code.
- `tests/` contains Playwright tests, a mock backend, and fixtures. `tests/native/` contains desktop checks.
- `public/fonts/` and `src-tauri/icons/` contain assets. `scripts/` contains development tools. `docs/` contains project documentation.

## Build, test, and development commands

Install Bun 1.4.2, Rust 1.98.1 or later, and the platform dependencies in the [build guide](docs/how-to/build.md).

- `bun install --frozen-lockfile` installs JavaScript dependencies.
- `bun run hooks:install` installs repository Git hooks.
- `bun run tauri dev` starts the desktop app. `bun run dev` starts only the browser frontend.
- `bun run build` checks TypeScript and builds the frontend.
- `bun run tauri build` creates desktop packages for the host OS.
- `bun run verify` checks repository files, formatting, types, and browser smoke tests.
- `bun run verify:full` adds all browser tests, Rust tests, Clippy, and the frontend build. Run this before merge.

## Coding style and naming

Use two spaces in TypeScript, TSX, and CSS. Use double quotes and semicolons in TypeScript. Use PascalCase for components and types, camelCase for TypeScript functions, and snake_case for Rust functions and modules. Rust uses four spaces. Keep TypeScript strict.

Format with `bun run format` and `cargo fmt --manifest-path src-tauri/Cargo.toml`. Keep search logic and OS operations in Rust.

## Testing guidelines

Name Playwright files `tests/<feature>.spec.ts`. Keep Rust tests in `#[cfg(test)]` modules or existing `*_tests.rs` files. No numeric coverage target is configured. Test changed behavior.

Install Chromium with `bunx --bun --no-install playwright install chromium`. Run focused checks with `bun run test:ui tests/pins.spec.ts` or `bun run test:rust`.

Browser tests use mocked IPC. Follow the [verification guide](docs/how-to/verify.md) for desktop checks and platform limits.

## Commit and pull request guidelines

Follow the commit history, for example `fix(search): prioritize apps` or `docs: update verification`. Keep commits focused.

Explain changes, link related issues, and record test commands and results. Include screenshots for UI changes. For desktop checks, record the commit, OS, and checks not run. Update affected feature documentation.

Keep private research, plans, and evidence in ignored folders such as `.local/` and `test-results/`.
