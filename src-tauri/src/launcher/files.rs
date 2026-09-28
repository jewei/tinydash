use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    generation: AtomicU64,
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
    pub(super) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.warning(None);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(super) fn cancelled(&self, generation: u64) -> bool {
        self.stopped.load(Ordering::Acquire) || self.generation() != generation
    }

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

    fn warning_for(&self, generation: u64, warning: Option<String>) {
        if let Ok(mut stored) = self.warning.lock()
            && !self.cancelled(generation)
        {
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

// Aggregate counters contain no roots, filenames, queries, or error strings.
// The window and total scans expose frequency; cancelled work counts toward
// both duration and visits rather than disappearing from workload diagnostics.
struct ScanMetrics {
    since: Instant,
    scans: u64,
    cancelled: u64,
    visited: u64,
    duration: Duration,
}

impl ScanMetrics {
    fn new() -> Self {
        Self {
            since: Instant::now(),
            scans: 0,
            cancelled: 0,
            visited: 0,
            duration: Duration::ZERO,
        }
    }

    fn record(&mut self, elapsed: Duration, visited: usize, cancelled: bool) {
        self.scans += 1;
        self.cancelled += u64::from(cancelled);
        self.visited += visited as u64;
        self.duration += elapsed;
        tracing::info!(
            scans = self.scans,
            cancelled = self.cancelled,
            visited = self.visited,
            scan_ms = self.duration.as_millis(),
            window_ms = self.since.elapsed().as_millis(),
            "File scan workload"
        );
    }
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
    let mut active_generation = state.files.generation();
    let mut not_before = Instant::now();
    let mut metrics = ScanMetrics::new();
    while let Ok(request) = rx.recv() {
        if stop.load(Ordering::Acquire) || matches!(request, Request::Stop) {
            break;
        }
        let mut keep_running = || {
            if state.files.generation() != active_generation {
                // Settings/disable must remove old watches even during a long
                // cooldown, not just when the replacement scan finally starts.
                drop(watcher.take());
                active_settings = None;
                watch_warning = None;
            }
            !stop.load(Ordering::Acquire)
        };
        if !file_watch::wait_for_budget(&rx, not_before, &mut keep_running) {
            break;
        }
        if matches!(request, Request::Changed)
            && !file_watch::settle_cancellable(
                &rx,
                Duration::from_millis(300),
                Duration::from_secs(2),
                &mut keep_running,
            )
        {
            break;
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        // Read the generation first: a concurrent settings write will either
        // be included by settings() or invalidate this scan before publication.
        let generation = state.files.generation();
        let settings = state.settings();
        active_generation = generation;
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
        let provider = files::scan_cancellable(
            scan_roots,
            &settings.file_search_excluded_dirs,
            settings.file_limit(),
            &mut report,
            || state.files.cancelled(generation),
        );
        let outcome = match provider {
            Some(provider) => state.accept_file_scan(&settings, generation, provider),
            None => Ok(false),
        };
        let elapsed = started.elapsed();
        not_before = Instant::now() + file_watch::scan_rest(elapsed);
        let obsolete = matches!(outcome, Ok(false)) || state.files.cancelled(generation);
        metrics.record(elapsed, report.visited, obsolete);
        if obsolete {
            // Keep any same-settings snapshot. Removed-root invalidation is
            // synchronous in replace_settings, not delayed until this worker.
            drop(watcher.take());
            active_settings = None;
            watch_warning = None;
            state.files.running.store(false, Ordering::Release);
            if stop.load(Ordering::Acquire) {
                break;
            }
            let _ = callback_sender.try_send(Request::Refresh);
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
        state.files.warning_for(
            generation,
            (!warnings.is_empty()).then(|| warnings.join(" ")),
        );
        state.files.running.store(false, Ordering::Release);
        let _ = worker.emit("files-changed", ());
    }
    state.files.running.store(false, Ordering::Release);
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
        assert!(
            state
                .accept_file_scan(&previous, 0, scan(&documents))
                .unwrap()
        );
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
            !state
                .accept_file_scan(&previous, 0, scan(&documents))
                .unwrap(),
            "A scan for removed folders must not replace current results"
        );
        assert!(state.accept_file_scan(&next, 1, scan(&downloads)).unwrap());
        let mut search = state.search.lock().unwrap();
        let results = search.search("example", SearchMode::Files).unwrap().results;
        assert_eq!(results.len(), 1);
        assert_eq!(
            PathBuf::from(&results[0].subtitle).canonicalize().unwrap(),
            downloads.canonicalize().unwrap().join("example.txt")
        );
        drop(search);
        state.replace_settings(Settings {
            file_search_roots: Some(vec![]),
            ..next.clone()
        });
        assert!(!state.accept_file_scan(&next, 1, scan(&downloads)).unwrap());
        assert_eq!(state.search.lock().unwrap().file_count(), 0);
    }

    #[test]
    fn rapid_root_changes_cancel_a_large_traversal_and_only_newest_can_publish() {
        let directory = tempfile::tempdir().unwrap();
        let old = directory.path().join("old");
        let newest = directory.path().join("newest");
        std::fs::create_dir(&old).unwrap();
        std::fs::create_dir(&newest).unwrap();
        for index in 0..4096 {
            std::fs::write(old.join(format!("old-{index}.txt")), "").unwrap();
        }
        std::fs::write(newest.join("newest.txt"), "").unwrap();
        let previous = Settings {
            file_search_roots: Some(vec![old.clone()]),
            ..Settings::default()
        };
        let next = Settings {
            file_search_roots: Some(vec![newest.clone()]),
            ..previous.clone()
        };
        let state = LauncherState::new(previous.clone(), vec![]);
        let generation = state.files.generation();
        let completed_old = files::scan(vec![old.clone()], &[], 5000, &mut ScanReport::default());
        let (paused_tx, paused_rx) = sync_channel(1);
        let (resume_tx, resume_rx) = sync_channel(0);
        std::thread::scope(|scope| {
            let state = &state;
            let scan = scope.spawn(move || {
                let mut report = ScanReport::default();
                let mut checks = 0;
                let provider = files::scan_cancellable(vec![old], &[], 5000, &mut report, || {
                    checks += 1;
                    if checks == 128 {
                        paused_tx.send(()).unwrap();
                        resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    }
                    state.files.cancelled(generation)
                });
                (provider, report)
            });
            paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            state.replace_settings(next.clone());
            state.replace_settings(previous.clone());
            // Even identical settings cannot resurrect a completed A scan
            // after A -> B -> A. Checking settings equality alone missed this.
            assert!(
                !state
                    .accept_file_scan(&previous, generation, completed_old)
                    .unwrap()
            );
            state.replace_settings(next.clone());
            assert_eq!(state.search.lock().unwrap().file_count(), 0);
            resume_tx.send(()).unwrap();
            let (provider, report) = scan.join().unwrap();
            assert!(
                provider.is_none(),
                "partial obsolete scans must be discarded"
            );
            assert!(report.visited > 100 && report.visited < 128);
        });
        let latest = files::scan(vec![newest], &[], 5000, &mut ScanReport::default());
        assert!(
            state
                .accept_file_scan(&next, state.files.generation(), latest)
                .unwrap()
        );
        let mut search = state.search.lock().unwrap();
        let results = search.search("", SearchMode::Files).unwrap().results;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "newest.txt");
    }

    #[test]
    fn disable_and_stop_cancel_in_traversal_but_same_settings_keep_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..512 {
            std::fs::write(directory.path().join(format!("entry-{index}.txt")), "").unwrap();
        }
        let settings = Settings {
            file_search_roots: Some(vec![directory.path().into()]),
            ..Settings::default()
        };
        for disable in [false, true] {
            let state = LauncherState::new(settings.clone(), vec![]);
            let generation = state.files.generation();
            let original = files::scan(
                vec![directory.path().into()],
                &[],
                1000,
                &mut ScanReport::default(),
            );
            assert!(
                state
                    .accept_file_scan(&settings, generation, original)
                    .unwrap()
            );
            let mut checks = 0;
            let mut report = ScanReport::default();
            let provider = files::scan_cancellable(
                vec![directory.path().into()],
                &[],
                1000,
                &mut report,
                || {
                    checks += 1;
                    if checks == 32 {
                        state.replace_settings(settings.clone());
                        assert!(!state.files.cancelled(generation));
                        assert_eq!(state.search.lock().unwrap().file_count(), 512);
                        if disable {
                            state.replace_settings(Settings {
                                file_watch_enabled: false,
                                ..settings.clone()
                            });
                        } else {
                            state.files.stop();
                        }
                    }
                    state.files.cancelled(generation)
                },
            );
            assert!(provider.is_none());
            assert!(report.visited > 0 && report.visited < 32);
            assert_eq!(
                state.search.lock().unwrap().file_count(),
                if disable { 0 } else { 512 }
            );
            let completed = files::scan(vec![], &[], 1000, &mut ScanReport::default());
            assert!(
                !state
                    .accept_file_scan(&settings, generation, completed)
                    .unwrap()
            );
        }
    }

    #[test]
    fn shutdown_while_publication_waits_for_search_rejects_the_scan() {
        let state = LauncherState::new(Settings::default(), vec![]);
        let generation = state.files.generation();
        let search = state.search.lock().unwrap();
        std::thread::scope(|scope| {
            let state = &state;
            let publish = scope.spawn(move || {
                state.accept_file_scan(
                    &Settings::default(),
                    generation,
                    files::FileProvider::default(),
                )
            });
            // Publication takes the settings read lock before it waits for
            // search. Wait for that point without filesystem/timing assumptions.
            let deadline = Instant::now() + Duration::from_secs(5);
            let publication_waiting = loop {
                if state.settings.try_write().is_err() {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::yield_now();
            };
            state.files.stop();
            drop(search);
            let accepted = publish.join().unwrap().unwrap();
            assert!(publication_waiting, "publication did not start");
            assert!(!accepted);
        });
    }

    #[test]
    fn obsolete_warnings_cannot_restore_removed_root_warnings() {
        let files = FileScan::default();
        let old = files.generation();
        files.warning_for(old, Some("old warning".into()));
        files.invalidate();
        files.warning_for(old, Some("late old warning".into()));
        assert!(files.status(0).warning.is_none());
        files.warning_for(files.generation(), Some("current warning".into()));
        assert_eq!(files.status(0).warning.as_deref(), Some("current warning"));
    }

    #[test]
    fn workload_metrics_include_cancelled_visits_and_duration() {
        let mut metrics = ScanMetrics::new();
        metrics.record(Duration::from_millis(20), 120, true);
        metrics.record(Duration::from_millis(80), 500, false);
        assert_eq!(metrics.scans, 2);
        assert_eq!(metrics.cancelled, 1);
        assert_eq!(metrics.visited, 620);
        assert_eq!(metrics.duration, Duration::from_millis(100));
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
