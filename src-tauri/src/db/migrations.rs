use rusqlite::{Connection, TransactionBehavior};

use super::{Error, Result};

const MIGRATIONS: &[&str] = &["CREATE TABLE usage_history (
        result_id TEXT PRIMARY KEY NOT NULL,
        use_count INTEGER NOT NULL CHECK (use_count BETWEEN 1 AND 4294967295),
        last_used_at INTEGER NOT NULL CHECK (last_used_at >= 0)
    ) STRICT;"];

pub fn apply(connection: &mut Connection) -> Result<()> {
    migrate(connection, MIGRATIONS)
}

fn migrate(connection: &mut Connection, migrations: &[&str]) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version as usize > migrations.len() {
        return Err(Error::NewerSchema {
            found: version,
            supported: migrations.len(),
        });
    }
    for (index, sql) in migrations.iter().enumerate().skip(version as usize) {
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", (index + 1) as u32)?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_migration_rolls_back_schema_and_version() {
        let mut connection = Connection::open_in_memory().expect("database");
        let migrations = ["CREATE TABLE first (id INTEGER);", "invalid SQL"];
        assert!(migrate(&mut connection, &migrations).is_err());
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert_eq!(version, 0);
        assert!(connection.prepare("SELECT * FROM first").is_err());
    }

    #[test]
    fn upgrades_existing_schema_once_and_preserves_data() {
        let mut connection = Connection::open_in_memory().expect("database");
        let migrations = [
            "CREATE TABLE example (value TEXT); INSERT INTO example VALUES ('kept');",
            "ALTER TABLE example ADD COLUMN count INTEGER NOT NULL DEFAULT 1;",
        ];
        migrate(&mut connection, &migrations[..1]).expect("first version");
        migrate(&mut connection, &migrations).expect("upgrade");
        migrate(&mut connection, &migrations).expect("repeat");
        let value: (String, u32) = connection
            .query_row("SELECT value, count FROM example", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("row");
        assert_eq!(value, ("kept".into(), 1));
    }

    #[test]
    fn refuses_newer_schema_without_changes() {
        let mut connection = Connection::open_in_memory().expect("database");
        connection
            .pragma_update(None, "user_version", 99)
            .expect("version");
        assert!(matches!(
            apply(&mut connection),
            Err(Error::NewerSchema { found: 99, .. })
        ));
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert_eq!(version, 99);
    }
}
