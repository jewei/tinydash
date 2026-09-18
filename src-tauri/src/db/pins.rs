use rusqlite::params;

use super::{Database, Result};
use crate::launcher::{pins::Pins, query::SearchMode};

impl Database {
    pub fn load_pins(&self) -> Result<Pins> {
        let mut statement = self
            .connection
            .prepare("SELECT category, result_id FROM pinned_items ORDER BY category, result_id")?;
        let rows = statement.query_map([], |row| {
            let category = serde_json::from_value(serde_json::Value::String(row.get(0)?)).map_err(
                |error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                },
            )?;
            Ok((category, row.get::<_, String>(1)?))
        })?;
        let mut pins = Pins::new();
        for row in rows {
            let (category, id) = row?;
            pins.entry(category).or_default().insert(id);
        }
        Ok(pins)
    }

    pub fn set_pinned(&self, id: &str, category: SearchMode, pinned: bool) -> Result<()> {
        self.connection.execute(
            if pinned {
                "INSERT INTO pinned_items (category, result_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM pinned_items WHERE category = ?1 AND result_id = ?2"
            },
            params![category.as_str(), id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn pins_survive_reopen_and_repeated_writes_without_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let id = "app:/Applications/Alice's app.app";
        {
            let database = Database::open(&path).unwrap();
            database.set_pinned(id, SearchMode::Apps, true).unwrap();
            database.set_pinned(id, SearchMode::Apps, true).unwrap();
        }
        let database = Database::open(&path).unwrap();
        assert_eq!(
            database.load_pins().unwrap(),
            Pins::from([(SearchMode::Apps, HashSet::from([id.into()]))])
        );
        database.set_pinned(id, SearchMode::Apps, false).unwrap();
        database.set_pinned(id, SearchMode::Apps, false).unwrap();
        drop(database);
        assert!(
            Database::open(&path)
                .unwrap()
                .load_pins()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_write_preserves_saved_pins() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let database = Database::open(&path).unwrap();
        database
            .set_pinned("app:kept", SearchMode::All, true)
            .unwrap();
        let lock = rusqlite::Connection::open(&path).unwrap();
        lock.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(
            database
                .set_pinned("app:kept", SearchMode::All, false)
                .is_err()
        );
        assert!(
            database
                .set_pinned("app:new", SearchMode::Apps, true)
                .is_err()
        );
        assert_eq!(
            database.load_pins().unwrap(),
            Pins::from([(SearchMode::All, HashSet::from(["app:kept".into()]))])
        );
    }

    #[test]
    fn categories_persist_independently_and_clipboard_pins_protect_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let mut database = Database::open(&path).unwrap();
        let clip = database
            .capture_clipboard("Keep this text", 1, 1)
            .unwrap()
            .0;
        let key = format!("clipboard:{}", clip.id);
        database.set_pinned(&key, SearchMode::All, true).unwrap();
        database
            .set_pinned(&key, SearchMode::Clipboard, true)
            .unwrap();
        database.set_pinned(&key, SearchMode::All, false).unwrap();
        drop(database);
        let mut database = Database::open(&path).unwrap();
        assert_eq!(
            database.load_pins().unwrap(),
            Pins::from([(SearchMode::Clipboard, HashSet::from([key.clone()]))])
        );
        database.capture_clipboard("Second", 2, 1).unwrap();
        database.capture_clipboard("Third", 3, 1).unwrap();
        database.prune_clipboard(1).unwrap();
        assert!(
            database
                .load_clipboard()
                .unwrap()
                .iter()
                .any(|entry| entry.id == clip.id)
        );
        database
            .set_pinned(&key, SearchMode::Clipboard, false)
            .unwrap();
        database.prune_clipboard(1).unwrap();
        assert_eq!(database.load_clipboard().unwrap().len(), 1);
        assert!(database.load_pins().unwrap().is_empty());
        let id = database.load_clipboard().unwrap()[0].id;
        database
            .set_pinned(&format!("clipboard:{id}"), SearchMode::All, true)
            .unwrap();
        database.clear_clipboard().unwrap();
        assert!(database.load_pins().unwrap().is_empty());
    }
}
