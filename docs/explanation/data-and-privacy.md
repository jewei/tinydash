# Data storage and privacy

TinyDash keeps settings in a JSON file and saved state in a local SQLite database. Clipboard history can contain private text. It is not encrypted. Capture starts only after the user enables it on a fresh installation. Turning capture off keeps saved entries until they are cleared.

Currency refresh downloads a shared rate table. Search text, amounts, clipboard text, and file paths are not part of that request. Web search opens a URL only when the user chooses the action. URL cleaning and the other local tools work offline.

## Usage and ranking

The **All** mode keeps text results together by category, in this order: Apps, Files, System, Clipboard, then Emoji. A prefix of at least two characters from a system command name or alias puts System first. This includes complete names and aliases. For example, `sl`, `sle`, `re`, and `shu` keep their commands visible when app or file matches fill the 30-result limit. Other text queries keep matching apps first, so `sa` puts Safari above the exact emoji shortcode `:sa:`. Valid calculations appear before these groups. Explicit tool commands and the `:` and `=` prefixes keep their own search scope. Category priority applies before the result limit.

Successful app launches, file opens, emoji copies, and accepted system commands update a use count and last-used time. Frequently used and recently used results move higher within their category. This also applies when the search field is empty. Exact and prefix match bonuses still favor close matches within each category. A usage bonus cannot add an item that does not match the query.

The frequency bonus is 25 points per use, up to 500 points. The recency bonus starts at 500 points and decreases with the number of days since the last use. Their combined limit is 1,000 points, compared with 10,000 for an exact match and 2,000 for a prefix match. Results with equal scores in the same category retain their existing order.

TinyDash stores `result_id`, `use_count`, and `last_used_at` in `tinydash.sqlite3`:

| Platform | Default database location                                                                                           |
| -------- | ------------------------------------------------------------------------------------------------------------------- |
| macOS    | `~/Library/Application Support/dev.tinydash.launcher/tinydash.sqlite3`                                              |
| Windows  | `%APPDATA%\dev.tinydash.launcher\tinydash.sqlite3`                                                                  |
| Linux    | `$XDG_DATA_HOME/dev.tinydash.launcher/tinydash.sqlite3`, or `~/.local/share/dev.tinydash.launcher/tinydash.sqlite3` |

Opening a location, failed actions, and calculation copies do not change usage counts. Calculation IDs are temporary. Tool pins store the input needed to run that tool again. Other search queries are not stored. Copied calculation results can enter clipboard history while capture is enabled. App and file usage is tied to the indexed path; moving an item gives it a new ID. File usage IDs contain the full path.

The database loads once on a background worker. Search uses an in-memory copy and performs no database reads while typing. SQLite writes use one connection, a short lock timeout, and explicit transactions for schema migrations and clipboard capture with pruning. There are no database polling timers. SQLite is bundled through [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/), so users do not need a separate SQLite installation.

If the database cannot load or save, TinyDash shows a warning, pauses clipboard capture, and keeps ranking changes in memory until exit. A failed delete or clear reports an error and retains the entries. It leaves an unreadable or newer database intact. Fix the file access problem and restart to restore persistence. To reset all stored data, quit TinyDash, move `tinydash.sqlite3` to a backup location, then reopen the app.

## Recovery

Before a schema migration, TinyDash creates a checked recovery archive. It does not restore it automatically. Follow [data recovery](../how-to/recover-data.md) to inspect and restore an archive.
