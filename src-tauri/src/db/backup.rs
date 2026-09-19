//! One atomic, private archive containing a consistent SQLite snapshot and settings.
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, ensure};
use rusqlite::Connection;

use super::{Error, Result, migrations};

pub(super) fn create(connection: &Connection, database: &Path, settings: &Path) -> Result<()> {
    create_archive(connection, database, settings)
        .map_err(|error| Error::Backup(format!("{error:#}")))
}

fn create_archive(connection: &Connection, database: &Path, settings: &Path) -> anyhow::Result<()> {
    let directory = database
        .parent()
        .context("Database has no parent directory")?;
    // NamedTempFile restricts Unix permissions to 0600. On Windows the archive
    // stays inside the current user's application-data directory.
    let snapshot = tempfile::NamedTempFile::new_in(directory)?;
    connection.backup(rusqlite::MAIN_DB, snapshot.path(), None)?;
    let check =
        Connection::open_with_flags(snapshot.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = check.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    ensure!(
        integrity == "ok",
        "The database snapshot failed its integrity check"
    );
    let schema: u32 = check.pragma_query_value(None, "user_version", |row| row.get(0))?;
    drop(check);
    let settings_bytes = match std::fs::read(settings) {
        Ok(bytes) => {
            let json: serde_json::Value =
                serde_json::from_slice(&bytes).context("Settings are not valid JSON")?;
            ensure!(json.is_object(), "Settings must be a JSON object");
            Some(bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("Read settings for the backup"),
    };
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "format": 1,
        "createdByVersion": env!("CARGO_PKG_VERSION"),
        "sourceSchema": schema,
        "targetSchema": migrations::VERSION,
        "createdAt": chrono::Utc::now().to_rfc3339(),
        "settingsPresent": settings_bytes.is_some(),
    }))?;
    let mut archive = tempfile::NamedTempFile::new_in(directory)?;
    {
        let mut builder = tar::Builder::new(archive.as_file_mut());
        builder.append_path_with_name(snapshot.path(), "tinydash.sqlite3")?;
        if let Some(bytes) = &settings_bytes {
            append(&mut builder, "settings.json", bytes)?;
        }
        append(&mut builder, "manifest.json", &manifest)?;
        builder.finish()?;
    }
    archive.as_file_mut().flush()?;
    archive.as_file().sync_all()?;
    archive.as_file_mut().seek(SeekFrom::Start(0))?;
    // Read every entry before replacing the previous verified archive.
    let mut reader = tar::Archive::new(archive.as_file_mut());
    let mut count = 0;
    for entry in reader.entries()? {
        let mut entry = entry?;
        let expected = entry.size();
        ensure!(
            std::io::copy(&mut entry, &mut std::io::sink())? == expected,
            "Incomplete backup entry"
        );
        count += 1;
    }
    ensure!(
        count == if settings_bytes.is_some() { 3 } else { 2 },
        "Incomplete backup archive"
    );
    // Atomic replacement keeps the previous backup if any earlier step fails.
    archive
        .persist(directory.join("recovery.tar"))
        .context("Replace the recovery archive")?;
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    Ok(())
}

fn append(builder: &mut tar::Builder<&mut File>, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder.append_data(&mut header, name, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn old_database(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch("CREATE TABLE usage_history (result_id TEXT PRIMARY KEY, use_count INTEGER, last_used_at INTEGER); INSERT INTO usage_history VALUES ('app:kept', 4, 123); PRAGMA user_version=1;").unwrap();
    }

    #[test]
    fn upgrade_keeps_a_readable_private_snapshot_and_exact_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tinydash.sqlite3");
        let settings = dir.path().join("settings.json");
        let original = br#"{"clipboardHistoryEnabled":false,"unknownFutureSetting":42}"#;
        std::fs::write(&settings, original).unwrap();
        old_database(&path);
        let database = Database::open_with_settings(&path, &settings).unwrap();
        assert_eq!(database.load_usage().unwrap()["app:kept"].count, 4);
        let archive_path = dir.path().join("recovery.tar");
        let first_backup = std::fs::read(&archive_path).unwrap();
        let restore = tempfile::tempdir().unwrap();
        tar::Archive::new(File::open(&archive_path).unwrap())
            .unpack(restore.path())
            .unwrap();
        assert_eq!(
            std::fs::read(restore.path().join("settings.json")).unwrap(),
            original
        );
        let snapshot = Connection::open(restore.path().join("tinydash.sqlite3")).unwrap();
        assert_eq!(
            snapshot
                .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            snapshot
                .query_row("SELECT use_count FROM usage_history", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            4
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&archive_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(database);
        Database::open_with_settings(&path, &settings).unwrap();
        assert_eq!(
            std::fs::read(&archive_path).unwrap(),
            first_backup,
            "Normal startup must retain the pre-migration backup"
        );
    }

    #[test]
    fn failed_backup_does_not_migrate_or_replace_the_previous_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tinydash.sqlite3");
        let settings = dir.path().join("settings.json");
        old_database(&path);
        let backup = dir.path().join("recovery.tar");
        std::fs::write(&backup, "previous verified backup").unwrap();
        std::fs::write(&settings, "damaged JSON").unwrap();
        assert!(matches!(
            Database::open_with_settings(&path, &settings),
            Err(Error::Backup(_))
        ));
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "previous verified backup"
        );
        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert!(
            connection
                .prepare("SELECT * FROM clipboard_history")
                .is_err()
        );
        std::fs::write(&settings, "{}").unwrap();
        std::fs::remove_file(&backup).unwrap();
        std::fs::create_dir(&backup).unwrap();
        assert!(matches!(
            Database::open_with_settings(&path, &settings),
            Err(Error::Backup(_))
        ));
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn newer_and_unreadable_databases_leave_data_and_the_backup_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tinydash.sqlite3");
        let settings = dir.path().join("settings.json");
        let archive = dir.path().join("recovery.tar");
        std::fs::write(&archive, "previous verified backup").unwrap();
        let connection = Connection::open(&path).unwrap();
        connection.pragma_update(None, "user_version", 999).unwrap();
        drop(connection);
        let original = std::fs::read(&path).unwrap();
        assert!(matches!(
            Database::open_with_settings(&path, &settings),
            Err(Error::NewerSchema { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::write(&path, "unreadable database").unwrap();
        assert!(Database::open_with_settings(&path, &settings).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "unreadable database"
        );
        assert_eq!(
            std::fs::read_to_string(&archive).unwrap(),
            "previous verified backup"
        );
    }
}
