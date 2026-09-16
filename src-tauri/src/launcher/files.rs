use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::LauncherState;
use crate::{
    error::Error,
    providers::files::{self, ScanReport},
};

#[derive(Default)]
pub struct FileScan {
    running: AtomicBool,
    warning: Mutex<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub total: usize,
    pub indexing: bool,
    pub warning: Option<String>,
}

impl FileScan {
    pub fn status(&self, total: usize) -> FileStatus {
        FileStatus {
            total,
            indexing: self.running.load(Ordering::Acquire),
            warning: self.warning.lock().ok().and_then(|warning| warning.clone()),
        }
    }
}

#[tauri::command]
pub fn refresh_files(app: AppHandle) {
    scan_files(&app);
}

pub fn scan_files(app: &AppHandle) {
    if app
        .state::<LauncherState>()
        .files
        .running
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app.emit("files-changed", ());
        let worker = app.clone();
        let started = std::time::Instant::now();
        let scanned = tauri::async_runtime::spawn_blocking(move || {
            let state = worker.state::<LauncherState>();
            let settings = &state.settings;
            let mut report = ScanReport::default();
            let roots = match &settings.file_search_roots {
                Some(roots) => {
                    let home = worker.path().home_dir().ok();
                    roots
                        .iter()
                        .filter_map(|root| {
                            let expanded = files::expand_root(root, home.as_deref());
                            if expanded.is_none() {
                                report.issue(root, "Use an absolute path or ~/folder.");
                            }
                            expanded
                        })
                        .collect()
                }
                None => {
                    // Resolve Windows Known Folders and Linux XDG user directories.
                    // Missing default folders are normal on minimal installations.
                    [
                        worker.path().desktop_dir(),
                        worker.path().document_dir(),
                        worker.path().download_dir(),
                    ]
                    .into_iter()
                    .filter_map(|path| path.ok())
                    // Let the scanner report access failures.
                    .filter(|path| path.try_exists().unwrap_or(true))
                    .collect()
                }
            };
            let provider = files::scan(
                roots,
                &settings.file_search_excluded_dirs,
                settings.file_limit(),
                &mut report,
            );
            let count = provider.len();
            // Build and free indexes outside the search lock. Only the swap is locked.
            let previous = state
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable.to_string())?
                .replace_files(provider);
            drop(previous);
            tracing::info!(
                count,
                skipped = report.issue_count,
                limited = report.limited,
                elapsed_ms = started.elapsed().as_millis(),
                "File index ready"
            );
            Ok::<_, String>(report.warning())
        })
        .await;
        let warning = match scanned {
            Ok(Ok(warning)) => warning,
            Ok(Err(error)) => Some(error),
            Err(error) => Some(format!("File scan failed: {error}")),
        };
        let state = app.state::<LauncherState>();
        if let Ok(mut stored) = state.files.warning.lock() {
            *stored = warning;
        }
        state.files.running.store(false, Ordering::Release);
        let _ = app.emit("files-changed", ());
    });
}
