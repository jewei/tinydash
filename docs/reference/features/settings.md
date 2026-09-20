# Settings, recovery, and updates

Open Settings from Actions, the tray, Command/Ctrl + comma, or `tinydash --settings`. Save changes to apply the form. Discard restores saved values. Closing the window retains the unfinished form. Appearance applies immediately.

Settings covers shortcuts, start at login, appearance, categories, application aliases, custom web searches, clipboard capture, file roots, currency requests, and privacy. See the [settings reference](../settings.md).

Privacy exports a versioned settings file and previews imports before application. Export and clipboard file saving refuse the live settings, database, and recovery paths. Unknown import keys appear in the preview.

The app creates a recovery archive before database migrations. Recovery is manual. Follow [data recovery](../../how-to/recover-data.md).

About has an explicit update check. Mac and Windows release builds need an updater public key and HTTPS feed. The user chooses whether to install an available update. Development builds can report that updates are not configured. Ubuntu uses manual Debian package replacement.

## Verification

Change a setting, save, and reopen. Export settings, preview a changed import, cancel it, then confirm saved state is unchanged. Check the unconfigured updater message in a development build.

Tests: [tests/settings.spec.ts](../../../tests/settings.spec.ts), [tests/tinycast-features.spec.ts](../../../tests/tinycast-features.spec.ts), [tests/release-config.spec.ts](../../../tests/release-config.spec.ts).

Mocked update results do not prove signatures or installation. Recovery and installed updates need the desktop and release procedures.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
