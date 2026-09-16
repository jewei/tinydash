pub mod actions;
pub mod clipboard;
pub mod files;
pub mod query;
pub mod result;
pub mod search;
mod storage;
pub mod window;

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{error::Error, platform, providers::apps::AppProvider, settings::Settings};
use query::SearchMode;
use result::SearchResponse;
use search::SearchManager;

pub struct LauncherState {
    pub search: Mutex<SearchManager>,
    pub scanning: AtomicBool,
    pub ready: AtomicBool,
    pub settings: Settings,
    pub warnings: Vec<String>,
    pub index_error: Mutex<Option<String>>,
    pub storage: storage::Storage,
    pub clipboard: clipboard::Monitor,
    pub files: files::FileScan,
}

impl LauncherState {
    pub fn new(settings: Settings, warnings: Vec<String>) -> Self {
        Self {
            search: Mutex::new(SearchManager::default()),
            scanning: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            settings,
            warnings,
            index_error: Mutex::new(None),
            storage: storage::Storage::default(),
            clipboard: clipboard::Monitor::default(),
            files: files::FileScan::default(),
        }
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
        window::show(&app).map_err(|error| error.to_string())?;
        files::scan_files(&app);
    }
    Ok(LauncherInfo {
        settings: state.settings.clone(),
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
        let scanned = tauri::async_runtime::spawn_blocking(|| {
            platform::discover_apps().map(AppProvider::new)
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
