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
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use unicode_normalization::UnicodeNormalization;

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RichKind {
    Image,
    Files,
}

impl RichKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Files => "files",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RichEntry {
    pub id: i64,
    pub kind: RichKind,
    pub title: String,
    pub created_at: i64,
    pub source_app: Option<String>,
    pub bytes: u32,
    pub pinned: bool,
    pub custom_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RichPreview {
    pub entry: RichEntry,
    pub png: Option<Vec<u8>>,
    pub files: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RichHistory {
    pub entries: Vec<RichEntry>,
    pub total: u32,
    pub source_apps: Vec<String>,
    pub capture_supported: bool,
    pub support_notice: String,
    pub storage_notice: String,
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
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(250))?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        ensure!(
            version <= 3,
            "Rich clipboard history requires a newer TinyDash version."
        );
        connection.execute_batch("PRAGMA secure_delete = ON; PRAGMA max_page_count = 12288;")?;
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS rich_clipboard (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL CHECK(kind IN ('image','files')),
                payload BLOB NOT NULL CHECK(length(payload) <= 4194304),
                title TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                source_app TEXT,
                sort_order INTEGER NOT NULL);",
        )?;
        if version < 2 {
            transaction.execute_batch("ALTER TABLE rich_clipboard ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1));")?;
        }
        if version < 3 {
            transaction.execute_batch("ALTER TABLE rich_clipboard ADD COLUMN custom_name TEXT CHECK(custom_name IS NULL OR (length(custom_name) <= 120 AND length(CAST(custom_name AS BLOB)) <= 480));")?;
        }
        transaction.pragma_update(None, "user_version", 3)?;
        transaction.commit()?;
        Ok(Self { connection })
    }

    fn list(&self) -> anyhow::Result<Vec<RichEntry>> {
        let mut statement = self.connection.prepare("SELECT id,kind,title,created_at,source_app,length(payload),pinned,custom_name FROM rich_clipboard ORDER BY pinned DESC,sort_order DESC")?;
        Ok(statement
            .query_map([], entry)?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn search(
        &self,
        query: &str,
        kind: Option<RichKind>,
        source_app: Option<&str>,
    ) -> anyhow::Result<(Vec<RichEntry>, u32, Vec<String>)> {
        ensure!(query.len() <= 1024, "Search is limited to 1 KiB of text.");
        ensure!(
            source_app.is_none_or(|source| source.len() <= 1024),
            "Source app filter is too long."
        );
        let entries = self.list()?;
        let total = entries.len() as u32;
        let source_apps = entries
            .iter()
            .filter_map(|entry| entry.source_app.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let normalize = |text: &str| text.nfc().collect::<String>().to_lowercase();
        let query = normalize(query);
        let terms: Vec<_> = query.split_whitespace().collect();
        let source = source_app.map(normalize);
        let mut matches = Vec::new();
        for entry in entries {
            if kind.is_some_and(|kind| kind != entry.kind)
                || source.as_ref().is_some_and(|source| {
                    entry
                        .source_app
                        .as_deref()
                        .is_none_or(|app| normalize(app) != *source)
                })
            {
                continue;
            }
            let mut text = normalize(&format!(
                "{} {} {}",
                entry.title,
                entry.custom_name.as_deref().unwrap_or_default(),
                entry.source_app.as_deref().unwrap_or_default()
            ));
            if !terms.iter().all(|term| text.contains(term)) && entry.kind == RichKind::Files {
                // Read bounded file metadata only. Never load image blobs or check
                // the filesystem for search: missing references must stay findable.
                let data = self.connection.query_row(
                    "SELECT payload FROM rich_clipboard WHERE id = ?1",
                    [entry.id],
                    |row| row.get::<_, Vec<u8>>(0),
                )?;
                if let Payload::Files(paths) = Payload::decode("files", data)? {
                    for path in paths {
                        text.push('\n');
                        text.push_str(&normalize(&path.to_string_lossy()));
                    }
                }
            }
            if terms.iter().all(|term| text.contains(term)) {
                matches.push(entry);
            }
        }
        Ok((matches, total, source_apps))
    }

    fn prune(&self, days: u32, now: i64, limit: usize) -> anyhow::Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        Self::prune_connection(&transaction, days, now, limit)?;
        transaction.commit()?;
        Ok(())
    }

    fn prune_connection(
        connection: &Connection,
        days: u32,
        now: i64,
        limit: usize,
    ) -> anyhow::Result<()> {
        if days > 0 {
            connection.execute(
                "DELETE FROM rich_clipboard WHERE pinned = 0 AND created_at <= ?1",
                [now.saturating_sub(i64::from(days) * 86_400)],
            )?;
        }
        let (pinned_count, pinned_bytes) = Self::pinned_usage(connection)?;
        let available_count = limit.min(MAX_ENTRIES).saturating_sub(pinned_count);
        let available_bytes = MAX_TOTAL_BYTES.saturating_sub(pinned_bytes);
        connection.execute("DELETE FROM rich_clipboard WHERE id IN (SELECT id FROM rich_clipboard WHERE pinned = 0 ORDER BY sort_order DESC LIMIT -1 OFFSET ?1)", [available_count as i64])?;
        // Pinned blobs consume the same fixed budget but are never evicted.
        connection.execute("DELETE FROM rich_clipboard WHERE id IN (SELECT id FROM (SELECT id, SUM(length(payload)) OVER (ORDER BY sort_order DESC) AS bytes FROM rich_clipboard WHERE pinned = 0) WHERE bytes > ?1)", [available_bytes as i64])?;
        Ok(())
    }

    fn pinned_usage(connection: &Connection) -> anyhow::Result<(usize, usize)> {
        Ok(connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(length(payload)),0) FROM rich_clipboard WHERE pinned = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, u32>(0)? as usize,
                    row.get::<_, u32>(1)? as usize,
                ))
            },
        )?)
    }

    fn storage_notice(&self, limit: usize) -> anyhow::Result<String> {
        let (count, bytes): (usize, usize) = self.connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(length(payload)),0) FROM rich_clipboard",
            [],
            |row| {
                Ok((
                    row.get::<_, u32>(0)? as usize,
                    row.get::<_, u32>(1)? as usize,
                ))
            },
        )?;
        let (pins, pinned_bytes) = Self::pinned_usage(&self.connection)?;
        let limit = limit.min(MAX_ENTRIES);
        let policy = if pins >= limit || pinned_bytes >= MAX_TOTAL_BYTES {
            "Pins fill the capture limit. Unpin or delete an entry to save new content."
        } else {
            "New content replaces the oldest unpinned entries. Captures that cannot fit beside pins are skipped."
        };
        Ok(format!(
            "{count} entries · {:.2} of 16 MiB · {pins} pinned. Capture limit: {limit} entries (maximum {MAX_ENTRIES}). {policy}",
            bytes as f64 / (1024.0 * 1024.0)
        ))
    }

    fn set_pinned(
        &mut self,
        id: i64,
        pinned: bool,
        days: u32,
        now: i64,
        limit: usize,
    ) -> anyhow::Result<()> {
        ensure!(id > 0, "Invalid clipboard entry.");
        let transaction = self.connection.transaction()?;
        ensure!(
            transaction.execute(
                "UPDATE rich_clipboard SET pinned = ?1 WHERE id = ?2",
                rusqlite::params![pinned, id]
            )? == 1,
            "This clipboard entry is no longer available."
        );
        Self::prune_connection(&transaction, days, now, limit)?;
        transaction.commit()?;
        Ok(())
    }

    fn set_name(&self, id: i64, name: &str) -> anyhow::Result<()> {
        ensure!(id > 0, "Invalid clipboard entry.");
        ensure!(name.len() <= 4096, "Name is too long.");
        ensure!(
            !name.chars().any(char::is_control),
            "Use a single-line name without control characters."
        );
        let name: String = name.trim().nfc().collect();
        ensure!(
            name.chars().count() <= 120 && name.len() <= 480,
            "Name must have at most 120 characters (480 UTF-8 bytes)."
        );
        let name = (!name.is_empty()).then_some(name);
        ensure!(
            self.connection.execute(
                "UPDATE rich_clipboard SET custom_name = ?1 WHERE id = ?2 AND pinned = 1",
                rusqlite::params![name, id],
            )? == 1,
            "Pin an available entry before naming it."
        );
        Ok(())
    }

    fn clear(&self, keep_pinned: bool) -> anyhow::Result<()> {
        self.connection.execute(
            "DELETE FROM rich_clipboard WHERE ?1 = 0 OR pinned = 0",
            [keep_pinned],
        )?;
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
            let (pins, pinned_bytes) = Self::pinned_usage(&transaction)?;
            ensure!(
                pins < limit.min(MAX_ENTRIES)
                    && data.len() <= MAX_TOTAL_BYTES.saturating_sub(pinned_bytes),
                "Pinned history leaves too little space for this capture. Unpin or delete an entry."
            );
            transaction.execute("INSERT INTO rich_clipboard(kind,payload,title,created_at,source_app,sort_order) VALUES(?1,?2,?3,?4,?5,(SELECT COALESCE(MAX(sort_order),0)+1 FROM rich_clipboard))", params![kind,data,title,now,source.map(|s| &s.id)])?;
        }
        Self::prune_connection(&transaction, days, now, limit)?;
        transaction.commit()?;
        Ok(())
    }

    fn preview(&self, id: i64) -> anyhow::Result<RichPreview> {
        ensure!(id > 0, "Invalid clipboard entry.");
        let (entry, bytes): (RichEntry, Vec<u8>) = self.connection.query_row("SELECT id,kind,title,created_at,source_app,length(payload),pinned,custom_name,payload FROM rich_clipboard WHERE id = ?1", [id], |row| Ok((entry(row)?, row.get(8)?))).context("This clipboard entry is no longer available.")?;
        let (png, files) = match Payload::decode(entry.kind.as_str(), bytes)? {
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

    fn file_to_reveal(&self, id: i64, file_index: u32) -> anyhow::Result<PathBuf> {
        ensure!((file_index as usize) < MAX_FILES, "Invalid file reference.");
        let preview = self.preview(id)?;
        let files = preview
            .files
            .context("Only saved file references can be revealed.")?;
        let path = files
            .get(file_index as usize)
            .context("This file reference is no longer available.")?;
        Ok(PathBuf::from(path))
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
        kind: match row.get::<_, String>(1)?.as_str() {
            "image" => RichKind::Image,
            "files" => RichKind::Files,
            _ => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Unknown rich clipboard format",
                    )
                    .into(),
                ));
            }
        },
        title: row.get(2)?,
        created_at: row.get(3)?,
        source_app: row.get(4)?,
        bytes: row.get(5)?,
        pinned: row.get(6)?,
        custom_name: row.get(7)?,
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

