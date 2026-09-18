use std::collections::HashSet;

use super::{Database, Result};

impl Database {
    pub fn load_pins(&self) -> Result<HashSet<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT result_id FROM pinned_apps")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<()> {
        self.connection.execute(
            if pinned {
                "INSERT INTO pinned_apps (result_id) VALUES (?1) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM pinned_apps WHERE result_id = ?1"
            },
            [id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_survive_reopen_and_repeated_writes_without_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let id = "app:/Applications/Alice's app.app";
        {
            let database = Database::open(&path).unwrap();
            database.set_pinned(id, true).unwrap();
            database.set_pinned(id, true).unwrap();
        }
        let database = Database::open(&path).unwrap();
        assert_eq!(database.load_pins().unwrap(), HashSet::from([id.into()]));
        database.set_pinned(id, false).unwrap();
        database.set_pinned(id, false).unwrap();
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
        database.set_pinned("app:kept", true).unwrap();
        let lock = rusqlite::Connection::open(&path).unwrap();
        lock.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(database.set_pinned("app:kept", false).is_err());
        assert!(database.set_pinned("app:new", true).is_err());
        assert_eq!(
            database.load_pins().unwrap(),
            HashSet::from(["app:kept".into()])
        );
    }
}
