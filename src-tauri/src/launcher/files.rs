use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    LauncherState,
    file_watch::{self, FileWatcher, Request},
};
use crate::{
    error::Error,
    providers::files::{self, ScanReport},
};

#[derive(Default)]
pub struct FileScan {
    running: AtomicBool,
    warning: Mutex<Option<String>>,
    sender: Mutex<Option<SyncSender<Request>>>,
    stopped: Arc<AtomicBool>,
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

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(sender) = self.sender.lock()
            && let Some(sender) = sender.as_ref()
        {
            let _ = sender.try_send(Request::Stop);
        }
    }

    fn warning(&self, warning: Option<String>) {
        if let Ok(mut stored) = self.warning.lock() {
            *stored = warning;
        }
    }
}

#[tauri::command]
pub fn refresh_files(app: AppHandle) {
    scan_files(&app);
}

pub fn scan_files(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    let Ok(mut sender) = state.files.sender.lock() else {
        return;
    };
    if sender.is_none() {
        let (tx, rx) = sync_channel(1);
        let worker = app.clone();
        let callback_sender = tx.clone();
        let stop = Arc::clone(&state.files.stopped);
        state.files.running.store(true, Ordering::Release);
        match std::thread::Builder::new()
            .name("file-index".into())
            .spawn(move || run_worker(worker, rx, callback_sender, stop))
        {
            Ok(_) => *sender = Some(tx),
            Err(error) => {
                state.files.running.store(false, Ordering::Release);
                state
                    .files
                    .warning(Some(format!("Cannot start the file scanner: {error}")));
                let _ = app.emit("files-changed", ());
            }
        }
    }
    if let Some(sender) = sender.as_ref() {
        let _ = sender.try_send(Request::Refresh);
    }
}

fn roots(app: &AppHandle) -> (Vec<PathBuf>, Option<String>) {
    let state = app.state::<LauncherState>();
    let mut report = ScanReport::default();
    let roots = match &state.settings.file_search_roots {
        Some(roots) => {
            let home = app.path().home_dir().ok();
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
        None => [
            app.path().desktop_dir(),
            app.path().document_dir(),
            app.path().download_dir(),
        ]
        .into_iter()
        .filter_map(|path| path.ok())
        .collect(),
    };
    (roots, report.warning())
}

fn run_worker(
    worker: AppHandle,
    rx: Receiver<Request>,
    callback_sender: SyncSender<Request>,
    stop: Arc<AtomicBool>,
) {
    let state = worker.state::<LauncherState>();
    let (roots, root_warning) = roots(&worker);
    let mut watch_warning = None;
    let mut watcher = if state.settings.file_watch_enabled && !roots.is_empty() {
        match FileWatcher::new(
            roots
                .iter()
                .map(|path| file_watch::resolve_root(path))
                .collect(),
            state.settings.file_search_excluded_dirs.clone(),
            callback_sender.clone(),
        ) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                watch_warning = Some(format!(
                    "Automatic file updates are unavailable: {error}. Use Refresh files."
                ));
                None
            }
        }
    } else {
        None
    };
    while let Ok(request) = rx.recv() {
        if stop.load(Ordering::Acquire) || matches!(request, Request::Stop) {
            break;
        }
        if matches!(request, Request::Changed)
            && !file_watch::settle(&rx, Duration::from_millis(300), Duration::from_secs(2))
        {
            break;
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        state.files.running.store(true, Ordering::Release);
        let _ = worker.emit("files-changed", ());
        let started = Instant::now();
        let mut report = ScanReport::default();
        let scan_roots = roots
            .iter()
            .filter(|path| {
                state.settings.file_search_roots.is_some() || path.try_exists().unwrap_or(true)
            })
            .cloned()
            .collect();
        let provider = files::scan(
            scan_roots,
            &state.settings.file_search_excluded_dirs,
            state.settings.file_limit(),
            &mut report,
        );
        let count = provider.len();
        let outcome = state
            .search
            .lock()
            .map(|mut search| search.replace_files(provider))
            .map_err(|_| Error::IndexUnavailable.to_string());
        // Drop the previous index outside the search lock.
        let mut warnings: Vec<_> = [
            root_warning.clone(),
            report.warning(),
            outcome.as_ref().err().cloned(),
        ]
        .into_iter()
        .flatten()
        .collect();
        drop(outcome);
        if let Some(watcher) = watcher.as_mut() {
            let (changed, warning) = watcher.update(&report);
            watch_warning = warning;
            // Cover changes made between traversal and watch registration.
            if changed {
                let _ = callback_sender.try_send(Request::Changed);
            }
        }
        warnings.extend(watch_warning.clone());
        state
            .files
            .warning((!warnings.is_empty()).then(|| warnings.join(" ")));
        state.files.running.store(false, Ordering::Release);
        let _ = worker.emit("files-changed", ());
        tracing::info!(
            count,
            skipped = report.issue_count,
            limited = report.limited,
            elapsed_ms = started.elapsed().as_millis(),
            "File index ready"
        );
    }
}
