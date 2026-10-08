//! The SQLite database: the only module that knows the schema or runs SQL.
//! Usage, pins, clipboard history, the snippet library, the scratch note,
//! and cached exchange rates and weather live in one file in the app's local data folder (`lib.rs`: on
//! Windows `%LOCALAPPDATA%`, which roaming profiles do not copy).

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    error::Result,
    features::{
        clipboard::{ClipKind, Content, Entry, MAX_IMAGES},
        currency::Rates,
        library::{LibraryItem, LibraryKind},
        weather::Weather,
    },
    search::{
        id::Source,
        usage::{MAX_USAGE, Pins, Usage, Use},
    },
};

pub const FILE_NAME: &str = "tinydash.db";

/// Each entry upgrades the schema by one version. Never edit a shipped entry;
/// append a new one.
const MIGRATIONS: &[&str] = &[
    r"
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
",
    r"
    CREATE TABLE note (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        text TEXT NOT NULL
    );
",
    r"
    CREATE TABLE hidden (
        id TEXT PRIMARY KEY,
        hidden_at INTEGER NOT NULL
    ) WITHOUT ROWID;
",
];

pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(FILE_NAME);
        let store = Self::new(Connection::open(&path)?)?;
        // Clipboard history can hold private text.
        if let Err(error) = crate::platform::restrict_to_owner(&path) {
            tracing::warn!(%error, "Could not restrict database permissions");
        }
        Ok(store)
    }

    /// A private database for tests, or when the real one cannot open.
    pub fn in_memory() -> Self {
        Self::new(Connection::open_in_memory().expect("SQLite can open a memory database"))
            .expect("the schema applies to an empty database")
    }

    fn new(mut connection: Connection) -> Result<Self> {
        // Deleted clipboard rows are overwritten on disk, not just unlinked.
        connection.execute_batch("PRAGMA secure_delete = ON;")?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn with<T>(&self, work: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut connection = self.connection.lock().unwrap_or_else(|e| e.into_inner());
        Ok(work(&mut connection)?)
    }

    pub fn usage(&self) -> Result<Usage> {
        self.with(|db| {
            let mut statement = db.prepare("SELECT id, count, last_used FROM usage")?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    Use {
                        count: row.get(1)?,
                        last_used: row.get(2)?,
                    },
                ))
            })?;
            Ok(Usage::new(rows.collect::<rusqlite::Result<Vec<_>>>()?))
        })
    }

    /// Save one result's usage, keeping only the most recently used rows.
    /// Two uses at once may save in either order, so a row never goes back.
    pub fn record_use(&self, id: &str, used: Use) -> Result<()> {
        self.with(|db| {
            db.execute(
                "INSERT INTO usage (id, count, last_used) VALUES (?1, ?2, ?3)
                 ON CONFLICT (id) DO UPDATE
                 SET count = max(count, ?2), last_used = max(last_used, ?3)",
                params![id, used.count, used.last_used],
            )?;
            db.execute(
                "DELETE FROM usage WHERE id NOT IN
                 (SELECT id FROM usage ORDER BY last_used DESC, id LIMIT ?1)",
                [MAX_USAGE as i64],
            )
            .map(drop)
        })
    }

    pub fn pins(&self) -> Result<Pins> {
        self.with(|db| {
            let mut statement = db.prepare("SELECT id FROM pins ORDER BY position")?;
            let ids = statement.query_map([], |row| row.get(0))?;
            Ok(Pins::new(ids.collect::<rusqlite::Result<_>>()?))
        })
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<()> {
        self.with(|db| {
            if pinned {
                db.execute(
                    "INSERT OR IGNORE INTO pins (id, position)
                     VALUES (?1, (SELECT COALESCE(MAX(position), 0) + 1 FROM pins))",
                    [id],
                )
            } else {
                db.execute("DELETE FROM pins WHERE id = ?1", [id])
            }
            .map(drop)
        })
    }

    /// Saved entries, newest first, without image bytes.
    pub fn clipboard_history(&self) -> Result<Vec<Entry>> {
        self.with(|db| {
            let mut statement = db.prepare(
                "SELECT id, kind, title, content, copied_at FROM clipboard ORDER BY copied_at DESC, id DESC",
            )?;
            let rows = statement.query_map([], |row| {
                let kind = kind_from_sql(&row.get::<_, String>(1)?)?;
                let content: String = row.get(3)?;
                // File lists are stored as JSON; search their plain paths.
                let searchable = match kind {
                    ClipKind::Files => serde_json::from_str::<Vec<String>>(&content)
                        .map(|paths| paths.join("\n"))
                        .unwrap_or(content),
                    _ => content,
                };
                Ok(Entry::new(row.get(0)?, kind, row.get(2)?, &searchable, row.get(4)?))
            })?;
            rows.collect()
        })
    }

    /// Save a capture. A repeat moves the existing entry to the top. Then the
    /// oldest unpinned entries beyond `limit` (and beyond the image cap) go.
    pub fn save_clip(&self, content: &Content, now: i64, limit: u32) -> Result<()> {
        let (text, image, width, height) = match content {
            Content::Text(text) => (text.clone(), None, None, None),
            Content::Image { png, width, height } => (
                String::new(),
                Some(png.as_slice()),
                Some(*width),
                Some(*height),
            ),
            Content::Files(paths) => (serde_json::to_string(paths)?, None, None, None),
        };
        self.with(|db| {
            let transaction = db.transaction()?;
            transaction.execute(
                "INSERT INTO clipboard (kind, hash, title, content, image, width, height, copied_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (hash) DO UPDATE SET copied_at = ?8",
                params![
                    kind_to_sql(content.kind()),
                    content.hash(),
                    content.title(),
                    text,
                    image,
                    width,
                    height,
                    now
                ],
            )?;
            trim(&transaction, limit)?;
            transaction.commit()
        })
    }

    /// Delete the oldest unpinned entries beyond `limit`, for example after
    /// the user lowered it.
    pub fn trim_clipboard(&self, limit: u32) -> Result<()> {
        self.with(|db| trim(db, limit))
    }

    /// The full saved content of one entry.
    pub fn clip(&self, id: i64) -> Result<Option<Content>> {
        let row = self.with(|db| {
            db.query_row(
                "SELECT kind, content, image, width, height FROM clipboard WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        kind_from_sql(&row.get::<_, String>(0)?)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<Vec<u8>>>(2)?,
                        row.get::<_, Option<u32>>(3)?,
                        row.get::<_, Option<u32>>(4)?,
                    ))
                },
            )
            .optional()
        })?;
        Ok(match row {
            None => None,
            Some((ClipKind::Text, text, ..)) => Some(Content::Text(text)),
            Some((ClipKind::Files, json, ..)) => {
                Some(Content::Files(serde_json::from_str::<Vec<PathBuf>>(&json)?))
            }
            Some((ClipKind::Image, _, png, width, height)) => Some(Content::Image {
                png: png.unwrap_or_default(),
                width: width.unwrap_or_default(),
                height: height.unwrap_or_default(),
            }),
        })
    }

    pub fn delete_clip(&self, id: i64) -> Result<()> {
        self.with(|db| {
            let transaction = db.transaction()?;
            transaction.execute("DELETE FROM clipboard WHERE id = ?1", [id])?;
            transaction.execute("DELETE FROM pins WHERE id = ?1", [Source::Clip.id(id)])?;
            transaction.commit()
        })
    }

    /// Remove every entry except pinned ones.
    pub fn clear_clipboard(&self) -> Result<()> {
        self.with(|db| {
            db.execute(
                "DELETE FROM clipboard WHERE 'clip:' || id NOT IN (SELECT id FROM pins)",
                [],
            )
            .map(drop)
        })
    }

    pub fn library(&self) -> Result<Vec<LibraryItem>> {
        self.with(|db| {
            let mut statement = db.prepare("SELECT id, kind, name, keyword, text FROM library")?;
            let rows = statement.query_map([], |row| {
                Ok(LibraryItem {
                    id: Some(row.get(0)?),
                    kind: match row.get::<_, String>(1)?.as_str() {
                        "snippet" => LibraryKind::Snippet,
                        "quicklink" => LibraryKind::Quicklink,
                        other => return Err(invalid_kind(1, other)),
                    },
                    name: row.get(2)?,
                    keyword: row.get(3)?,
                    text: row.get(4)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Insert or update an item, returning it with its ID. Updating an item
    /// that was deleted meanwhile fails instead of saving nothing.
    pub fn save_library_item(&self, item: &LibraryItem) -> Result<LibraryItem> {
        let kind = match item.kind {
            LibraryKind::Snippet => "snippet",
            LibraryKind::Quicklink => "quicklink",
        };
        let (id, changed) = self.with(|db| match item.id {
            Some(id) => db
                .execute(
                    "UPDATE library SET kind = ?2, name = ?3, keyword = ?4, text = ?5 WHERE id = ?1",
                    params![id, kind, item.name, item.keyword, item.text],
                )
                .map(|changed| (id, changed)),
            None => db
                .execute(
                    "INSERT INTO library (kind, name, keyword, text) VALUES (?1, ?2, ?3, ?4)",
                    params![kind, item.name, item.keyword, item.text],
                )
                .map(|changed| (db.last_insert_rowid(), changed)),
        })?;
        if changed == 0 {
            return Err(crate::error::Error::msg(
                "This item was deleted, so it was not saved. Save again to add it as a new item.",
            ));
        }
        Ok(LibraryItem {
            id: Some(id),
            ..item.clone()
        })
    }

    pub fn delete_library_item(&self, id: i64) -> Result<()> {
        self.with(|db| {
            let transaction = db.transaction()?;
            transaction.execute("DELETE FROM library WHERE id = ?1", [id])?;
            transaction.execute(
                "DELETE FROM pins WHERE id IN (?1, ?2)",
                [Source::Snippet.id(id), Source::Link.id(id)],
            )?;
            transaction.execute(
                "DELETE FROM usage WHERE id IN (?1, ?2)",
                [Source::Snippet.id(id), Source::Link.id(id)],
            )?;
            transaction.commit()
        })
    }

    /// Result IDs the user hid, oldest first.
    pub fn hidden(&self) -> Result<Vec<String>> {
        self.with(|db| {
            let mut statement = db.prepare("SELECT id FROM hidden ORDER BY hidden_at, id")?;
            let rows = statement.query_map([], |row| row.get(0))?;
            rows.collect()
        })
    }

    pub fn set_hidden(&self, id: &str, hidden: bool, now: i64) -> Result<()> {
        self.with(|db| {
            if hidden {
                db.execute(
                    "INSERT INTO hidden (id, hidden_at) VALUES (?1, ?2)
                     ON CONFLICT (id) DO NOTHING",
                    params![id, now],
                )
            } else {
                db.execute("DELETE FROM hidden WHERE id = ?1", [id])
            }
            .map(drop)
        })
    }

    /// The scratch note of the widget pane; empty until it is first saved.
    pub fn note(&self) -> Result<String> {
        let text = self.with(|db| {
            db.query_row("SELECT text FROM note WHERE id = 1", [], |row| row.get(0))
                .optional()
        })?;
        Ok(text.unwrap_or_default())
    }

    pub fn save_note(&self, text: &str) -> Result<()> {
        self.with(|db| {
            db.execute(
                "INSERT INTO note (id, text) VALUES (1, ?1)
                 ON CONFLICT (id) DO UPDATE SET text = ?1",
                [text],
            )
            .map(drop)
        })
    }

    /// The latest weather download, for the widget while offline.
    pub fn weather(&self) -> Result<Option<Weather>> {
        self.cached("weather")
    }

    pub fn save_weather(&self, weather: &Weather) -> Result<()> {
        self.cache("weather", weather)
    }

    fn cached<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let json: Option<String> = self.with(|db| {
            db.query_row("SELECT value FROM cache WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
        })?;
        Ok(json.map(|json| serde_json::from_str(&json)).transpose()?)
    }

    fn cache(&self, key: &str, value: &impl serde::Serialize) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.with(|db| {
            db.execute(
                "INSERT INTO cache (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = ?2",
                [key, json.as_str()],
            )
            .map(drop)
        })
    }

    pub fn rates(&self) -> Result<Option<Rates>> {
        self.cached("currency_rates")
    }

    pub fn save_rates(&self, rates: &Rates) -> Result<()> {
        self.cache("currency_rates", rates)
    }

    pub fn delete_rates(&self) -> Result<()> {
        self.with(|db| {
            db.execute("DELETE FROM cache WHERE key = 'currency_rates'", [])
                .map(drop)
        })
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let version = usize::try_from(version).unwrap_or(usize::MAX);
    if version > MIGRATIONS.len() {
        return Err(crate::error::Error::msg(
            "The database was created by a newer TinyDash. Update TinyDash to use it.",
        ));
    }
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", index as i64 + 1)?;
        transaction.commit()?;
    }
    Ok(())
}

fn kind_to_sql(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Text => "text",
        ClipKind::Image => "image",
        ClipKind::Files => "files",
    }
}

fn kind_from_sql(kind: &str) -> rusqlite::Result<ClipKind> {
    match kind {
        "text" => Ok(ClipKind::Text),
        "image" => Ok(ClipKind::Image),
        "files" => Ok(ClipKind::Files),
        other => Err(invalid_kind(1, other)),
    }
}

/// Delete the oldest unpinned entries beyond `limit`, and unpinned images
/// beyond `MAX_IMAGES`.
fn trim(db: &Connection, limit: u32) -> rusqlite::Result<()> {
    let unpinned = "'clip:' || id NOT IN (SELECT id FROM pins)";
    db.execute(
        &format!(
            "DELETE FROM clipboard WHERE id IN (SELECT id FROM clipboard WHERE {unpinned}
             ORDER BY copied_at DESC, id DESC LIMIT -1 OFFSET ?1)"
        ),
        [limit],
    )?;
    db.execute(
        &format!(
            "DELETE FROM clipboard WHERE id IN (SELECT id FROM clipboard WHERE kind = 'image'
             AND {unpinned} ORDER BY copied_at DESC, id DESC LIMIT -1 OFFSET ?1)"
        ),
        [MAX_IMAGES as i64],
    )?;
    Ok(())
}

/// A kind column held a value this version does not know.
fn invalid_kind(column: usize, kind: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnType(column, format!("kind {kind}"), rusqlite::types::Type::Text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Content {
        Content::Text(value.into())
    }

    fn titles(store: &Store) -> Vec<String> {
        store
            .clipboard_history()
            .unwrap()
            .into_iter()
            .map(|entry| entry.title)
            .collect()
    }

    #[test]
    fn usage_and_pins_round_trip() {
        let store = Store::in_memory();
        store
            .record_use(
                "app:/a",
                Use {
                    count: 3,
                    last_used: 9,
                },
            )
            .unwrap();
        let older = Use {
            count: 2,
            last_used: 8,
        };
        store.record_use("app:/a", older).unwrap();
        assert_eq!(store.usage().unwrap().bonus("app:/a", 9), 575);
        store.set_pinned("b", true).unwrap();
        store.set_pinned("a", true).unwrap();
        store.set_pinned("b", true).unwrap();
        assert_eq!(store.pins().unwrap().ids(), ["b", "a"]);
        store.set_pinned("b", false).unwrap();
        assert_eq!(store.pins().unwrap().ids(), ["a"]);
    }

    #[test]
    fn usage_drops_the_same_entry_in_memory_and_on_disk() {
        let store = Store::in_memory();
        let mut usage = Usage::default();
        for n in 0..=MAX_USAGE {
            let id = format!("app:/{n:04}");
            let used = usage.record(&id, 7);
            store.record_use(&id, used).unwrap();
        }
        let saved = store.usage().unwrap();
        let (mut on_disk, mut in_memory) = (saved.ranked(7), usage.ranked(7));
        on_disk.sort();
        in_memory.sort();
        assert_eq!(on_disk, in_memory);
    }

    #[test]
    fn repeats_move_to_the_top_and_pins_survive_pruning() {
        let store = Store::in_memory();
        for (now, value) in [(1, "one"), (2, "two"), (3, "three")] {
            store.save_clip(&text(value), now, 10).unwrap();
        }
        store.save_clip(&text("one"), 4, 10).unwrap();
        assert_eq!(titles(&store), ["one", "three", "two"]);

        let two = store.clipboard_history().unwrap()[2].id;
        store.set_pinned(&format!("clip:{two}"), true).unwrap();
        store.save_clip(&text("four"), 5, 2).unwrap();
        assert_eq!(titles(&store), ["four", "one", "two"]);

        store.save_clip(&text("five"), 6, 10).unwrap();
        store.trim_clipboard(1).unwrap();
        assert_eq!(titles(&store), ["five", "two"]);

        store.clear_clipboard().unwrap();
        assert_eq!(titles(&store), ["two"]);
        store.delete_clip(two).unwrap();
        assert!(titles(&store).is_empty());
        assert!(store.pins().unwrap().ids().is_empty());
    }

    #[test]
    fn stores_images_and_files_in_their_original_form() {
        let store = Store::in_memory();
        let image = Content::Image {
            png: vec![1, 2, 3],
            width: 2,
            height: 1,
        };
        let files = Content::Files(vec!["/tmp/a b.txt".into()]);
        store.save_clip(&image, 1, 10).unwrap();
        store.save_clip(&files, 2, 10).unwrap();
        let history = store.clipboard_history().unwrap();
        assert_eq!(store.clip(history[0].id).unwrap(), Some(files));
        assert_eq!(store.clip(history[1].id).unwrap(), Some(image));
        assert_eq!(store.clip(999).unwrap(), None);
    }

    #[test]
    fn images_beyond_the_cap_are_pruned() {
        let store = Store::in_memory();
        for n in 0..(MAX_IMAGES as i64 + 2) {
            let image = Content::Image {
                png: n.to_le_bytes().to_vec(),
                width: 1,
                height: 1,
            };
            store.save_clip(&image, n, 1000).unwrap();
        }
        assert_eq!(store.clipboard_history().unwrap().len(), MAX_IMAGES);
    }

    #[test]
    fn library_items_save_update_and_delete() {
        let store = Store::in_memory();
        let item = LibraryItem {
            id: None,
            kind: LibraryKind::Snippet,
            name: "Sig".into(),
            keyword: "sig".into(),
            text: "Regards".into(),
        };
        let saved = store.save_library_item(&item).unwrap();
        let id = saved.id.unwrap();
        store.set_pinned(&format!("snippet:{id}"), true).unwrap();
        store
            .save_library_item(&LibraryItem {
                text: "Cheers".into(),
                ..saved.clone()
            })
            .unwrap();
        assert_eq!(store.library().unwrap()[0].text, "Cheers");
        store.delete_library_item(id).unwrap();
        assert!(store.library().unwrap().is_empty());
        assert!(store.pins().unwrap().ids().is_empty());
        assert!(store.save_library_item(&saved).is_err());
        assert!(store.library().unwrap().is_empty());
    }

    #[test]
    fn hides_and_shows_results_again() {
        let store = Store::in_memory();
        store.set_hidden("app:/b", true, 2).unwrap();
        store.set_hidden("app:/a", true, 1).unwrap();
        store.set_hidden("app:/a", true, 3).unwrap();
        assert_eq!(store.hidden().unwrap(), ["app:/a", "app:/b"]);
        store.set_hidden("app:/a", false, 4).unwrap();
        assert_eq!(store.hidden().unwrap(), ["app:/b"]);
    }

    #[test]
    fn the_note_saves_over_itself() {
        let store = Store::in_memory();
        assert_eq!(store.note().unwrap(), "");
        store.save_note("call the bank").unwrap();
        store.save_note("call the bank\nbuy milk").unwrap();
        assert_eq!(store.note().unwrap(), "call the bank\nbuy milk");
    }

    #[test]
    fn rates_round_trip_and_newer_schemas_are_refused() {
        let store = Store::in_memory();
        assert_eq!(store.rates().unwrap(), None);
        store.save_rates(&Rates::fixture()).unwrap();
        assert_eq!(store.rates().unwrap(), Some(Rates::fixture()));
        store.delete_rates().unwrap();
        assert_eq!(store.rates().unwrap(), None);

        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        assert!(migrate(&mut connection).is_err());
    }
}
