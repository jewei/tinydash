# Data recovery

TinyDash keeps a recovery archive named `recovery.tar` in its application data directory. TinyDash creates the archive before a database schema migration. There is no manual backup or restore command. The archive contains:

- `tinydash.sqlite3`: a consistent SQLite snapshot.
- `settings.json`: the settings file, when it exists.
- `manifest.json`: archive format, schema, and creation details.

The archive is private to the current user. TinyDash replaces it only after the new archive passes its checks. A failed backup keeps the previous archive.

Recovery is manual. TinyDash does not restore files automatically. Changes made after the backup are not in the archive. Use an app version that supports the archive's `sourceSchema` value in `manifest.json`. The `createdByVersion` value identifies the app that made the archive; it does not prove that an older app can read it.

1. Quit TinyDash.
2. Copy the current `tinydash.sqlite3`, `settings.json`, and `recovery.tar` files to a separate safe folder. Also copy any `tinydash.sqlite3-wal`, `tinydash.sqlite3-shm`, or `tinydash.sqlite3-journal` files. Keep these copies together.
3. Copy `recovery.tar` to a temporary folder. List its contents before extraction:

   ```sh
   tar -tf recovery.tar
   ```

4. Extract the archive to a new temporary folder. Confirm that it contains only the expected files listed above. Read `manifest.json` and check app compatibility before you continue.
5. Move the current database and its `-wal`, `-shm`, and `-journal` companion files out of TinyDash's application data directory. Copy `tinydash.sqlite3` from the temporary folder into that directory. Do not leave companion files from a different database beside the restored file.
6. If the archive contains `settings.json`, copy it to TinyDash's application configuration directory. Otherwise, keep the current settings.
7. Start TinyDash and check the search history, pins, settings, and file search roots.

Keep the copies from step 2 until recovery is complete. A later migration can replace the live recovery archive. If the result is wrong, quit TinyDash. Move the restored database and any new companion files aside, then restore the files you kept in step 2.

The usual settings and data locations are:

| Platform | Settings                                                                                                   | Database and recovery archive                                                                                                          |
| -------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/settings.json`                                        | `~/Library/Application Support/dev.tinydash.launcher/tinydash.sqlite3` and `recovery.tar`                                              |
| Windows  | `%APPDATA%\dev.tinydash.launcher\settings.json`                                                            | `%APPDATA%\dev.tinydash.launcher\tinydash.sqlite3` and `recovery.tar`                                                                  |
| Linux    | `$XDG_CONFIG_HOME/dev.tinydash.launcher/settings.json`, or `~/.config/dev.tinydash.launcher/settings.json` | `$XDG_DATA_HOME/dev.tinydash.launcher/tinydash.sqlite3`, or `~/.local/share/dev.tinydash.launcher/tinydash.sqlite3` and `recovery.tar` |

The real paths shown in TinyDash Settings take precedence when environment variables or packaging change these defaults.
