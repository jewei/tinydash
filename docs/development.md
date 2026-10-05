# Development

## Set up

1. Install [Bun](https://bun.sh) 1.4.2, Node.js 24 (Vite+ runs its tools on Node), and Rust through [rustup](https://rustup.rs). `rust-toolchain.toml` selects the Rust version.
2. Install the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS. On Ubuntu:
   ```sh
   sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
     librsvg2-dev libxdo-dev libssl-dev
   ```
3. `bun install`. This also installs the pre-commit hook, which formats staged files.

## Daily commands

| Command          | What it does                                                                                                              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `bun run dev`    | Runs the app with hot reload as "TinyDash Dev", with its own data (see below).                                            |
| `bun run web`    | Runs only the Vite server, for styling; IPC calls fail without Tauri.                                                     |
| `bun run check`  | `vp check` (Oxfmt, Oxlint, TypeScript), `cargo fmt --check`, Clippy.                                                      |
| `bun run test`   | `vp test` (Vitest + jsdom) and `cargo test`. `cargo test` also writes `src/generated`.                                    |
| `bun run verify` | `check`, `vp test`, then `scripts/bindings.ts`: Rust tests, failing if they changed `src/generated`. Run before every PR. |
| `bun run fix`    | Formats TypeScript, CSS, JSON, Markdown, and Rust.                                                                        |
| `bun run build`  | Builds installers for this OS into `src-tauri/target/release/bundle`.                                                     |

Logs go to stderr. Set `RUST_LOG=tinydash_lib=debug` for more.

## Run the app without touching your own TinyDash

The app identifier decides the data folder and the single-instance lock. `bun run dev` uses `src-tauri/tauri.dev.conf.json`, which sets the identifier `dev.tinydash.dev`, so it never reads or writes an installed TinyDash's settings and history, and does not hand over to it. Its data is in the `dev.tinydash.dev` folder (on macOS, `~/Library/Application Support/dev.tinydash.dev`); delete that folder to start fresh.

To try a build with the frontend bundled in, without hot reload:

```sh
bunx tauri build --debug --no-bundle --config src-tauri/tauri.dev.conf.json
./src-tauri/target/debug/tinydash            # shows the launcher
./src-tauri/target/debug/tinydash --settings # opens Settings
```

If another TinyDash holds the global shortcut, run the binary again to toggle the launcher.

## Tests

- **Rust:** unit tests in each module. Platform tests in `platform/macos.rs` read the real system (apps, icons) without changing it.
- **Frontend:** `*.test.ts(x)` next to the code. Component tests render real components against `src/test/backend.ts`, a fake backend built on `@tauri-apps/api/mocks`. Assert on what the user sees (roles, labels) and on the commands sent.

Automated tests do not cover the OS effects. Check these by hand on a real desktop when you change them, with throwaway data:

- The global shortcut shows and hides the launcher; Escape returns focus to the previous app.
- With clipboard history on, copied text appears; a password-manager copy does not; Enter copies an entry back.
- An app launches; a file opens; Show in Finder/Explorer selects it.
- Lock Screen works. Cancel the confirmation for Restart, Shut Down, Log Out, and Empty Trash.
- Settings: record a shortcut, toggle open at login and the tray icon.

## Change the IPC contract

1. Change the Rust type or command (`commands.rs`, `actions.rs`, `search/result.rs`, ...).
2. Run `bun scripts/bindings.ts`. It rebuilds `src/generated` from scratch (removing bindings of deleted types) and fails once to show that it changed.
3. Fix the type errors that `bun run check` reports in the frontend, and update `src/lib/ipc.ts` for new commands.
4. Commit the generated files with the change.

## Dependencies

- Pin exact versions in `package.json`. `bun install --frozen-lockfile` must pass.
- Vite+ pins `vite` and `vitest` through `overrides`, and Dependabot ignores all three. To upgrade, set the new `vite-plus` version in `package.json`, run `bun install`, then `bunx vp migrate`; on a project already on Vite+ it only re-pins `vite` and `vitest` to match.
- Rust crates follow Cargo semver; commit `Cargo.lock`. Change the Rust version in `rust-toolchain.toml` and fix new Clippy findings in the same PR.

## Bundled data

`src-tauri/data/README.md` lists sources and licenses. Regenerate emoji keywords with `bun run emoji-data`; the script checks the SHA-256 of each source file.

## Release

1. Bump `version` in `src-tauri/Cargo.toml`; Tauri reads the app version from it.
2. Merge to `main`, then push a tag such as `v0.2.0`.
3. The Release workflow checks that the tag matches the version, builds a universal macOS app and DMG (Apple silicon and Intel), the Windows installer, and the Debian package, and attaches them to a draft GitHub release. Builds are not code-signed; macOS uses an ad-hoc signature.
4. Review the draft and publish it.
