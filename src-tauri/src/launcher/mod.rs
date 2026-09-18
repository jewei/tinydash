pub mod actions;
pub mod clipboard;
pub mod currency;
mod file_watch;
pub mod files;
pub mod pins;
pub mod preferences;
pub mod query;
pub mod result;
pub mod search;
mod storage;
pub mod window;

use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    error::Error,
    platform,
    providers::{apps::AppProvider, files::FileProvider},
    settings::Settings,
};
use query::SearchMode;
use result::SearchResponse;
use search::SearchManager;

pub struct LauncherState {
    pub search: Mutex<SearchManager>,
    pub scanning: AtomicBool,
    pub ready: AtomicBool,
    #[cfg(target_os = "macos")]
    pub focus: platform::LauncherFocus,
    settings: RwLock<Settings>,
    pub settings_update: Mutex<()>,
    pub shortcut_recording: AtomicBool,
    pub warnings: Vec<String>,
    pub index_error: Mutex<Option<String>>,
    pub storage: storage::Storage,
    pub clipboard: clipboard::Monitor,
    pub files: files::FileScan,
    pub currency: currency::Currency,
}

impl LauncherState {
    pub fn new(settings: Settings, warnings: Vec<String>) -> Self {
        Self {
            search: Mutex::new(SearchManager::default()),
            scanning: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            #[cfg(target_os = "macos")]
            focus: platform::LauncherFocus::default(),
            settings: RwLock::new(settings),
            settings_update: Mutex::new(()),
            shortcut_recording: AtomicBool::new(false),
            warnings,
            index_error: Mutex::new(None),
            storage: storage::Storage::default(),
            clipboard: clipboard::Monitor::default(),
            files: files::FileScan::default(),
            currency: currency::Currency::default(),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn replace_settings(&self, settings: Settings) {
        // Use the same lock order as scan publication. A scan for the old roots
        // must not restore removed files after the new settings are accepted.
        let mut current = self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let previous_files = if !current.same_file_settings(&settings) {
            self.search
                .lock()
                .ok()
                .map(|mut search| search.replace_files(FileProvider::default()))
        } else {
            None
        };
        *current = settings;
        drop(current);
        drop(previous_files);
    }

    pub fn accept_file_scan(
        &self,
        settings: &Settings,
        files: FileProvider,
    ) -> Result<bool, String> {
        let current = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        if !current.same_file_settings(settings) {
            return Ok(false);
        }
        let previous = self
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .replace_files(files);
        drop(current);
        drop(previous);
        Ok(true)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherInfo {
    settings: Settings,
    platform: &'static str,
    warnings: Vec<String>,
}

#[tauri::command]
pub async fn launcher_ready(app: AppHandle) -> Result<LauncherInfo, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<LauncherState>();
        state.storage.initialize(&worker_app, &state.search);
    })
    .await
    .map_err(|error| error.to_string())?;
    let state = app.state::<LauncherState>();
    if !state.ready.swap(true, Ordering::AcqRel) {
        clipboard::start(&app);
        if std::env::args().any(|arg| arg == "--settings") {
            preferences::open_settings(app.clone()).await?;
        } else {
            window::show(&app).map_err(|error| error.to_string())?;
        }
        files::scan_files(&app);
    }
    Ok(LauncherInfo {
        settings: state.settings(),
        platform: std::env::consts::OS,
        warnings: state.warnings.clone(),
    })
}

#[tauri::command]
pub async fn search(
    query: String,
    mode: SearchMode,
    app: AppHandle,
) -> Result<SearchResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<LauncherState>();
        let mut search = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?;
        let outcome = search
            .search(&query, mode)
            .map_err(|error| error.to_string())?;
        Ok(SearchResponse {
            results: outcome.results,
            notice: outcome.notice,
            storage_error: state
                .storage
                .warning()
                .or_else(|| state.clipboard.warning()),
            total: search.app_count(),
            files: state.files.status(search.file_count()),
            currency: state.currency.status(search.rates()),
            indexing: state.scanning.load(Ordering::Acquire),
            index_error: state
                .index_error
                .lock()
                .ok()
                .and_then(|error| error.clone()),
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn refresh_apps(app: AppHandle) {
    scan_apps(&app);
}

#[tauri::command]
pub async fn set_pinned(
    app: AppHandle,
    id: String,
    category: SearchMode,
    pinned: bool,
) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker_app
            .state::<LauncherState>()
            .storage
            .set_pinned(&worker_app, &id, category, pinned)
    })
    .await
    .map_err(|error| error.to_string())??;
    if let Err(error) = app.emit("pins-changed", ()) {
        tracing::debug!(%error, "No pin listener");
    }
    Ok(())
}

pub fn scan_apps(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    if state.scanning.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = app.emit("apps-changed", ()) {
            tracing::debug!(%error, "No index listener");
        }
        let started = std::time::Instant::now();
        let worker_app = app.clone();
        let scanned = tauri::async_runtime::spawn_blocking(move || {
            let entries = platform::discover_apps()?;
            // Native icon resolution can take longer than app discovery. Publish
            // searchable names first and never hold the search lock while loading images.
            #[cfg(target_os = "macos")]
            {
                let state = worker_app.state::<LauncherState>();
                state
                    .search
                    .lock()
                    .map_err(|_| Error::IndexUnavailable)?
                    .replace_apps(AppProvider::new(entries.clone()));
                if let Err(error) = worker_app.emit("apps-changed", ()) {
                    tracing::debug!(%error, "No index listener");
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = worker_app;
            Ok::<_, Error>(AppProvider::new(platform::load_app_icons(entries)))
        })
        .await;
        let state = app.state::<LauncherState>();
        let outcome = match scanned {
            Ok(Ok(provider)) => state
                .search
                .lock()
                .map(|mut manager| {
                    let count = provider.len();
                    manager.replace_apps(provider);
                    tracing::info!(
                        count,
                        elapsed_ms = started.elapsed().as_millis(),
                        "Application index ready"
                    );
                })
                .map_err(|_| Error::IndexUnavailable.to_string()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        if let Err(error) = &outcome {
            tracing::warn!(%error, "Application scan failed");
        }
        if let Ok(mut error) = state.index_error.lock() {
            *error = outcome.err();
        }
        state.scanning.store(false, Ordering::Release);
        if let Err(error) = app.emit("apps-changed", ()) {
            tracing::debug!(%error, "No index listener");
        }
    });
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
