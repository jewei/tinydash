mod backup;
mod clipboard;
mod currency;
mod migrations;
mod pins;

pub use clipboard::{CaptureRevision, CleanupOutcome};

use std::{collections::HashMap, path::Path, time::Duration};

use rusqlite::{Connection, params};

use crate::ranking::Usage;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not access the database file: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Stored currency rates are invalid: {0}")]
    Currency(String),
    #[error("Database version {found} is newer than supported version {supported}")]
    NewerSchema { found: u32, supported: usize },
    #[error("Could not make a recovery backup. Your data was not migrated: {0}")]
    Backup(String),
}

impl Error {
    /// Only SQLite lock contention is safe to retry without manual recovery.
    /// Match primary codes, including extended BUSY/LOCKED variants.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Sqlite(rusqlite::Error::SqliteFailure(error, _))
            if matches!(error.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Database {
    connection: Connection,
}

impl Database {
    #[cfg(test)]
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_settings(path, &path.with_file_name("settings.json"))
    }

    pub fn open_with_settings(path: &Path, settings: &Path) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(path)?.permissions();
            permissions.set_mode(permissions.mode() & 0o600);
            std::fs::set_permissions(path, permissions)?;
        }
        connection.busy_timeout(Duration::from_millis(250))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version as usize > migrations::VERSION {
            return Err(Error::NewerSchema {
                found: version,
                supported: migrations::VERSION,
            });
        }
        if version > 0 && (version as usize) < migrations::VERSION {
            backup::create(&connection, path, settings)?;
        }
        connection.pragma_update(None, "secure_delete", true)?;
        // One connection and infrequent writes need no WAL or background checkpoint.
        // Keep SQLite's default durable transactions.
        migrations::apply(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn load_usage(&self) -> Result<HashMap<String, Usage>> {
        let mut statement = self
            .connection
            .prepare("SELECT result_id, use_count, last_used_at FROM usage_history")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                Usage {
                    count: row.get(1)?,
                    last_used_at: row.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_usage_batch(&mut self, usage: &HashMap<String, Usage>) -> Result<()> {
        if usage.is_empty() {
            return Ok(());
        }
        // This method owns the transaction; save_usage does not start one.
        let transaction = self.connection.unchecked_transaction()?;
        for (id, usage) in usage {
            self.save_usage(id, *usage)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn save_usage(&self, id: &str, usage: Usage) -> Result<()> {
        self.connection.execute(
            "INSERT INTO usage_history (result_id, use_count, last_used_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(result_id) DO UPDATE SET
                 use_count = excluded.use_count, last_used_at = excluded.last_used_at",
            params![id, usage.count, usage.last_used_at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_busy_and_locked_errors_are_transient() {
        for code in [
            rusqlite::ffi::SQLITE_BUSY,
            rusqlite::ffi::SQLITE_BUSY_SNAPSHOT,
            rusqlite::ffi::SQLITE_LOCKED,
            rusqlite::ffi::SQLITE_LOCKED_SHAREDCACHE,
        ] {
            assert!(
                Error::Sqlite(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(code),
                    None
                ))
                .is_transient()
            );
        }
        for code in [
            rusqlite::ffi::SQLITE_CORRUPT,
            rusqlite::ffi::SQLITE_NOTADB,
            rusqlite::ffi::SQLITE_READONLY,
            rusqlite::ffi::SQLITE_FULL,
            rusqlite::ffi::SQLITE_CONSTRAINT,
        ] {
            assert!(
                !Error::Sqlite(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(code),
                    None
                ))
                .is_transient()
            );
        }
        assert!(!Error::Backup("database is locked".into()).is_transient());
        assert!(
            !Error::NewerSchema {
                found: 999,
                supported: 5
            }
            .is_transient()
        );
    }

    #[test]
    fn usage_batch_is_atomic_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let mut database = Database::open(&directory.path().join("state.sqlite3")).unwrap();
        let entries = HashMap::from([
            (
                "app:good".into(),
                Usage {
                    count: 1,
                    last_used_at: 100,
                },
            ),
            (
                "app:invalid".into(),
                Usage {
                    count: 0,
                    last_used_at: -1,
                },
            ),
        ]);
        assert!(database.save_usage_batch(&entries).is_err());
        assert!(database.load_usage().unwrap().is_empty());
    }

    #[test]
    fn usage_survives_reopen_and_upserts_without_duplicate_rows() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("data/tinydash.sqlite3");
        let id = "app:/an app's path; DROP TABLE usage_history;";
        {
            let database = Database::open(&path).expect("open");
            assert!(database.load_usage().expect("load").is_empty());
            database
                .save_usage(
                    id,
                    Usage {
                        count: 1,
                        last_used_at: 100,
                    },
                )
                .expect("insert");
            database
                .save_usage(
                    id,
                    Usage {
                        count: 2,
                        last_used_at: 200,
                    },
                )
                .expect("update");
            database
                .save_usage(
                    "emoji:🚀",
                    Usage {
                        count: 1,
                        last_used_at: 150,
                    },
                )
                .expect("emoji");
        }
        let database = Database::open(&path).expect("reopen");
        let usage = database.load_usage().expect("load");
        assert_eq!(usage.len(), 2);
        assert_eq!(
            usage[id],
            Usage {
                count: 2,
                last_used_at: 200
            }
        );
        assert_eq!(usage["emoji:🚀"].count, 1);
    }

    #[test]
    fn invalid_usage_and_locked_writes_do_not_replace_good_data() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("state.sqlite3");
        let database = Database::open(&path).expect("database");
        let good = Usage {
            count: 1,
            last_used_at: 100,
        };
        database.save_usage("app:test", good).expect("insert");
        assert!(
            database
                .save_usage(
                    "app:test",
                    Usage {
                        count: 0,
                        last_used_at: -1
                    }
                )
                .is_err()
        );
        let lock = Connection::open(&path).expect("second connection");
        lock.execute_batch("BEGIN IMMEDIATE").expect("lock");
        assert!(
            database
                .save_usage(
                    "app:test",
                    Usage {
                        count: 2,
                        last_used_at: 200
                    }
                )
                .is_err()
        );
        lock.execute_batch("ROLLBACK").expect("unlock");
        assert_eq!(database.load_usage().expect("load")["app:test"], good);
    }

    #[test]
    fn corrupt_database_is_not_deleted_or_replaced() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("state.sqlite3");
        std::fs::write(&path, "not a database").expect("write");
        assert!(Database::open(&path).is_err());
        assert_eq!(
            std::fs::read_to_string(path).expect("read"),
            "not a database"
        );
    }
}