pub fn clear(app: &AppHandle, keep_pinned: bool) -> anyhow::Result<()> {
    if app
        .path()
        .app_data_dir()?
        .join("clipboard-rich.sqlite3")
        .exists()
    {
        with_store(app, |store| store.clear(keep_pinned))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn rich_clipboard_history(
    query: String,
    kind: Option<RichKind>,
    source_app: Option<String>,
    app: AppHandle,
) -> Result<RichHistory, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| {
            let (entries, total, source_apps) = store.search(&query, kind, source_app.as_deref())?;
            Ok(RichHistory {
                entries,
                total,
                source_apps,
                storage_notice: store.storage_notice(app.state::<LauncherState>().settings().clipboard_limit())?,
                capture_supported: cfg!(target_os = "macos"),
                support_notice: if cfg!(target_os = "macos") {
                    "Captures native PNG images and Finder file lists on macOS. TIFF-only images and URL-only file sources are skipped. Images: up to 4 MiB and 4 megapixels. Files: up to 64 existing paths, not file contents. Pinned entries survive retention and Clear unpinned. Unpinning applies the current retention and count limits immediately.".into()
                } else { "Image and file capture, copy, and paste are currently supported only on macOS. Text history remains available.".into() },
            })
        }).map_err(|error| error.to_string())
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn rich_clipboard_preview(id: i64, app: AppHandle) -> Result<RichPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| store.preview(id)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn copy_saved(app: &AppHandle, id: i64) -> Result<(), String> {
    with_store(app, |store| {
        // Serialize live resolution/write with delete, prune, and capture.
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
}

#[tauri::command]
pub async fn copy_rich_clipboard(id: i64, app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || copy_saved(&app, id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn paste_rich_clipboard(id: i64, app: AppHandle) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Pasting saved images and files is currently supported only on macOS.".into());
    }
    let target = super::super::paste::prepare(&app)?;
    let writer_app = app.clone();
    super::super::paste::paste_with_clipboard(&app, target, move || copy_saved(&writer_app, id))
        .await
}

#[tauri::command]
pub async fn reveal_rich_clipboard_file(
    id: i64,
    file_index: u32,
    app: AppHandle,
) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Revealing saved file references is currently supported only on macOS.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        // Resolve the saved entry again; the frontend supplies no path.
        // Release clipboard storage before asking the file manager to reveal it.
        let path = with_store(&app, |store| store.file_to_reveal(id, file_index))
            .map_err(|error| error.to_string())?;
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn set_rich_clipboard_pinned(
    id: i64,
    pinned: bool,
    app: AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| {
            let settings = app.state::<LauncherState>().settings();
            store.set_pinned(
                id,
                pinned,
                settings.clipboard_retention_days,
                crate::ranking::now(),
                settings.clipboard_limit(),
            )
        })
        .map_err(|error| error.to_string())?;
        super::changed(&app);
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn set_rich_clipboard_name(id: i64, name: String, app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        with_store(&app, |store| store.set_name(id, &name)).map_err(|error| error.to_string())?;
        super::changed(&app);
        Ok(())
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
    fn rich_search_combines_metadata_filters_and_preserves_missing_references() {
        let (dir, mut store) = store();
        let report = dir.path().join("Résumé 100%_done.pdf");
        let design = dir.path().join("design.png");
        std::fs::write(&report, "private file content not searched").unwrap();
        std::fs::write(&design, "fixture").unwrap();
        let finder = SourceApp {
            id: "com.apple.Finder".into(),
            name: "Finder".into(),
        };
        store
            .capture(
                &Payload::Files(vec![report.clone()]),
                Some(&finder),
                100,
                0,
                32,
            )
            .unwrap();
        let first = store.list().unwrap()[0].id;
        store
            .capture(&Payload::Files(vec![design]), None, 101, 0, 32)
            .unwrap();
        store.set_pinned(first, true, 0, 102, 32).unwrap();
        let (entries, total, sources) = store.search("", None, None).unwrap();
        assert_eq!(total, 2);
        assert_eq!(entries[0].id, first);
        assert_eq!(sources, vec!["com.apple.Finder"]);
        std::fs::remove_file(report).unwrap();
        let (entries, _, _) = store
            .search(
                "RE\u{301}SUME\u{301} finder",
                Some(RichKind::Files),
                Some("COM.APPLE.FINDER"),
            )
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, first);
        assert!(store.preview(first).is_err());
        assert_eq!(store.search("100%_done", None, None).unwrap().0.len(), 1);
        assert!(
            store
                .search("100%_missing", None, None)
                .unwrap()
                .0
                .is_empty()
        );
        assert!(
            store
                .search("private file content", None, None)
                .unwrap()
                .0
                .is_empty()
        );
        assert!(
            store
                .search("design", None, Some("com.apple.Finder"))
                .unwrap()
                .0
                .is_empty()
        );
        assert!(
            store
                .search("", Some(RichKind::Image), None)
                .unwrap()
                .0
                .is_empty()
        );
        drop(store);
        let store = Store::open(&dir.path().join("rich.sqlite3")).unwrap();
        assert_eq!(
            store.search("100%_done", None, None).unwrap().0[0].id,
            first
        );
    }

    #[test]
    fn rich_search_never_decodes_image_blobs_and_bounds_input() {
        let (_dir, mut store) = store();
        store
            .capture(
                &Payload::Png(include_bytes!("../../icons/32x32.png").to_vec()),
                None,
                1,
                0,
                32,
            )
            .unwrap();
        store
            .connection
            .execute("UPDATE rich_clipboard SET payload = X'00'", [])
            .unwrap();
        assert_eq!(
            store
                .search("", Some(RichKind::Image), None)
                .unwrap()
                .0
                .len(),
            1
        );
        assert!(
            store
                .search("no match", Some(RichKind::Image), None)
                .unwrap()
                .0
                .is_empty()
        );
        assert!(store.search(&"a".repeat(1025), None, None).is_err());
        assert!(store.search("", None, Some(&"a".repeat(1025))).is_err());
        assert!(serde_json::from_str::<RichKind>(r#""unknown""#).is_err());
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
    fn clipboard_rich_migrates_v1_without_changing_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rich.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE rich_clipboard (
            id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, payload BLOB NOT NULL,
            title TEXT NOT NULL, created_at INTEGER NOT NULL, source_app TEXT, sort_order INTEGER NOT NULL);
            PRAGMA user_version = 1;").unwrap();
        let png = include_bytes!("../../icons/32x32.png").to_vec();
        connection
            .execute(
                "INSERT INTO rich_clipboard VALUES (7,'image',?1,'Saved image',100,'fixture',4)",
                [&png],
            )
            .unwrap();
        drop(connection);
        let mut store = Store::open(&path).unwrap();
        assert!(!store.list().unwrap()[0].pinned);
        assert_eq!(store.preview(7).unwrap().png, Some(png.clone()));
        store.set_pinned(7, true, 0, 101, 32).unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert!(store.list().unwrap()[0].pinned);
        assert_eq!(store.preview(7).unwrap().png, Some(png));
    }

    #[test]
    fn clipboard_rich_pins_survive_retention_recapture_and_clear_unpinned() {
        let (dir, mut store) = store();
        let file = dir.path().join("keep.txt");
        std::fs::write(&file, "fixture").unwrap();
        let files = Payload::Files(vec![file.clone()]);
        let image = Payload::Png(include_bytes!("../../icons/32x32.png").to_vec());
        store.capture(&files, None, 1, 0, 32).unwrap();
        let file_id = store.list().unwrap()[0].id;
        store.set_pinned(file_id, true, 0, 1, 32).unwrap();
        store.capture(&image, None, 2, 0, 32).unwrap();
        let image_id = store
            .list()
            .unwrap()
            .iter()
            .find(|entry| entry.kind == RichKind::Image)
            .unwrap()
            .id;
        store.set_pinned(image_id, true, 0, 2, 32).unwrap();
        store.prune(1, 90_000, 1).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
        store.capture(&files, None, 90_001, 1, 1).unwrap();
        assert_eq!(store.list().unwrap()[0].id, file_id);
        assert!(store.list().unwrap().iter().all(|entry| entry.pinned));
        store.clear(true).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
        std::fs::remove_file(file).unwrap();
        assert!(store.preview(file_id).is_err());
        // Missing references can still be unpinned; expired entries are removed.
        store.set_pinned(file_id, false, 1, 90_002, 32).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.list().unwrap()[0].id, image_id);
        store.clear(false).unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn clipboard_rich_pin_count_limit_rejects_new_capture_without_eviction() {
        let (dir, mut store) = store();
        for index in 0..MAX_ENTRIES {
            let file = dir.path().join(index.to_string());
            std::fs::write(&file, "fixture").unwrap();
            store
                .capture(&Payload::Files(vec![file]), None, index as i64, 0, 32)
                .unwrap();
            let id = store
                .list()
                .unwrap()
                .iter()
                .find(|entry| !entry.pinned)
                .unwrap()
                .id;
            store.set_pinned(id, true, 0, 100, 32).unwrap();
        }
        let before: Vec<_> = store.list().unwrap().iter().map(|entry| entry.id).collect();
        let image = Payload::Png(include_bytes!("../../icons/32x32.png").to_vec());
        assert!(store.capture(&image, None, 101, 0, 32).is_err());
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            before
        );
        assert!(
            store
                .storage_notice(32)
                .unwrap()
                .contains("Pins fill the capture limit")
        );
        store.set_pinned(before[0], false, 0, 102, 32).unwrap();
        store.capture(&image, None, 103, 0, 32).unwrap();
        let entries = store.list().unwrap();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(
            entries.iter().filter(|entry| entry.pinned).count(),
            MAX_ENTRIES - 1
        );
        assert!(!entries.iter().any(|entry| entry.id == before[0]));
    }

    #[test]
    fn clipboard_rich_pinned_blobs_share_the_hard_byte_budget() {
        let (_dir, mut store) = store();
        for order in 0..4 {
            store.connection.execute("INSERT INTO rich_clipboard(kind,payload,title,created_at,sort_order,pinned) VALUES('image',zeroblob(?1),'budget fixture',1,?2,1)", params![MAX_IMAGE_BYTES as i64, order]).unwrap();
        }
        let png = Payload::Png(include_bytes!("../../icons/32x32.png").to_vec());
        assert!(store.capture(&png, None, 100, 0, 32).is_err());
        assert_eq!(store.list().unwrap().len(), 4);
        store.prune(1, 90_000, 1).unwrap();
        assert_eq!(store.list().unwrap().len(), 4);
        let id = store.list().unwrap()[0].id;
        store.set_pinned(id, false, 0, 101, 32).unwrap();
        store.capture(&png, None, 102, 0, 32).unwrap();
        let entries = store.list().unwrap();
        assert_eq!(entries.iter().filter(|entry| entry.pinned).count(), 3);
        assert!(
            entries
                .iter()
                .map(|entry| entry.bytes as usize)
                .sum::<usize>()
                <= MAX_TOTAL_BYTES
        );
    }

    #[test]
    fn clipboard_rich_failed_pin_update_keeps_saved_state() {
        let (dir, mut store) = store();
        store
            .capture(
                &Payload::Png(include_bytes!("../../icons/32x32.png").to_vec()),
                None,
                1,
                0,
                32,
            )
            .unwrap();
        let id = store.list().unwrap()[0].id;
        assert!(store.set_pinned(-1, true, 0, 2, 32).is_err());
        assert!(store.set_pinned(id + 1, true, 0, 2, 32).is_err());
        let blocker = Connection::open(dir.path().join("rich.sqlite3")).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(store.set_pinned(id, true, 0, 2, 32).is_err());
        assert!(!store.list().unwrap()[0].pinned);
        blocker.execute_batch("ROLLBACK").unwrap();
        store.set_pinned(id, true, 0, 2, 32).unwrap();
        assert!(store.list().unwrap()[0].pinned);
    }

    #[test]
    fn clipboard_rich_reveal_resolves_each_live_reference_without_writes() {
        let (dir, mut store) = store();
        let first = dir.path().join("report with spaces.txt");
        let second = dir.path().join("folder");
        std::fs::write(&first, "fixture").unwrap();
        std::fs::create_dir(&second).unwrap();
        store
            .capture(
                &Payload::Files(vec![first.clone(), second.clone()]),
                None,
                1,
                0,
                32,
            )
            .unwrap();
        let id = store.list().unwrap()[0].id;
        store.set_pinned(id, true, 0, 2, 32).unwrap();
        store.set_name(id, "Release files").unwrap();
        assert_eq!(store.file_to_reveal(id, 0).unwrap(), first);
        assert_eq!(store.file_to_reveal(id, 1).unwrap(), second);
        assert!(store.file_to_reveal(id, 2).is_err());
        assert!(store.file_to_reveal(id, u32::MAX).is_err());
        assert!(store.file_to_reveal(-1, 0).is_err());
        assert!(store.file_to_reveal(id + 1, 0).is_err());
        assert_eq!(
            store.list().unwrap()[0].custom_name.as_deref(),
            Some("Release files")
        );
        assert_eq!(store.list().unwrap()[0].created_at, 1);
        assert!(store.list().unwrap()[0].pinned);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "fixture");
        std::fs::remove_file(first).unwrap();
        assert!(store.file_to_reveal(id, 0).is_err());
        // Reveal uses the same whole-list availability check as preview/copy.
        assert!(store.file_to_reveal(id, 1).is_err());
        store.delete(Some(id)).unwrap();
        assert!(store.file_to_reveal(id, 1).is_err());
        store
            .capture(
                &Payload::Png(include_bytes!("../../icons/32x32.png").to_vec()),
                None,
                3,
                0,
                32,
            )
            .unwrap();
        assert!(
            store
                .file_to_reveal(store.list().unwrap()[0].id, 0)
                .is_err()
        );
    }

    #[test]
    fn clipboard_rich_names_persist_search_and_preserve_originals() {
        let (dir, mut store) = store();
        let png = include_bytes!("../../icons/32x32.png").to_vec();
        let payload = Payload::Png(png.clone());
        store.capture(&payload, None, 1, 0, 32).unwrap();
        let original = store.list().unwrap().remove(0);
        assert!(store.set_name(original.id, "Logo").is_err());
        store.set_pinned(original.id, true, 0, 2, 32).unwrap();
        store.set_name(original.id, "  Cafe\u{301} logo  ").unwrap();
        store.capture(&payload, None, 3, 0, 32).unwrap();
        drop(store);
        let mut store = Store::open(&dir.path().join("rich.sqlite3")).unwrap();
        let renamed = store.list().unwrap().remove(0);
        assert_eq!(renamed.custom_name.as_deref(), Some("Café logo"));
        assert_eq!(renamed.title, original.title);
        assert_eq!(renamed.created_at, original.created_at);
        assert_eq!(store.preview(original.id).unwrap().png, Some(png.clone()));
        assert_eq!(store.search("CAFÉ logo", None, None).unwrap().0.len(), 1);
        assert_eq!(
            store.search(&original.title, None, None).unwrap().0.len(),
            1
        );
        store.set_pinned(original.id, false, 0, 4, 32).unwrap();
        assert_eq!(
            store.list().unwrap()[0].custom_name.as_deref(),
            Some("Café logo")
        );
        assert!(store.set_name(original.id, "Changed").is_err());
        store.set_pinned(original.id, true, 0, 5, 32).unwrap();
        store.set_name(original.id, "   ").unwrap();
        assert!(store.list().unwrap()[0].custom_name.is_none());
        assert!(store.search("Café", None, None).unwrap().0.is_empty());
        assert_eq!(store.preview(original.id).unwrap().png, Some(png));
    }

    #[test]
    fn clipboard_rich_names_validate_and_failed_writes_keep_saved_state() {
        let (dir, mut store) = store();
        let path = dir.path().join("release.txt");
        std::fs::write(&path, "fixture").unwrap();
        store
            .capture(&Payload::Files(vec![path.clone()]), None, 1, 0, 32)
            .unwrap();
        let id = store.list().unwrap()[0].id;
        store.set_pinned(id, true, 0, 2, 32).unwrap();
        store.set_name(id, "Release files").unwrap();
        for name in [
            "x".repeat(121),
            "x".repeat(4097),
            "line\nbreak".into(),
            "nul\0".into(),
            "tab\t".into(),
        ] {
            assert!(store.set_name(id, &name).is_err());
        }
        assert!(store.set_name(-1, "Invalid").is_err());
        assert!(store.set_name(id + 1, "Missing").is_err());
        let blocker = Connection::open(dir.path().join("rich.sqlite3")).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(store.set_name(id, "Lost").is_err());
        assert_eq!(
            store.list().unwrap()[0].custom_name.as_deref(),
            Some("Release files")
        );
        assert_eq!(
            store.preview(id).unwrap().files,
            Some(vec![path.to_string_lossy().into_owned()])
        );
        blocker.execute_batch("ROLLBACK").unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(store.preview(id).is_err());
        store.set_name(id, "Missing release files").unwrap();
        assert_eq!(
            store.search("missing release", None, None).unwrap().0.len(),
            1
        );
        store.set_name(id, &"😀".repeat(120)).unwrap();
    }

    #[test]
    fn clipboard_rich_migrates_v2_names_without_changing_pins_or_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rich.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE rich_clipboard (
            id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, payload BLOB NOT NULL,
            title TEXT NOT NULL, created_at INTEGER NOT NULL, source_app TEXT,
            sort_order INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0);
            PRAGMA user_version = 2;",
            )
            .unwrap();
        let png = include_bytes!("../../icons/32x32.png").to_vec();
        connection
            .execute(
                "INSERT INTO rich_clipboard VALUES(7,'image',?1,'Saved image',100,'fixture',4,1)",
                [&png],
            )
            .unwrap();
        drop(connection);
        let store = Store::open(&path).unwrap();
        let entry = store.list().unwrap().remove(0);
        assert!(entry.pinned);
        assert!(entry.custom_name.is_none());
        assert_eq!(entry.title, "Saved image");
        assert_eq!(entry.created_at, 100);
        assert_eq!(store.preview(7).unwrap().png, Some(png));
        let order: i64 = store
            .connection
            .query_row(
                "SELECT sort_order FROM rich_clipboard WHERE id=7",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(order, 4);
        let version: u32 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
    }

    #[test]
    fn clipboard_rich_newer_database_is_not_replaced() {
        let (dir, store) = store();
        store
            .connection
            .pragma_update(None, "user_version", 4)
            .unwrap();
        drop(store);
        assert!(Store::open(&dir.path().join("rich.sqlite3")).is_err());
        let connection = Connection::open(dir.path().join("rich.sqlite3")).unwrap();
        assert_eq!(
            connection
                .pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            4
        );
    }
}
