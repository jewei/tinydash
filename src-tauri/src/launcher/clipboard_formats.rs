//! Opt-in rich clipboard history, separate from the existing text schema.
//! Payloads live inside a private SQLite file: deleting/pruning a row also
//! securely deletes its blob; no filesystem blob names or migrations to text.
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, ensure};
use rusqlite::Connection;
#[cfg(any(test, target_os = "macos", target_os = "windows"))]
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::super::LauncherState;

#[path = "clipboard_native.rs"]
mod native;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use native::counter;
pub use native::source_app;

pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 32;
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceApp {
    pub id: String,
    pub name: String,
}

/// Exact case-insensitive identity/name matching, not substring matching.
/// Unknown provenance fails closed whenever the user configured exclusions.
pub fn excluded(source: Option<&SourceApp>, exclusions: &[String]) -> bool {
    if exclusions.is_empty() {
        return false;
    }
    let Some(source) = source else {
        return true;
    };
    exclusions.iter().any(|value| {
        let value = value.trim();
        value.eq_ignore_ascii_case(&source.id) || value.eq_ignore_ascii_case(&source.name)
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Payload {
    Png(Vec<u8>),
    Files(Vec<PathBuf>),
}

fn png_dimensions(data: &[u8]) -> anyhow::Result<(u32, u32)> {
    ensure!(
        data.len() <= MAX_IMAGE_BYTES && data.len() >= 33,
        "PNG is empty or too large."
    );
    ensure!(
        &data[..8] == b"\x89PNG\r\n\x1a\n"
            && &data[12..16] == b"IHDR"
            && data[8..12] == 13u32.to_be_bytes(),
        "Invalid PNG header."
    );
    let width = u32::from_be_bytes(data[16..20].try_into()?);
    let height = u32::from_be_bytes(data[20..24].try_into()?);
    ensure!(
        width > 0
            && height > 0
            && width <= 4096
            && height <= 4096
            && u64::from(width) * u64::from(height) <= 4_194_304,
        "PNG dimensions exceed the preview limit."
    );
    // Animated images can accumulate decoded frames. Accept only a single PNG.
    let mut cursor = 8usize;
    let mut image_data = false;
    while cursor + 12 <= data.len() {
        let size = u32::from_be_bytes(data[cursor..cursor + 4].try_into()?) as usize;
        let end = cursor
            .checked_add(size)
            .and_then(|n| n.checked_add(12))
            .context("Invalid PNG size.")?;
        ensure!(end <= data.len(), "Truncated PNG.");
        let kind = &data[cursor + 4..cursor + 8];
        ensure!(kind != b"acTL", "Animated PNG history is unsupported.");
        image_data |= kind == b"IDAT";
        if kind == b"IEND" {
            ensure!(
                size == 0 && end == data.len() && image_data,
                "Invalid PNG end."
            );
            return Ok((width, height));
        }
        cursor = end;
    }
    anyhow::bail!("Incomplete PNG.")
}

fn valid_files(paths: &[PathBuf]) -> anyhow::Result<()> {
    ensure!(
        !paths.is_empty() && paths.len() <= MAX_FILES,
        "Select between 1 and 64 file references."
    );
    let mut bytes = 0usize;
    for path in paths {
        let text = path.to_str().context("File name is not UTF-8.")?;
        bytes = bytes.saturating_add(text.len());
        ensure!(
            path.is_absolute() && !text.contains('\0') && path.exists(),
            "A referenced file is no longer available."
        );
    }
    ensure!(
        bytes <= MAX_FILE_BYTES / 2,
        "File references are too large."
    );
    Ok(())
}

impl Payload {
    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    fn encode(&self) -> anyhow::Result<(&'static str, Vec<u8>, String)> {
        match self {
            Self::Png(data) => {
                let (width, height) = png_dimensions(data)?;
                Ok((
                    "image",
                    data.clone(),
                    format!("PNG image · {width} × {height}"),
                ))
            }
            Self::Files(paths) => {
                valid_files(paths)?;
                let data = serde_json::to_vec(paths)?;
                ensure!(
                    data.len() <= MAX_FILE_BYTES,
                    "File references are too large."
                );
                Ok(("files", data, format!("{} file references", paths.len())))
            }
        }
    }
    fn decode(kind: &str, data: Vec<u8>) -> anyhow::Result<Self> {
        match kind {
            "image" => {
                png_dimensions(&data)?;
                Ok(Self::Png(data))
            }
            "files" => {
                ensure!(
                    data.len() <= MAX_FILE_BYTES,
                    "File references are too large."
                );
                let paths = serde_json::from_slice(&data)?;
                // Existence is checked again before preview/copy, not while listing.
                Ok(Self::Files(paths))
            }
            _ => anyhow::bail!("Unsupported saved clipboard format."),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichEntry {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub created_at: i64,
    pub source_app: Option<String>,
    pub bytes: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichPreview {
    pub entry: RichEntry,
    pub png: Option<Vec<u8>>,
    pub files: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichHistory {
    pub entries: Vec<RichEntry>,
    pub capture_supported: bool,
    pub support_notice: String,
}

pub struct Store {
    connection: Connection,
}
impl Store {
    fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Create with owner-only permissions before opening, rather than exposing
        // clipboard bytes during a chmod-after-open window.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(250))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        ensure!(
            version <= 1,
            "Rich clipboard history requires a newer TinyDash version."
        );
        connection.execute_batch(
            "PRAGMA secure_delete = ON; PRAGMA max_page_count = 12288;
            CREATE TABLE IF NOT EXISTS rich_clipboard (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL CHECK(kind IN ('image','files')),
                payload BLOB NOT NULL CHECK(length(payload) <= 4194304),
                title TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                source_app TEXT,
                sort_order INTEGER NOT NULL);
            PRAGMA user_version = 1;",
        )?;
        Ok(Self { connection })
    }

    fn list(&self) -> anyhow::Result<Vec<RichEntry>> {
        let mut statement = self.connection.prepare("SELECT id,kind,title,created_at,source_app,length(payload) FROM rich_clipboard ORDER BY sort_order DESC")?;
        Ok(statement
            .query_map([], entry)?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn prune(&self, days: u32, now: i64, limit: usize) -> anyhow::Result<()> {
        Self::prune_connection(&self.connection, days, now, limit)
    }

    fn prune_connection(
        connection: &Connection,
        days: u32,
        now: i64,
        limit: usize,
    ) -> anyhow::Result<()> {
        if days > 0 {
            connection.execute(
                "DELETE FROM rich_clipboard WHERE created_at <= ?1",
                [now.saturating_sub(i64::from(days) * 86_400)],
            )?;
        }
        connection.execute("DELETE FROM rich_clipboard WHERE id IN (SELECT id FROM rich_clipboard ORDER BY sort_order DESC LIMIT -1 OFFSET ?1)", [limit.min(MAX_ENTRIES) as i64])?;
        // The window sum avoids reading image blobs while enforcing total bytes.
        connection.execute("DELETE FROM rich_clipboard WHERE id IN (SELECT id FROM (SELECT id, SUM(length(payload)) OVER (ORDER BY sort_order DESC) AS bytes FROM rich_clipboard) WHERE bytes > ?1)", [MAX_TOTAL_BYTES as i64])?;
        Ok(())
    }

    #[cfg(any(test, target_os = "macos", target_os = "windows"))]
    fn capture(
        &mut self,
        payload: &Payload,
        source: Option<&SourceApp>,
        now: i64,
        days: u32,
        limit: usize,
    ) -> anyhow::Result<()> {
        let (kind, data, title) = payload.encode()?;
        let transaction = self.connection.transaction()?;
        Self::prune_connection(&transaction, days, now, limit)?;
        // Deduplicate exact payloads without hashes/collision ambiguity.
        let existing: Option<i64> = transaction
            .query_row(
                "SELECT id FROM rich_clipboard WHERE kind = ?1 AND payload = ?2",
                params![kind, data],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            transaction.execute("UPDATE rich_clipboard SET sort_order = (SELECT COALESCE(MAX(sort_order),0)+1 FROM rich_clipboard) WHERE id = ?1", [id])?;
        } else {
            transaction.execute("INSERT INTO rich_clipboard(kind,payload,title,created_at,source_app,sort_order) VALUES(?1,?2,?3,?4,?5,(SELECT COALESCE(MAX(sort_order),0)+1 FROM rich_clipboard))", params![kind,data,title,now,source.map(|s| &s.id)])?;
        }
        Self::prune_connection(&transaction, days, now, limit)?;
        transaction.commit()?;
        Ok(())
    }

    fn preview(&self, id: i64) -> anyhow::Result<RichPreview> {
        ensure!(id > 0, "Invalid clipboard entry.");
        let (entry, bytes): (RichEntry, Vec<u8>) = self.connection.query_row("SELECT id,kind,title,created_at,source_app,length(payload),payload FROM rich_clipboard WHERE id = ?1", [id], |row| Ok((entry(row)?, row.get(6)?))).context("This clipboard entry is no longer available.")?;
        let (png, files) = match Payload::decode(&entry.kind, bytes)? {
            Payload::Png(bytes) => (Some(bytes), None),
            Payload::Files(paths) => {
                valid_files(&paths)?;
                (
                    None,
                    Some(
                        paths
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect(),
                    ),
                )
            }
        };
        Ok(RichPreview { entry, png, files })
    }

    fn delete(&self, id: Option<i64>) -> anyhow::Result<()> {
        if let Some(id) = id {
            ensure!(id > 0, "Invalid clipboard entry.");
            self.connection
                .execute("DELETE FROM rich_clipboard WHERE id = ?1", [id])?;
        } else {
            self.connection.execute("DELETE FROM rich_clipboard", [])?;
        }
        Ok(())
    }
}

fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<RichEntry> {
    Ok(RichEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        source_app: row.get(4)?,
        bytes: row.get(5)?,
    })
}

fn with_store<T>(
    app: &AppHandle,
    action: impl FnOnce(&mut Store) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let state = app.state::<LauncherState>();
    let mut guard = state
        .clipboard
        .rich
        .lock()
        .map_err(|_| anyhow::anyhow!("Rich clipboard storage is unavailable."))?;
    if guard.is_none() {
        *guard = Some(Store::open(
            &app.path().app_data_dir()?.join("clipboard-rich.sqlite3"),
        )?);
    }
    let store = guard
        .as_mut()
        .context("Rich clipboard storage is unavailable.")?;
    let settings = state.settings();
    if settings.clipboard_history_decided {
        store.prune(
            settings.clipboard_retention_days,
            crate::ranking::now(),
            settings.clipboard_limit(),
        )?;
    }
    action(store)
}

/// Called only after the regular snapshot passed its secret checks. The native
/// reader checks markers and the change counter again before touching payloads.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn capture(
    app: &AppHandle,
    counter: u64,
    generation: u64,
    source: Option<&SourceApp>,
) -> anyhow::Result<()> {
    let state = app.state::<LauncherState>();
    let settings = state.settings();
    if !settings.clipboard_history_enabled || excluded(source, &settings.clipboard_excluded_apps) {
        return Ok(());
    }
    let Some(payload) = native::read(
        counter,
        settings.clipboard_capture_images,
        settings.clipboard_capture_files,
    )?
    else {
        return Ok(());
    };
    // Match settings application lock order: settings_update, then rich store.
    // Hold only for the final policy recheck + save, never the native read.
    let _settings_update = state
        .settings_update
        .lock()
        .map_err(|_| anyhow::anyhow!("Clipboard settings are unavailable."))?;
    with_store(app, |store| {
        let settings = state.settings();
        if generation != state.clipboard.generation()
            || !settings.clipboard_history_enabled
            || excluded(source, &settings.clipboard_excluded_apps)
        {
            return Ok(());
        }
        let enabled = match &payload {
            Payload::Png(_) => settings.clipboard_capture_images,
            Payload::Files(_) => settings.clipboard_capture_files,
        };
        if enabled {
            store.capture(
                &payload,
                source,
                crate::ranking::now(),
                settings.clipboard_retention_days,
                settings.clipboard_limit(),
            )?;
            super::changed(app);
        }
        Ok(())
    })
}

/// Settings/startup/periodic pruning does not create a rich database when the
/// user has never opted in. Call after saving a new retention/count policy.
pub fn apply_retention(app: &AppHandle) -> anyhow::Result<()> {
    if app
        .path()
        .app_data_dir()?
        .join("clipboard-rich.sqlite3")
        .exists()
    {
        with_store(app, |_| Ok(()))?;
    }
    Ok(())
}

pub fn clear(app: &AppHandle) -> anyhow::Result<()> {
    if app
        .path()
        .app_data_dir()?
        .join("clipboard-rich.sqlite3")
        .exists()
    {
        with_store(app, |store| store.delete(None))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn rich_clipboard_history(app: AppHandle) -> Result<RichHistory, String> {
    tauri::async_runtime::spawn_blocking(move || with_store(&app, |store| Ok(RichHistory {
        entries: store.list()?,
        capture_supported: cfg!(target_os = "macos"),
        support_notice: if cfg!(target_os = "macos") {
            "Captures native PNG images and Finder file lists on macOS. TIFF-only images and URL-only file sources are skipped. Images: up to 4 MiB and 4 megapixels. Files: up to 64 existing paths, not file contents. Rich history: up to 32 entries and 16 MiB; no pins.".into()
        } else { "Image and file capture/copy is currently supported only on macOS. Text history remains available.".into() },
    })).map_err(|error| error.to_string())).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn rich_clipboard_preview(id: i64, app: AppHandle) -> Result<RichPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| store.preview(id)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn copy_rich_clipboard(id: i64, app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| {
            // The store mutex serializes copies with delete/prune/capture.
            let preview = store.preview(id)?;
            let payload = if let Some(png) = preview.png {
                Payload::Png(png)
            } else {
                Payload::Files(
                    preview
                        .files
                        .context("Missing file references.")?
                        .into_iter()
                        .map(PathBuf::from)
                        .collect(),
                )
            };
            native::write(&payload)?;
            app.state::<LauncherState>().clipboard.invalidate();
            Ok(())
        })
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_rich_clipboard(id: i64, app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| store.delete(Some(id))).map_err(|error| error.to_string())?;
        app.state::<LauncherState>().clipboard.invalidate();
        super::changed(&app);
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("rich.sqlite3")).unwrap();
        (dir, store)
    }
    #[test]
    fn clipboard_exclusions_are_exact_and_fail_closed_for_unknown_sources() {
        let app = SourceApp {
            id: "com.example.Editor".into(),
            name: "Editor".into(),
        };
        assert!(excluded(Some(&app), &[" COM.EXAMPLE.EDITOR ".into()]));
        assert!(excluded(Some(&app), &["editor".into()]));
        assert!(!excluded(Some(&app), &["edit".into()]));
        assert!(excluded(None, &["editor".into()]));
        assert!(!excluded(None, &[]));
    }
    #[test]
    fn clipboard_rich_files_persist_deduplicate_expire_and_revalidate() {
        let (dir, mut store) = store();
        let file = dir.path().join("fixture.txt");
        std::fs::write(&file, "not copied into history").unwrap();
        let payload = Payload::Files(vec![file.clone()]);
        store.capture(&payload, None, 100, 0, 100).unwrap();
        store.capture(&payload, None, 101, 0, 100).unwrap();
        let entries = store.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].created_at, 100);
        assert!(store.preview(entries[0].id).unwrap().png.is_none());
        drop(store);
        let store = Store::open(&dir.path().join("rich.sqlite3")).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        std::fs::remove_file(file).unwrap();
        assert!(store.preview(entries[0].id).is_err());
        store.prune(1, 86_500, 100).unwrap();
        assert!(store.list().unwrap().is_empty());
    }
    #[test]
    fn clipboard_rich_store_enforces_count_and_deletes_payloads() {
        let (dir, mut store) = store();
        for index in 0..40 {
            let file = dir.path().join(index.to_string());
            std::fs::write(&file, "fixture").unwrap();
            store
                .capture(&Payload::Files(vec![file]), None, index, 0, 100)
                .unwrap();
        }
        assert_eq!(store.list().unwrap().len(), MAX_ENTRIES);
        store.prune(0, 100, 2).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
        store.delete(None).unwrap();
        let bytes: i64 = store
            .connection
            .query_row(
                "SELECT COALESCE(SUM(length(payload)),0) FROM rich_clipboard",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bytes, 0);
        assert!(store.list().unwrap().is_empty());
    }
    #[test]
    fn clipboard_rich_rejects_invalid_payloads_without_writing() {
        let (_dir, mut store) = store();
        for bytes in [vec![], vec![0; MAX_IMAGE_BYTES + 1], b"not a png".to_vec()] {
            assert!(
                store
                    .capture(&Payload::Png(bytes), None, 1, 0, 100)
                    .is_err()
            );
        }
        assert!(
            store
                .capture(
                    &Payload::Files(vec![PathBuf::from("relative")]),
                    None,
                    1,
                    0,
                    100
                )
                .is_err()
        );
        assert!(store.list().unwrap().is_empty());
    }
    #[test]
    fn clipboard_rich_png_preview_is_bounded_and_original_bytes_survive() {
        let (_dir, mut store) = store();
        let png = include_bytes!("../../icons/32x32.png").to_vec();
        assert_eq!(png_dimensions(&png).unwrap(), (32, 32));
        store
            .capture(&Payload::Png(png.clone()), None, 1, 0, 100)
            .unwrap();
        let id = store.list().unwrap()[0].id;
        assert_eq!(store.preview(id).unwrap().png.unwrap(), png);
        let mut oversized = png.clone();
        oversized[16..20].copy_from_slice(&5000u32.to_be_bytes());
        assert!(png_dimensions(&oversized).is_err());
        let mut truncated = png.clone();
        truncated.pop();
        assert!(png_dimensions(&truncated).is_err());
        let mut animated = png;
        // Inject an animation-control chunk ahead of IDAT. CRC deliberately
        // irrelevant here: the history parser rejects the format itself.
        animated.splice(33..33, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
        assert!(png_dimensions(&animated).is_err());
    }

    #[test]
    fn clipboard_rich_pruning_enforces_total_blob_budget() {
        let (_dir, store) = store();
        for order in 0..6 {
            store.connection.execute("INSERT INTO rich_clipboard(kind,payload,title,created_at,sort_order) VALUES('image',zeroblob(?1),'fixture',1,?2)", params![MAX_IMAGE_BYTES as i64, order]).unwrap();
        }
        store.prune(0, 1, 100).unwrap();
        assert_eq!(store.list().unwrap().len(), 4);
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(|entry| u64::from(entry.bytes))
                .sum::<u64>(),
            MAX_TOTAL_BYTES as u64
        );
        assert_eq!(store.list().unwrap()[0].id, 6);
    }

    #[test]
    fn clipboard_rich_newer_database_is_not_replaced() {
        let (dir, store) = store();
        store
            .connection
            .pragma_update(None, "user_version", 2)
            .unwrap();
        drop(store);
        assert!(Store::open(&dir.path().join("rich.sqlite3")).is_err());
        let connection = Connection::open(dir.path().join("rich.sqlite3")).unwrap();
        assert_eq!(
            connection
                .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            2
        );
    }
}
