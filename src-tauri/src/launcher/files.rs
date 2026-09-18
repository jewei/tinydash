use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
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
    providers::files::{self, ScanReport},
    settings::Settings,
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

    fn request(
        &self,
        spawn: impl FnOnce(Receiver<Request>, SyncSender<Request>) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let mut sender = self
            .sender
            .lock()
            .map_err(|_| std::io::Error::other("File scanner is unavailable"))?;
        if self.stopped.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(active) = sender.as_ref() {
            match active.try_send(Request::Refresh) {
                Ok(()) | Err(TrySendError::Full(_)) => return Ok(()),
                // An exited worker drops its receiver. Discard the dead sender
                // so settings changes and manual refresh can start a new worker.
                Err(TrySendError::Disconnected(_)) => *sender = None,
            }
        }
        if sender.is_none() {
            let (tx, rx) = sync_channel(1);
            self.running.store(true, Ordering::Release);
            if let Err(error) = spawn(rx, tx.clone()) {
                self.running.store(false, Ordering::Release);
                return Err(error);
            }
            *sender = Some(tx);
        }
        if let Some(sender) = sender.as_ref() {
            let _ = sender.try_send(Request::Refresh);
        }
        Ok(())
    }
}

#[tauri::command]
pub fn refresh_files(app: AppHandle) {
    scan_files(&app);
}

pub fn scan_files(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    if let Err(error) = state.files.request(|rx, callback_sender| {
        let worker = app.clone();
        let stop = Arc::clone(&state.files.stopped);
        std::thread::Builder::new()
            .name("file-index".into())
            .spawn(move || run_worker(worker, rx, callback_sender, stop))
            .map(|_| ())
    }) {
        state
            .files
            .warning(Some(format!("Cannot start the file scanner: {error}")));
        let _ = app.emit("files-changed", ());
    }
}

fn roots(app: &AppHandle, settings: &Settings) -> (Vec<PathBuf>, Option<String>) {
    let mut report = ScanReport::default();
    let roots = match &settings.file_search_roots {
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
    let mut watch_warning = None;
    let mut watcher: Option<FileWatcher> = None;
    let mut active_settings: Option<Settings> = None;
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
        let settings = state.settings();
        let (roots, root_warning) = roots(&worker, &settings);
        if active_settings
            .as_ref()
            .is_none_or(|active| !active.same_file_settings(&settings))
        {
            // Drop watches for removed roots before installing the new configuration.
            drop(watcher.take());
            watch_warning = None;
            if settings.file_watch_enabled && !roots.is_empty() {
                match FileWatcher::new(
                    roots
                        .iter()
                        .map(|path| file_watch::resolve_root(path))
                        .collect(),
                    settings.file_search_excluded_dirs.clone(),
                    callback_sender.clone(),
                ) {
                    Ok(value) => watcher = Some(value),
                    Err(error) => {
                        watch_warning = Some(format!(
                            "Automatic file updates are unavailable: {error}. Use Refresh files."
                        ))
                    }
                }
            }
            active_settings = Some(settings.clone());
        }
        state.files.running.store(true, Ordering::Release);
        let _ = worker.emit("files-changed", ());
        let started = Instant::now();
        let mut report = ScanReport::default();
        let scan_roots = roots
            .iter()
            .filter(|path| {
                settings.file_search_roots.is_some() || path.try_exists().unwrap_or(true)
            })
            .cloned()
            .collect();
        let provider = files::scan(
            scan_roots,
            &settings.file_search_excluded_dirs,
            settings.file_limit(),
            &mut report,
        );
        let count = provider.len();
        let outcome = state.accept_file_scan(&settings, provider);
        if matches!(outcome, Ok(false)) {
            // The queued refresh will scan the current folders. Do not publish
            // stale warnings or rearm watches from this obsolete scan.
            continue;
        }
        let mut warnings: Vec<_> = [
            root_warning.clone(),
            report.warning(),
            outcome.as_ref().err().cloned(),
        ]
        .into_iter()
        .flatten()
        .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::query::SearchMode;

    #[test]
    fn changing_roots_removes_old_results_and_rejects_an_unfinished_old_scan() {
        let directory = tempfile::tempdir().unwrap();
        let documents = directory.path().join("Documents");
        let downloads = directory.path().join("Downloads");
        for root in [&documents, &downloads] {
            std::fs::create_dir(root).unwrap();
            std::fs::write(root.join("example.txt"), "").unwrap();
        }
        let previous = Settings {
            file_search_roots: Some(vec![documents.clone()]),
            ..Settings::default()
        };
        let next = Settings {
            file_search_roots: Some(vec![downloads.clone()]),
            ..previous.clone()
        };
        let scan =
            |root: &PathBuf| files::scan(vec![root.clone()], &[], 100, &mut ScanReport::default());
        let state = LauncherState::new(previous.clone(), vec![]);
        assert!(state.accept_file_scan(&previous, scan(&documents)).unwrap());
        state.replace_settings(next.clone());
        assert!(
            state
                .search
                .lock()
                .unwrap()
                .search("example", SearchMode::Files)
                .unwrap()
                .results
                .is_empty(),
            "Documents must disappear when the saved folder changes"
        );
        assert!(
            !state.accept_file_scan(&previous, scan(&documents)).unwrap(),
            "A scan for removed folders must not replace current results"
        );
        assert!(state.accept_file_scan(&next, scan(&downloads)).unwrap());
        let mut search = state.search.lock().unwrap();
        let results = search.search("example", SearchMode::Files).unwrap().results;
        assert_eq!(results.len(), 1);
        assert_eq!(
            PathBuf::from(&results[0].subtitle),
            downloads.canonicalize().unwrap().join("example.txt")
        );
        drop(search);
        state.replace_settings(Settings {
            file_search_roots: Some(vec![]),
            ..next.clone()
        });
        assert!(!state.accept_file_scan(&next, scan(&downloads)).unwrap());
        assert_eq!(state.search.lock().unwrap().file_count(), 0);
    }

    #[test]
    fn refresh_restarts_a_worker_that_stopped() {
        let files = FileScan::default();
        let mut receiver = None;
        files
            .request(|rx, _| {
                receiver = Some(rx);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            receiver.as_ref().unwrap().try_recv(),
            Ok(Request::Refresh)
        ));
        drop(receiver.take());
        files
            .request(|rx, _| {
                receiver = Some(rx);
                Ok(())
            })
            .unwrap();
        assert!(
            receiver.is_some(),
            "A disconnected scanner must be replaced"
        );
        assert!(matches!(receiver.unwrap().try_recv(), Ok(Request::Refresh)));
    }

    #[test]
    fn queued_refresh_keeps_one_worker_and_shutdown_prevents_restart() {
        let files = FileScan::default();
        let mut receiver = None;
        files
            .request(|rx, _| {
                receiver = Some(rx);
                Ok(())
            })
            .unwrap();
        files
            .request(|_, _| panic!("A queued refresh must reuse the worker"))
            .unwrap();
        assert!(matches!(
            receiver.as_ref().unwrap().try_recv(),
            Ok(Request::Refresh)
        ));
        assert!(receiver.as_ref().unwrap().try_recv().is_err());
        files
            .request(|_, _| panic!("An idle worker must receive the refresh"))
            .unwrap();
        assert!(matches!(
            receiver.as_ref().unwrap().try_recv(),
            Ok(Request::Refresh)
        ));
        files.stop();
        drop(receiver);
        files
            .request(|_, _| panic!("Shutdown must not start a worker"))
            .unwrap();
    }
}
