# Settings, recovery, and updates

Open Settings from Actions, the tray, Command/Ctrl + comma, or `tinydash --settings`. Save changes to apply the form. Discard restores saved values. Closing the window retains the unfinished form. Appearance applies immediately.

On macOS, **Shortcut > Show menu bar icon** is off by default, including existing settings without an explicit choice. Save to show or hide it without restarting; Discard restores the saved choice. The choice is included in settings exports. TinyDash never shows a Dock icon. Windows and Linux retain their tray icon.

The section list and settings content scroll separately, so the save controls remain visible in short windows. At narrow widths, Search settings shows matching section buttons. Clear the search to return to the section selector. Searching and switching sections retain the draft.

Settings covers shortcuts, start at login, appearance, categories, application aliases, custom web searches, emoji skin tone and language keywords, clipboard capture, file roots, currency requests, and privacy. **Search settings** filters sections by labels and related terms; Enter opens the first match without discarding drafts. Search also configures item aliases, global shortcuts, and separate Hide/Disable controls for apps, built-in commands, and library entries. Hidden items retain shortcuts; disabled items reject execution. Shortcut conflicts are validated in Rust before saving. Clipboard adds age retention, app exclusions, and independent image/file opt-ins; File search adds hidden-file inclusion and bounded ignore patterns. See the [settings reference](../settings.md).

Appearance has five themes: Light, Dark, Sage, Rose, and Ink. Compact is a separate layout switch. Both choices apply immediately and are included in settings exports. On macOS, Follow macOS Liquid Glass also saves immediately and is included in exports. Turn it off for a solid theme background. Import previews keep them unsaved until Save changes. The macOS launcher uses system-controlled Liquid Glass where supported. See [appearance](appearance.md) for the OS limits and desktop checks.

The shortcut recorder accepts Shift + Space for the launcher or a visible category. Shift with an ordinary typing key remains invalid. **Reset search and category on open** clears the query and selects the first visible category. Turn it off to retain both the query and category.

Privacy exports a versioned settings file and previews imports before application. Export and clipboard file saving refuse the live settings, database, and recovery paths. Unknown import keys appear in the preview.

The app creates a recovery archive before database migrations. Recovery is manual. Follow [data recovery](../../how-to/recover-data.md).

Settings-only IPC permissions restrict settings replacement, import/export, shortcut recording, recovery access, and updates to this window. Main retains its narrow app-preference and clipboard-consent actions; both windows can clear history and synchronize appearance. See the [IPC permission boundary](../ipc-permissions.md).

About has an explicit update check. Mac and Windows release builds need an updater public key and HTTPS feed. The user chooses whether to install an available update. Development builds can report that updates are not configured. Ubuntu uses manual Debian package replacement.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/settings.spec.ts tests/tinycast-features.spec.ts tests/release-config.spec.ts tests/roadmap.spec.ts
bun run verify:browser tests/settings-draft.spec.ts tests/native-subscriptions.spec.ts
```

Draft merging preserves local app and item aliases while accepting remote changes to unchanged fields. A remote removal restores default visibility, disabled state, and shortcuts without discarding edited aliases. Shortcut warnings clear when the affected bindings change; unrelated preference changes keep those warnings.

Browser layout checks also cover the visible save controls at 720 × 550 and matching section navigation with a retained draft at 360 × 600.

Standalone draft tests check local edits against incoming saved values, nested application preferences, remote deletion of default preferences while aliases are dirty, local deletions, category normalization, and incomplete folder input. Browser tests additionally exercise clean and dirty forms receiving changes from another window. A saved-settings event must not discard unrelated local edits.

For affected backend behavior:

```sh
bun run test:rust -- acl_tests
bun run test:rust -- settings
bun run test:rust -- db::backup
bun run test:rust -- db::migrations
```

Open Settings through Actions, tray, Ctrl/Command + comma, and `tinydash --settings` when those paths change. Save and reopen; also check Discard and an unsaved close. Export, preview an import, cancel, and confirm saved state is unchanged. An invalid import must preserve saved state.

Check settings search with “retention”, “patterns”, and “hotkey”. Save aliases and a unique item shortcut; verify hidden versus disabled behavior and conflict rollback. Use a disruptive system-command shortcut and cancel its confirmation without executing it.

Use desktop Settings checks on each affected OS for shortcut registration, login behavior, native file dialogs, and persistence. Recovery requires a disposable database and the recovery procedure. Updates require an installed release candidate, test feed, signature checks, and installation evidence. These are explicit gaps in the mocked tests and current native smoke suite.

Record Shift + Space, save it, and reopen Settings. Use it while another app has focus, then test it as a category shortcut. Check that an occupied binding leaves the previous binding usable. Test the selected input method before choosing this shortcut for daily use. Restore the original binding after the check.

On macOS, start with a fresh profile and an older settings file without `showMenuBarIcon`. Confirm no Dock or menu bar icon appears. Enable **Show menu bar icon**, save, and exercise its launcher, Settings, refresh, and quit actions. Restart and confirm the choice persists. Disable it, save, and confirm the icon disappears while the shortcut and second launch still work. Keep the Dock hidden with the launcher and Settings open. Browser tests prove only the form and mocked persistence; these OS effects require a controlled desktop session.

Change a setting, save, and reopen. Export settings, preview a changed import, cancel it, then confirm saved state is unchanged. Check the unconfigured updater message in a development build.

Tests: [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tinycast-features.spec.ts](../../../tests/tinycast-features.spec.ts), [tests/release-config.spec.ts](../../../tests/release-config.spec.ts).

Mocked update results do not prove signatures or installation. Recovery and installed updates need the desktop and release procedures.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
