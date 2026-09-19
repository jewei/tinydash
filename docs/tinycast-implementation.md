# Concrete recommendation checklist

Scope confirmed on 19 September 2026. Implement the concrete changes in the [product review](tinycast-product-review.md). Keep the deferred tools for later. Existing work on backups, category order, ranking, and the download site was preserved.

| Work                                                          | State                  | Local evidence                                                                                                                                                                                                 |
| ------------------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Select the latest clipboard entry while keeping pins visible  | Implemented            | Fresh empty searches select the latest entry. Typed queries use match order. Refresh preserves the user's selection.                                                                                           |
| Clear unpinned history and retain a separate clear-all action | Implemented            | All and Clipboard pins survive unpinned cleanup and database reopening. Clear-all removes entries and their pins.                                                                                              |
| Edit a copy, combine text entries, and save text as a file    | Implemented            | Exact text and order, separators, size limits, errors, private file writes, and original-entry preservation have tests.                                                                                        |
| App aliases and reversible hidden app results                 | Implemented            | Exact names, Unicode aliases, alias removal, hidden results, refresh, and saved preferences have tests.                                                                                                        |
| Category shortcuts and command-line category access           | Implemented            | Parsing, duplicate bindings, conflicts, rollback, removal, and category events have tests. Each route clears the query.                                                                                        |
| Custom HTTP and HTTPS search templates                        | Implemented            | Preview, URL encoding, origin protection, validation, disabled entries, and stable pinned searches have tests.                                                                                                 |
| Date, time-zone, and explicit currency input cases            | Implemented            | Named dates, source dates, target-zone labels, aliases, invalid dates and zones, daylight-saving cases, and `10 USD CAD` have tests.                                                                           |
| Search within the actions menu                                | Implemented            | Filtering, focus, arrows, Enter, Escape, and input composition have browser checks.                                                                                                                            |
| First-use clipboard choice and start at login                 | Implemented            | Fresh settings disable capture until a choice. Existing choices remain. Login startup uses the background command. OS login checks remain pending.                                                             |
| Settings export and validated import with preview             | Implemented            | Version, types, size, unknown keys, invalid paths, cancel, draft changes, and Save behavior have tests.                                                                                                        |
| Migration backup and recovery route                           | Integrated             | The existing backup implementation passes snapshot, migration-failure, newer-schema, archive, and permissions checks. Settings can show the archive and manual recovery steps.                                 |
| Signed-update command and release preparation                 | Prepared               | Manual check and install use Tauri signature verification. Configuration guards and release scripts have local checks. Signed-package verification still needs publisher credentials and target desktops.      |
| Discovery, keyboard, focus, and large-data checks             | Expanded               | Mac discovery fixtures cover nested apps, PWA metadata, duplicate bundle IDs, and Unicode names. Tests cover 500 clipboard entries, 16 KiB text, and bounded app results. A 50,000-file benchmark is recorded. |
| Signed packages and physical desktop checks                   | Pending external setup | Signing credentials, a test update feed, Windows, Ubuntu X11 and Wayland, and clean installed-app sessions are required.                                                                                       |

The [search and control guide](search-controls.md) explains the new controls. [Data recovery](data-recovery.md) explains how to preserve current files and use a compatible app version. [Release setup](release-setup.md) lists the required signing configuration. [Desktop checks](desktop-checks.md) includes the new manual cases.

Local verification on arm64 macOS, 19 September 2026:

- Rust tests: 151 passed, with three host or performance checks ignored by default.
- Browser tests: 95 passed. The 11 feature tests also passed after the final narrow-layout change.
- Rust Clippy passed for all local targets with warnings treated as errors. Rust formatting and the frontend type check passed.
- The frontend production build passed. Screenshots were inspected for the first-use panel, Actions menu, and clipboard dialogs.
- The macOS arm64 release executable built with `bun run tauri build --no-bundle`. Command-line help and invalid-category handling passed. This did not create a signed installer.
- The existing release benchmark ran separately. See [performance checks](performance.md) for its results and measurement limits.
- Two additional Mac checks passed against 126 installed applications. They checked discovery, search, and native icons without opening those applications.
- Release fixtures passed for artifact collection, checksums, staging, and updater manifest generation. Checks rejected changed bytes, wrong architectures, wrong versions, and modified source trees. Unsigned test packages retain their standalone archives.

These are checks of the shared working tree, not a published release. Browser mocks cannot verify native dialogs, OS shortcut registration, focus return, login startup, publisher trust, or updater installation. The Windows and Linux native smoke script now checks first-use capture, newest-entry selection, and both clear actions. It was not executed on this Mac. Use the [release verification record](release-verification.md) before release promotion.

AI, notes, calendar, extensions, window management, global key remapping, clipboard images, sync, OCR, custom shell commands, folder previews, and locale number parsing remain deferred. Snippets remain a later validation option. No new provider or automatic paste behavior was added for those ideas.

## Review corrections, 19 September 2026

The two independent reviews found five defects. The corrections:

- Supply the updater public key to both signed release configurations. Reject an empty key before the build.
- Restore each saved shortcut after recording. Keep working shortcuts when another shortcut conflicts.
- Receive settings changes from the launcher. Keep local edits and update untouched fields, including app aliases and hidden states.
- Select the newest clipboard entry after delayed capture, until the user selects an entry.
- Reject custom search keywords that belong to date and URL commands.

The shared working tree passed 152 Rust tests, with three optional tests skipped, and 104 browser tests. The new checks cover the release configuration, shortcut recovery, settings changes between windows, and delayed clipboard capture. Type checking, Rust formatting, and diff checks passed. Browser checks use mock IPC. Signed installers and physical shortcut registration still need platform checks.

The isolated commit copy passed 148 Rust tests, with three optional tests skipped, and 104 browser tests. It also passed Clippy with warnings treated as errors, the frontend production build, formatting, and shell syntax checks. The four additional Rust tests in the shared working tree belong to the separate ranking change.

For coordination, this commit includes the shared database backup dependency. Site, ranking, category-order, and later release-task edits remain in the working tree for their owners. The later release configuration retains `plugins.updater.pubkey`; keep that field when changing the Windows distribution route. Release publication and signing verification remain separate work.
