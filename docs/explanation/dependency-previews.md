# Dependency previews

This branch tests published development releases as of 29 September 2026. It uses the highest available preview version for each direct dependency. Packages without a newer preview use the latest stable release. Lockfiles select compatible indirect dependencies. Bun and Rust remain at the supported toolchain versions in the [build guide](../how-to/build.md).

## Version selection

| Component                        | Version                    |
| -------------------------------- | -------------------------- |
| Tauri Rust crate and Wry runtime | 3.0.0-alpha.3              |
| Tauri build crate                | 3.0.0-alpha.2              |
| Tauri CLI                        | 3.0.0-alpha.3              |
| Tauri JavaScript API             | 3.0.0-alpha.2              |
| Tauri plugins                    | 3.0.0-alpha.1              |
| Solid and its web runtime        | 2.0.0-rc.11                |
| Solid Vite plugin                | 3.0.0-next.46              |
| TypeScript                       | 7.1.0-dev.20260928.1       |
| Playwright                       | 1.64.0-alpha-1790635538000 |
| Prettier                         | 4.0.0-alpha.13             |
| Vite                             | 8.3.1                      |
| notify                           | 9.0.0-rc.5                 |
| libc                             | 1.0.0-alpha.4              |

`@tauri-apps/api@3.0.0-alpha.3` is not published. The API and CLI have separate release numbers. Use the published API `alpha.2` with the CLI and Rust crate `alpha.3`. See the [npm API metadata](https://registry.npmjs.org/@tauri-apps/api) and [Tauri release](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.3).

GTK stays at 0.18.2. The Wry runtime requires that version of the GTK bindings. GTK 0.19 uses the same native library link and cannot be installed beside it in this dependency graph. Other indirect dependencies can also remain below their latest release when their parent package restricts the version.

## Required code changes

Tauri now receives an explicit Wry runtime. Enable `macos-private-api` on both Tauri and that runtime. The Tauri build check reads the direct feature list. Native webview access uses the Wry extension trait. Global shortcut state uses Tauri's default `DynRuntime`. See the [Tauri migration notes](https://github.com/tauri-apps/tauri/releases/tag/tauri-v3.0.0-alpha.0).

Solid now uses `@solidjs/web` for rendering and JSX types, and `@solidjs/vite-plugin` for compilation. Effects separate tracked inputs from side effects and return their cleanup function. Component setup uses `onSettled`. Store updates use draft callbacks; `reconcile` changes the draft in place, and `snapshot` creates plain values.

Solid batches state changes until the next microtask. Search commits pending state before reading a new category or visibility value. This keeps clipboard previews hidden until results arrive. Focus returns after a dialog is removed. ARIA and data attributes use explicit strings where CSS and accessibility checks require `true` or `false`. See the [Solid migration guide](https://github.com/solidjs/solid/blob/ee49b3e/documentation/solid-2.0/MIGRATION.md).

The notify 9 watcher uses `watch` and `unwatch` in place of the removed `paths_mut` API. Failed registrations still report an error and retain successful watches.

## Temporary package repairs

These repairs are part of this branch and apply from a fresh checkout. Remove each repair when its replacement release passes the same checks.

- [Solid type patch](../../patches/solid-js@2.0.0-rc.11.patch): restores internal declarations that the published entry point exports but the package omitted. The declarations follow the tagged source. Bun applies the patch during installation. Runtime code and strict application type checks are unchanged.
- [Dialog plugin source](../../src-tauri/vendor/tauri-plugin-dialog/src/desktop.rs): adds `use tauri::Manager;` for Tauri's moved `run_on_main_thread` method.
- [Updater plugin source](../../src-tauri/vendor/tauri-plugin-updater/src/updater.rs): adds the same import.

The Rust repairs use Cargo path overrides. Both directories contain the published 3.0.0-alpha.1 crate source and licenses, with only the import added. Their standalone lockfiles are omitted because the application lockfile controls the dependency graph. Third-party formatting is preserved.

| Published crate                    | Archive SHA-256                                                    |
| ---------------------------------- | ------------------------------------------------------------------ |
| tauri-plugin-dialog 3.0.0-alpha.1  | `c83b450aab99762914964c402b0bcb20529c910568ca920f9fd3924221bd6271` |
| tauri-plugin-updater 3.0.0-alpha.1 | `0ba0b95e3e2c17d5d92fe4bb88104f8b00edd489027925b03c9ebb3b42377ff8` |

## Checks before adoption

Run a frozen dependency install, `bun run verify:full`, and an [identified desktop build](../how-to/verify.md#drive-the-real-desktop-app). Browser tests use mocked IPC. They cover the Solid migration, including result reuse, keyboard control, confirmation, settings, previews, and focus restoration.

A successful build does not prove desktop behavior. Test shortcuts, focus return, native glass, dialogs, updates, and file watching on the required operating systems before adoption. Keep this branch experimental until those checks pass. Store run results and platform limits in the private acceptance record required by the [verification procedure](../how-to/verify.md).
