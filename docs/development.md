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

| Command          | What it does                                                                                                                          |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `bun run dev`    | Runs the app with hot reload as "TinyDash Dev", with its own data (see below).                                                        |
| `bun run web`    | Runs only the Vite server, for styling; IPC calls fail without Tauri.                                                                 |
| `bun run check`  | `vp check` (Oxfmt, Oxlint, TypeScript), `cargo fmt --check`, Clippy.                                                                  |
| `bun run test`   | `vp test` (Vitest + jsdom) and `cargo test`. `cargo test` also writes `src/generated`.                                                |
| `bun run verify` | `check`, `vp test`, `vp build`, then `scripts/bindings.ts`: Rust tests, failing if they changed `src/generated`. Run before every PR. |
| `bun run fix`    | Formats Rust, then TypeScript, CSS, JSON, and Markdown; Rust is formatted even when TypeScript has errors.                            |
| `bun run build`  | Builds installers for this OS into `src-tauri/target/release/bundle`.                                                                 |

Logs go to stderr. Set `RUST_LOG=tinydash_lib=debug` for more.

## Run the app without touching your own TinyDash

The app identifier decides the data folder and the single-instance lock. `bun run dev` uses `src-tauri/tauri.dev.conf.json`, which sets the identifier `dev.tinydash.dev`, so it never reads or writes an installed TinyDash's settings and history, and does not hand over to it. Its settings and data are in `dev.tinydash.dev` folders; delete them to start fresh. On macOS both are in `~/Library/Application Support/dev.tinydash.dev`. On Linux, settings are in `~/.config/dev.tinydash.dev` and data in `~/.local/share/dev.tinydash.dev`. On Windows, settings are in `%APPDATA%\dev.tinydash.dev` and data in `%LOCALAPPDATA%\dev.tinydash.dev`. Settings > About shows both.

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
3. Fix the type errors that `bun run check` reports in the frontend. For a new command, also register it in `generate_handler!` in `lib.rs`, add a wrapper in `src/lib/ipc.ts`, and add a default reply in `src/test/backend.ts`.
4. Commit the generated files with the change.

## Window configuration

`src-tauri/tauri.windows.conf.json` repeats the whole launcher window, because Tauri replaces arrays when it merges configs; it differs only in `"transparent": false`. Change both files together.

## App icons

`app-icon.svg` is the source. Regenerate with `bunx tauri icon app-icon.svg -o src-tauri/icons`, then keep only the files listed under `bundle.icon` in `tauri.conf.json`.

## Dependencies

- Pin exact versions in `package.json`. `bun install --frozen-lockfile` must pass.
- Vite+ pins `vite` and `vitest` through `overrides`, and Dependabot ignores all three. To upgrade, set the new `vite-plus` version in `package.json`, run `bun install`, then `bunx vp migrate`; on a project already on Vite+ it only re-pins `vite` and `vitest` to match.
- The Audit workflow checks `Cargo.lock` against the RustSec advisory database every Monday (or from Actions > Audit > Run workflow) and fails on a known vulnerability. It does not run on pull requests, so a new advisory never blocks unrelated work. To check locally: `cargo install cargo-audit --locked`, then `cargo audit --file src-tauri/Cargo.lock`.
- Dependabot updates the `tauri*` crates and the `@tauri-apps` packages only to new patch versions, because the Tauri CLI refuses to build when the `tauri` crate and the packages have different minor versions, and new plugins need the newest `tauri`. To move to a new minor version, change `tauri`, `tauri-build`, and the `tauri-plugin-*` crates in `src-tauri/Cargo.toml` and both `@tauri-apps` packages in `package.json` in one commit, then run `bun install`, `cargo check --manifest-path src-tauri/Cargo.toml` (which updates `Cargo.lock`), and `bun run verify`.
- Rust crates follow Cargo semver; commit `Cargo.lock`. Change the Rust version in `rust-toolchain.toml` and fix new Clippy findings in the same PR.

## Bundled data

`src-tauri/data/README.md` lists sources and licenses. Regenerate emoji keywords with `bun run emoji-data`; the script checks the SHA-256 of each source file.

## Release

Merge everything for the release to `main`, including a change that renames Unreleased in `CHANGELOG.md` to the new version and date, and wait for CI to pass. Then run:

```sh
bun run release 0.2.1           # publish
bun run release 0.2.1 --draft   # or make a draft, try the installers, then publish it on GitHub
```

The command starts the Release workflow on `main` and follows it to the end, which takes about 15 minutes. You can also start it from Actions > Release > Run workflow. The workflow:

1. Stops unless it runs on `main`, the tag `vVERSION` does not exist, and CI ("Required checks") passed on that commit.
2. Writes the version into `src-tauri/Cargo.toml` and `Cargo.lock` in each runner (`scripts/set-version.ts`). Both stay at `0.0.0` in the repository, so a release needs no commit, and a local build reports 0.0.0.
3. Builds a universal macOS DMG (Apple silicon and Intel), the Windows installer, and the Debian package. The macOS app is signed with the Developer ID and notarized, from the `APPLE_*` repository secrets: certificate, its password, signing identity, Apple ID, app-specific password, and team ID. The build fails unless Gatekeeper accepts the app as notarized. Windows and Linux builds are not code-signed.
   On macOS and Windows it also makes update files, signed with `TAURI_SIGNING_PRIVATE_KEY` (and its password), and builds the app with the feed address (`TINYDASH_UPDATE_ENDPOINT`, a repository variable) and the matching `TAURI_UPDATER_PUBLIC_KEY`. A local build has neither, so it never checks for updates.
4. Only when all three builds pass: tags the commit `vVERSION`, and publishes the release with the installers, the update files, `latest.json` (the update feed), `SHA256SUMS`, and notes made from the merged pull requests. Installed copies read `latest.json` from the latest published release, so a draft reaches no one, and publishing one offers it to everyone, including TinyDash 0.1.3 on Apple silicon and Windows, which reads the same address.

If a build fails, nothing is tagged or published: fix the cause, merge, and run the command again. To replace a published release, release a new version.
