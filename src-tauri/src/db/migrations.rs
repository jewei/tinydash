use rusqlite::{Connection, TransactionBehavior};

use super::{Error, Result};

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE usage_history (
        result_id TEXT PRIMARY KEY NOT NULL,
        use_count INTEGER NOT NULL CHECK (use_count BETWEEN 1 AND 4294967295),
        last_used_at INTEGER NOT NULL CHECK (last_used_at >= 0)
    ) STRICT;",
    "CREATE TABLE clipboard_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        content TEXT NOT NULL UNIQUE CHECK (length(CAST(content AS BLOB)) BETWEEN 1 AND 16384),
        created_at INTEGER NOT NULL CHECK (created_at >= 0),
        last_used_at INTEGER CHECK (last_used_at >= 0),
        pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
        sort_order INTEGER NOT NULL
    ) STRICT;",
    "CREATE TABLE currency_rates (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        snapshot TEXT NOT NULL CHECK (length(snapshot) BETWEEN 1 AND 65536)
    ) STRICT;",
];

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
    fn adds_clipboard_to_the_usage_database_without_losing_usage() {
        let mut connection = Connection::open_in_memory().expect("database");
        migrate(&mut connection, &MIGRATIONS[..1]).expect("phase 4 schema");
        connection
            .execute("INSERT INTO usage_history VALUES ('emoji:🚀', 3, 100)", [])
            .expect("usage");
        apply(&mut connection).expect("phase 5 schema");
        apply(&mut connection).expect("idempotent");
        assert_eq!(
            connection
                .query_row("SELECT use_count FROM usage_history", [], |row| row
                    .get::<_, u32>(0))
                .expect("usage"),
            3
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM clipboard_history", [], |row| row
                    .get::<_, u32>(0))
                .expect("clipboard"),
            0
        );
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
