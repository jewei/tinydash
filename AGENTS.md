# TinyDash — agent guide

TinyDash is a keyboard-first desktop launcher for macOS, Windows, and Linux. The backend is Rust on Tauri 2; the frontend is SolidJS. Read this file before you change anything. Details live in `docs/`.

## Commands

| Task                                    | Command                                                  |
| --------------------------------------- | -------------------------------------------------------- |
| Install dependencies                    | `bun install`                                            |
| Run the desktop app (hot reload)        | `bun run dev`                                            |
| Format, lint, and type-check everything | `bun run check`                                          |
| Run all tests                           | `bun run test`                                           |
| Fix formatting                          | `bun run fix`                                            |
| **Before every commit or PR**           | `bun run verify`                                         |
| One Rust test                           | `cargo test --manifest-path src-tauri/Cargo.toml <name>` |
| One frontend test file                  | `bunx vp test src/launcher/keymap.test.ts`               |

`bun run verify` runs `check`, `test`, and fails if `src/generated` is stale. CI runs the same steps on macOS, Windows, and Linux.

## Where things are

```
src-tauri/src/
  lib.rs               App setup and the list of IPC commands. Start here.
  commands.rs          Every IPC command. TypeScript wrappers: src/lib/ipc.ts.
  actions.rs           The Action enum and what each action does to the OS.
  search/              Query → ranked results. Pure: no Tauri, no I/O.
  features/            One file per feature (apps, files, clipboard, emoji, ...). Pure.
  platform/            macOS, Windows, Linux. Each file implements the contract in platform/mod.rs.
  store.rs             SQLite schema, migrations, and all SQL.
  settings.rs          settings.json model and defaults.
  state.rs             In-memory state; indexes are swapped whole (shared.rs).
  refresh.rs           When indexes and exchange rates rebuild.
  watcher.rs           File system events → "index is dirty".
  monitor.rs           Clipboard capture thread.
  window.rs, tray.rs, shortcut.rs, images.rs, events.rs, system_clipboard.rs
src/
  launcher/            Launcher window: state.ts (logic), Launcher.tsx (view), keymap.ts.
  settings/            Settings window.
  ui/                  Components shared by both windows.
  lib/ipc.ts           The only module that calls the backend.
  generated/           TypeScript types written by ts-rs. Never edit by hand.
  test/backend.ts      Fake backend for component tests.
docs/                  architecture.md, features.md, development.md
```

## Rules

1. **Logic lives in Rust.** Search, ranking, validation, and OS work are Rust. The frontend renders results and maps keys to actions; it never decides what an action does.
2. **Feature modules are pure.** Files in `features/` and `search/` never call Tauri, SQLite, the clipboard, or the OS. Glue modules at the top level do that.
3. **Results carry their actions.** A `SearchResult` lists `ResultAction`s; the frontend sends the chosen `Action` back. `actions.rs` checks every action again before it runs.
4. **One source of truth for IPC types.** Add `#[derive(TS)] #[ts(export)]` to Rust types that cross IPC. `cargo test` regenerates `src/generated`; commit the result. Add each new command to `lib.rs`, `commands.rs`, and `src/lib/ipc.ts`.
5. **All SQL lives in `store.rs`.** Change the schema by appending to `MIGRATIONS`. Never edit a migration that has shipped.
6. **Platform code lives only in `platform/`.** Add a function to all three OS files and document it in `platform/mod.rs`.
7. **Bound what you keep.** Every list, cache, and stored item has a limit (see the constants in each feature).
8. **No new dependency without need.** Prefer the standard library and existing crates. Pin exact versions in `package.json`.
9. **No dead code.** No commented-out code, no TODOs, no unused exports, no speculative abstractions.
10. **Test what you change.** Rust tests sit in a `tests` module in the same file. Frontend tests sit next to the component and use `src/test/backend.ts`.
11. **Never touch the user's real TinyDash data** when you run the app. Use the isolated build in `docs/development.md`.

## Add a feature

- **New search source:** add `features/<name>.rs` with `search`, `browse`, and `get` (see `features/apps.rs`). Wire it into `search/mod.rs`: `all()`, `search()`, `browse()`, `resolve()`, and `Category` if it gets a tab.
- **New instant answer** (computed from the query, like the calculator): add an `answer(query)` function and call it from `answers()` in `search/mod.rs`.
- **New action:** add a variant to `Action`, handle it in `actions::run`, and decide whether it counts as use (`counts_as_use`).
- **New setting:** add a field with a default in `settings.rs`, apply it in `commands::update_settings`, and add a control in `src/settings/Settings.tsx`.
- **New system command:** add a `SystemCommand` variant, an entry in `features/system.rs`, and an arm in each `platform/*.rs`.

Then update `docs/features.md` and run `bun run verify`.

## Style

- Names say what a thing is or does: `refresh::files`, `Store::save_clip`, `latestOnly`. No abbreviations except common ones (`id`, `url`).
- Comments explain why, not what. Every `unsafe` block has a `// SAFETY:` comment.
- User-facing text: short sentences, active voice, no jargon. Errors say what happened and what to do.
- Rust: `cargo fmt`, Clippy with `-D warnings`. TypeScript: Oxfmt and Oxlint through `vp check`; strict mode.

## Commits and pull requests

- Conventional commits: `feat(search): …`, `fix(clipboard): …`, `docs: …`. One logical change per commit.
- The pre-commit hook formats staged files (`vp staged`).
- A PR explains the change, lists the commands you ran with their results, and includes screenshots for UI changes.
