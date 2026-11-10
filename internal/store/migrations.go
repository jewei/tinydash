package store

// migrations upgrade the schema by one version each, byte for byte the
// Rust app's MIGRATIONS, so both apps open the same tinydash.db. Never edit
// a shipped entry; append a new one. TestMigrationsNeverChange pins each.
//
// Each entry runs as one transaction of its statements, split at the
// semicolons outside quotes and comments, so an entry cannot hold a
// trigger body.
var migrations = []string{
	`
    CREATE TABLE usage (
        id TEXT PRIMARY KEY,
        count INTEGER NOT NULL,
        last_used INTEGER NOT NULL
    ) WITHOUT ROWID;
    CREATE TABLE pins (
        id TEXT PRIMARY KEY,
        position INTEGER NOT NULL
    ) WITHOUT ROWID;
    CREATE TABLE clipboard (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL CHECK (kind IN ('text', 'image', 'files')),
        hash INTEGER NOT NULL UNIQUE,
        title TEXT NOT NULL,
        -- Text, or a JSON array of file paths. Empty for images.
        content TEXT NOT NULL,
        image BLOB,
        width INTEGER,
        height INTEGER,
        copied_at INTEGER NOT NULL
    );
    CREATE INDEX clipboard_by_time ON clipboard (copied_at);
    CREATE TABLE library (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL CHECK (kind IN ('snippet', 'quicklink')),
        name TEXT NOT NULL,
        keyword TEXT NOT NULL,
        text TEXT NOT NULL
    );
    CREATE TABLE cache (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) WITHOUT ROWID;
`,
	`
    CREATE TABLE note (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        text TEXT NOT NULL
    );
`,
	`
    CREATE TABLE hidden (
        id TEXT PRIMARY KEY,
        hidden_at INTEGER NOT NULL
    ) WITHOUT ROWID;
`,
	`
    CREATE TABLE aliases (
        id TEXT PRIMARY KEY,
        alias TEXT NOT NULL UNIQUE
    ) WITHOUT ROWID;
`,
	`
    CREATE TABLE calculations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        expression TEXT NOT NULL UNIQUE,
        result TEXT NOT NULL,
        used_at INTEGER NOT NULL
    );
`,
	`
    CREATE TABLE hotkeys (
        id TEXT PRIMARY KEY,
        shortcut TEXT NOT NULL UNIQUE
    ) WITHOUT ROWID;
`,
}
