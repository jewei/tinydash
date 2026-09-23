# Settings, recovery, and updates

Open Settings from Actions, the tray, Command/Ctrl + comma, or `tinydash --settings`. Save changes to apply the form. Discard restores saved values. Closing the window retains the unfinished form. Appearance applies immediately.

Settings covers shortcuts, start at login, appearance, categories, application aliases, custom web searches, clipboard capture, file roots, currency requests, and privacy. See the [settings reference](../settings.md).

The shortcut recorder accepts Shift + Space for the launcher or a visible category. Shift with an ordinary typing key remains invalid. **Reset search and category on open** clears the query and selects the first visible category. Turn it off to retain both the query and category.

Privacy exports a versioned settings file and previews imports before application. Export and clipboard file saving refuse the live settings, database, and recovery paths. Unknown import keys appear in the preview.

The app creates a recovery archive before database migrations. Recovery is manual. Follow [data recovery](../../how-to/recover-data.md).

About has an explicit update check. Mac and Windows release builds need an updater public key and HTTPS feed. The user chooses whether to install an available update. Development builds can report that updates are not configured. Ubuntu uses manual Debian package replacement.

## Verification

Run this browser recipe from the repository root. It retains successful traces in a unique evidence directory:

```sh
bun run verify:browser tests/settings.spec.ts tests/tinycast-features.spec.ts tests/release-config.spec.ts
```

For affected backend behavior:

```sh
bun run test:rust -- settings
bun run test:rust -- db::backup
bun run test:rust -- db::migrations
```

Open Settings through Actions, tray, Ctrl/Command + comma, and `tinydash --settings` when those paths change. Save and reopen; also check Discard and an unsaved close. Export, preview an import, cancel, and confirm saved state is unchanged. An invalid import must preserve saved state.

Use desktop Settings checks on each affected OS for shortcut registration, login behavior, native file dialogs, and persistence. Recovery requires a disposable database and the recovery procedure. Updates require an installed release candidate, test feed, signature checks, and installation evidence. These are explicit gaps in the mocked tests and current native smoke suite.

Record Shift + Space, save it, and reopen Settings. Use it while another app has focus, then test it as a category shortcut. Check that an occupied binding leaves the previous binding usable. Test the selected input method before choosing this shortcut for daily use. Restore the original binding after the check.

Change a setting, save, and reopen. Export settings, preview a changed import, cancel it, then confirm saved state is unchanged. Check the unconfigured updater message in a development build.

Tests: [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tinycast-features.spec.ts](../../../tests/tinycast-features.spec.ts), [tests/release-config.spec.ts](../../../tests/release-config.spec.ts).

Mocked update results do not prove signatures or installation. Recovery and installed updates need the desktop and release procedures.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
