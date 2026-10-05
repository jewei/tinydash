# TinyDash — agent guide

TinyDash is a keyboard-first desktop launcher for macOS, Windows, and Linux. The backend is Rust on Tauri 2; the frontend is SolidJS. Read this file before you change anything. Details live in `docs/`.

## Commands

| Task                                    | Command                                                  |
| --------------------------------------- | -------------------------------------------------------- |
| Install dependencies                    | `bun install`                                            |
| Run the desktop app (hot reload)        | `bun run dev` (own identifier and data, never yours)     |
| Format, lint, and type-check everything | `bun run check`                                          |
| Run all tests                           | `bun run test`                                           |
| Fix formatting                          | `bun run fix`                                            |
| **Before every commit or PR**           | `bun run verify`                                         |
| One Rust test                           | `cargo test --manifest-path src-tauri/Cargo.toml <name>` |
| One frontend test file                  | `bunx vp test src/launcher/keymap.test.ts`               |

`bun run verify` runs `check`, the frontend tests, the frontend build, then `scripts/bindings.ts`, which runs the Rust tests and fails if that regenerated anything in `src/generated`. CI runs the frontend checks, rustfmt, and the binding check on Linux, and Clippy and the Rust tests on macOS, Windows, and Linux.

## Where things are

```
src-tauri/src/
  lib.rs               App setup and the list of IPC commands. Start here.
  cli.rs               Command-line arguments.
  commands.rs          Every IPC command. TypeScript wrappers: src/lib/ipc.ts.
  actions.rs           What each Action does to the OS, after checking it.
  preview.rs           Details for the preview pane.
  search/              Query → ranked results: result.rs (SearchResult, Action),
                       id.rs (result IDs), matcher.rs, usage.rs.
  features/            One file per feature (apps, files, clipboard, emoji, ...).
  platform/            macOS, Windows, Linux. Each file implements the contract in platform/mod.rs.
  store.rs             SQLite schema, migrations, and all SQL.
  settings.rs          settings.json model and defaults.
  state.rs             In-memory state; indexes are swapped whole (shared.rs).
  refresh.rs           When indexes and exchange rates rebuild.
  watcher.rs           File system events → "index is dirty".
  monitor.rs           Clipboard capture thread.
  window.rs, tray.rs, shortcut.rs, images.rs, events.rs, system_clipboard.rs, error.rs
src/
  launcher/            Launcher window: state.ts (logic), Launcher.tsx (view), keymap.ts.
  settings/            Settings window.
  ui/                  Components shared by both windows.
  lib/ipc.ts           The only module that calls the backend.
  generated/           TypeScript types written by ts-rs. Never edit by hand.
  test/backend.ts      Fake backend for component tests.
docs/                  architecture.md, features.md, development.md
scripts/               emoji-data.ts (regenerates CLDR data), bindings.ts (IPC type check)
```

Keep private notes, plans, and evidence in `.local/` (ignored by git).

## Rules

1. **Logic lives in Rust.** Search, ranking, validation, and OS work are Rust. The frontend renders results and maps keys to actions; it never decides what an action does.
2. **Features stay out of the app's plumbing.** Files in `features/` and `search/` never call Tauri, SQLite, the clipboard, or windows, and never change anything in the OS. They may read cheap OS state: the home folder path, the clock and time zone, and the random source (for passwords). The two indexers that read the disk or network, `FileIndex::scan` and `currency::fetch`, run only from `refresh.rs`. Glue modules at the top level do everything else.
3. **Results carry their actions.** A `SearchResult` lists `ResultAction`s; the frontend sends the chosen `Action` back. `actions.rs` checks every action again before it runs.
4. **One source of truth for IPC types.** Add `#[derive(TS)] #[ts(export)]` to Rust types that cross IPC. Run `bun scripts/bindings.ts` to regenerate `src/generated` (it also removes bindings of deleted types), then commit the result. Add each new command to `lib.rs`, `commands.rs`, `src/lib/ipc.ts`, and a default reply in `src/test/backend.ts`.
5. **All SQL lives in `store.rs`.** Change the schema by appending to `MIGRATIONS`. Never edit a migration that has shipped.
6. **Platform code lives only in `platform/`.** Add a function to all three OS files and document it in `platform/mod.rs`. Outside `platform/`, `cfg!` may choose only a label or a default value.
7. **Bound what you keep.** Every list, cache, and stored item has a limit (see the constants in each feature).
8. **No new dependency without need.** Prefer the standard library and existing crates. Pin exact versions in `package.json`.
9. **No dead code.** No commented-out code, no TODOs, no unused exports, no speculative abstractions.
10. **Test what you change.** Rust tests sit in a `tests` module in the same file. Frontend tests sit next to the component and use `src/test/backend.ts`.
11. **Never touch the user's real TinyDash data** when you run the app. Use the isolated build in `docs/development.md`.

## Add a feature

- **New search source:** add `features/<name>.rs` with `search`, `browse`, and `get(key)` (see `features/apps.rs`). Add a `Source` variant in `search/id.rs` for its result IDs, add it to `Source::ALL`, and decide `learns_from_use` and `suggestible` (the compiler asks). Add its index to `Snapshot` (`search/mod.rs`) and `State` (`state.rs`), and fill it from `refresh.rs` or `state.rs`; a source that a scan fills must report its first scan in `Freshness::ready` (the compiler asks). Wire it into `search/mod.rs`: `all()`, `search()`, `browse()`, and `resolve()`. If its results have details, load them in `preview::load` and add its kind to `LOADED` in `src/launcher/PreviewPane.tsx`. For a new tab, add a `Category` variant to `Category::ALL` and `Category::name`, an entry in `CATEGORIES` in `src/launcher/state.ts`, and its name in `cli::USAGE`.
- **New instant answer** (computed from the query, like the calculator): add an `answer(query)` function, call it from `answers()` in `search/mod.rs`, and, for a new `ResultKind`, list it in `ANSWERS` in `src/launcher/describe.ts` if its title is the answer.
- **New action:** add a variant to `Action`, handle it in `actions::run`, and decide whether it counts as use (`counts_as_use`).
- **New setting:** add a field with a default in `settings.rs` and clamp or clean it in `Settings::normalized`. If it changes the OS (like the shortcut or login item), apply it in `commands::apply_to_system`, which also rolls it back on failure; otherwise apply it in `commands::update_settings`. Add a control in `src/settings/Settings.tsx` and the field to `testSettings` in `src/test/backend.ts`.
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
