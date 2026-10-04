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
    generation: AtomicU64,
    lifecycle: Mutex<Lifecycle>,
    sender: Mutex<Option<SyncSender<Request>>>,
    stopped: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum FilePhase {
    Disabled,
    #[default]
    Idle,
    Queued,
    Scanning,
    Failed,
}

#[derive(Default)]
struct Lifecycle {
    phase: FilePhase,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FileStatus {
    pub total: usize,
    pub phase: FilePhase,
    pub warning: Option<String>,
}

impl FileScan {
    pub(super) fn new(settings: &Settings) -> Self {
        let scan = Self::default();
        scan.lifecycle.lock().unwrap().phase = if Self::enabled(settings) {
            FilePhase::Queued
        } else {
            FilePhase::Disabled
        };
        scan
    }

    fn enabled(settings: &Settings) -> bool {
        settings
            .file_search_roots
            .as_ref()
            .is_none_or(|roots| !roots.is_empty())
    }

    pub(super) fn invalidate(&self, settings: &Settings) {
        // Generation and status publication share this lock. A late old cycle
        // cannot restore its warning or phase after settings invalidation.
        let mut lifecycle = self.lifecycle.lock().unwrap();
        self.generation.fetch_add(1, Ordering::AcqRel);
        lifecycle.warning = None;
        lifecycle.phase = if Self::enabled(settings) {
            FilePhase::Queued
        } else {
            FilePhase::Disabled
        };
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(super) fn cancelled(&self, generation: u64) -> bool {
        self.stopped.load(Ordering::Acquire) || self.generation() != generation
    }

    pub fn status(&self, total: usize) -> FileStatus {
        let lifecycle = self.lifecycle.lock().unwrap();
        FileStatus {
            total,
            phase: lifecycle.phase,
            warning: lifecycle.warning.clone(),
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

    fn warning_for(&self, generation: u64, warning: Option<String>) {
        let mut lifecycle = self.lifecycle.lock().unwrap();
        if !self.cancelled(generation) {
            lifecycle.warning = warning;
        }
    }

    fn queued(&self) -> bool {
        let mut lifecycle = self.lifecycle.lock().unwrap();
        if matches!(lifecycle.phase, FilePhase::Idle | FilePhase::Failed) {
            lifecycle.phase = FilePhase::Queued;
            true
        } else {
            false
        }
    }

    fn begin(&self, generation: u64, rx: &Receiver<Request>) -> bool {
        let mut lifecycle = self.lifecycle.lock().unwrap();
        if self.cancelled(generation) || lifecycle.phase == FilePhase::Disabled {
            return false;
        }
        // Any signal already waiting is covered by the scan about to start.
        // Manual enqueue uses this lock, so later refreshes remain pending.
        if matches!(rx.try_recv(), Ok(Request::Stop)) {
            self.stopped.store(true, Ordering::Release);
            return false;
        }
        lifecycle.phase = FilePhase::Scanning;
        true
    }

    fn finish_stopped(&self) -> bool {
        let mut lifecycle = self.lifecycle.lock().unwrap();
        if matches!(lifecycle.phase, FilePhase::Queued | FilePhase::Scanning) {
            lifecycle.phase = FilePhase::Idle;
            true
        } else {
            false
        }
    }

    fn finish(&self, generation: u64, rx: &Receiver<Request>, retry: bool) -> Option<Request> {
        let mut lifecycle = self.lifecycle.lock().unwrap();
        // Own the pending request across cooldown even though the bounded
        // channel is now empty. Further signals coalesce into this same cycle.
        let pending = rx.try_recv().ok().or(retry.then_some(Request::Refresh));
        if self.generation() == generation && lifecycle.phase != FilePhase::Disabled {
            lifecycle.phase = if pending.is_some() && !self.stopped.load(Ordering::Acquire) {
                FilePhase::Queued
            } else {
                FilePhase::Idle
            };
        }
        pending
    }

    fn request(
        &self,
        spawn: impl FnOnce(Receiver<Request>, SyncSender<Request>) -> std::io::Result<()>,
    ) -> std::io::Result<bool> {
        let mut sender = self
            .sender
            .lock()
            .map_err(|_| std::io::Error::other("File scanner is unavailable"))?;
        if self.stopped.load(Ordering::Acquire) {
            return Ok(false);
        }
        // Serialize manual enqueue with completion's pending-request check.
        let mut lifecycle = self.lifecycle.lock().unwrap();
        let generation = self.generation();
        let changed = matches!(lifecycle.phase, FilePhase::Idle | FilePhase::Failed);
        if changed {
            lifecycle.phase = FilePhase::Queued;
        }
        if let Some(active) = sender.as_ref() {
            match active.try_send(Request::Refresh) {
                Ok(()) | Err(TrySendError::Full(_)) => return Ok(changed),
                // An exited worker drops its receiver. Discard the dead sender
                // so settings changes and manual refresh can start a new worker.
                Err(TrySendError::Disconnected(_)) => *sender = None,
            }
        }
        if sender.is_none() {
            let (tx, rx) = sync_channel(1);
            if let Err(error) = spawn(rx, tx.clone()) {
                if self.generation() == generation && lifecycle.phase != FilePhase::Disabled {
                    lifecycle.phase = FilePhase::Failed;
                    lifecycle.warning = Some(format!("Cannot start the file scanner: {error}"));
                }
                return Err(error);
            }
            *sender = Some(tx);
        }
        if let Some(sender) = sender.as_ref() {
            let _ = sender.try_send(Request::Refresh);
        }
        Ok(changed)
    }
}

#[tauri::command]
pub fn refresh_files(app: AppHandle) {
    scan_files(&app);
}

pub fn scan_files(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    let transition = state.files.request(|rx, callback_sender| {
        let worker = app.clone();
        std::thread::Builder::new()
            .name("file-index".into())
            .spawn(move || run_worker(worker, rx, callback_sender))
            .map(|_| ())
    });
    if !matches!(transition, Ok(false)) {
        let _ = app.emit("files-changed", ());
    }
}

fn roots(
    app: &AppHandle,
    settings: &Settings,
    cancelled: &dyn Fn() -> bool,
) -> Option<(Vec<PathBuf>, Option<String>)> {
    let mut report = ScanReport::default();
    let mut resolved = Vec::new();
    if cancelled() {
        return None;
    }
    match &settings.file_search_roots {
        Some(roots) => {
            let home = app.path().home_dir().ok();
            for root in roots {
                if cancelled() {
                    return None;
                }
                if let Some(expanded) = files::expand_root(root, home.as_deref()) {
                    resolved.push(expanded);
                } else {
                    report.issue(root, "Use an absolute path or ~/folder.");
                }
            }
        }
        None => {
            // Do not eagerly resolve all OS folders: one lookup can block.
            for folder in 0..3 {
                if cancelled() {
                    return None;
                }
                let path = match folder {
                    0 => app.path().desktop_dir(),
                    1 => app.path().document_dir(),
                    _ => app.path().download_dir(),
                };
                if let Ok(path) = path {
                    resolved.push(path);
                }
            }
        }
    }
    (!cancelled()).then(|| (resolved, report.warning()))
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

fn extend_rest_for_work(deadline: &mut Instant, started: Instant, finished: Instant) {
    // Rest already owed at the start of disposal is not rest actually taken
    // while disposal runs, even if the old deadline passes in the meantime.
    let remaining = deadline.saturating_duration_since(started);
    *deadline = finished + remaining + file_watch::scan_rest(finished.duration_since(started));
}

fn run_worker(worker: AppHandle, rx: Receiver<Request>, callback_sender: SyncSender<Request>) {
    let state = worker.state::<LauncherState>();
    run_worker_loop(
        &file_watch::RealClock,
        &state,
        rx,
        callback_sender,
        |settings, cancelled| roots(&worker, settings, cancelled),
        || {
            let _ = worker.emit("files-changed", ());
        },
        |watcher, report, cancelled| watcher.update(report, cancelled),
    );
}

// Keep the real worker loop independent of Tauri so phase boundaries, status
// events and workload accounting can be tested with controlled slow OS calls.
fn run_worker_loop(
    clock: &impl file_watch::WorkerClock,
    state: &LauncherState,
    rx: Receiver<Request>,
    callback_sender: SyncSender<Request>,
    mut resolve_roots: impl FnMut(
        &Settings,
        &dyn Fn() -> bool,
    ) -> Option<(Vec<PathBuf>, Option<String>)>,
    mut notify: impl FnMut(),
    mut register: impl FnMut(
        &mut FileWatcher,
        &ScanReport,
        &dyn Fn() -> bool,
    ) -> Option<(bool, Option<String>)>,
) -> ScanMetrics {
    let stop = &state.files.stopped;
    let mut watch_warning = None;
    let mut watcher: Option<FileWatcher> = None;
    let mut active_settings: Option<Settings> = None;
    let mut active_generation = state.files.generation();
    let mut not_before = clock.now();
    let mut metrics = ScanMetrics::new();
    let mut pending = None;
    while let Some(request) = pending.take().or_else(|| rx.recv().ok()) {
        if stop.load(Ordering::Acquire) || matches!(request, Request::Stop) {
            break;
        }
        if state.files.queued() {
            notify();
        }
        let mut keep_running = |deadline: &mut Instant| {
            if state.files.generation() != active_generation {
                // Settings/disable must remove old watches even during a long
                // cooldown, not just when the replacement scan finally starts.
                if let Some(obsolete) = watcher.take() {
                    let started = clock.now();
                    drop(obsolete);
                    let finished = clock.now();
                    let elapsed = finished.duration_since(started);
                    metrics.duration += elapsed;
                    // Disposal is work too; it cannot consume the remaining
                    // rest or leave its own work without a matching rest.
                    extend_rest_for_work(deadline, started, finished);
                }
                active_settings = None;
                watch_warning = None;
            }
            !stop.load(Ordering::Acquire)
        };
        if !file_watch::wait_for_budget(clock, &rx, &mut not_before, &mut keep_running) {
            break;
        }
        if matches!(request, Request::Changed)
            && !file_watch::settle_cancellable(
                clock,
                &rx,
                Duration::from_millis(300),
                Duration::from_secs(2),
                || keep_running(&mut not_before),
            )
        {
            break;
        }
        // A settings change during settling may have disposed watches and
        // extended the deadline. Do not start new work on that cleanup's rest.
        if !file_watch::wait_for_budget(clock, &rx, &mut not_before, &mut keep_running) {
            break;
        }
        // Read the generation first: a concurrent settings write will either
        // be included by settings() or invalidate this scan before publication.
        let generation = state.files.generation();
        let settings = state.settings();
        if !state.files.begin(generation, &rx) {
            // Invalidation can land after the final budget callback. Reconcile
            // old watch ownership (and its rest) before rejecting this cycle;
            // a disabled worker may receive no further requests.
            if !keep_running(&mut not_before) {
                break;
            }
            continue;
        }
        active_generation = generation;
        notify();
        // Account for the entire cycle, not only traversal: root lookup,
        // watcher creation/registration and obsolete-watcher disposal can block.
        let started = clock.now();
        let mut report = ScanReport::default();
        let cancelled = || state.files.cancelled(generation);
        let completed = (|| {
            let (roots, root_warning) = resolve_roots(&settings, &cancelled)?;
            if cancelled() {
                return None;
            }
            if active_settings
                .as_ref()
                .is_none_or(|active| !active.same_file_settings(&settings))
            {
                // Drop watches for removed roots before installing the new configuration.
                drop(watcher.take());
                watch_warning = None;
                if settings.file_watch_enabled && !roots.is_empty() {
                    let resolved = roots
                        .iter()
                        .map(|path| file_watch::resolve_root(path, &cancelled))
                        .collect::<Option<Vec<_>>>()?;
                    if cancelled() {
                        return None;
                    }
                    match FileWatcher::new(
                        resolved,
                        settings.file_search_excluded_dirs.clone(),
                        settings.file_search_include_hidden,
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
            let mut scan_roots = Vec::new();
            for path in roots {
                if cancelled() {
                    return None;
                }
                if settings.file_search_roots.is_some() || path.try_exists().unwrap_or(true) {
                    scan_roots.push(path);
                }
            }
            if cancelled() {
                return None;
            }
            let provider = files::scan_with_rules_cancellable(
                scan_roots,
                &settings.file_search_excluded_dirs,
                settings.file_limit(),
                settings.file_search_include_hidden,
                &settings.file_search_ignore_patterns,
                &mut report,
                || state.files.cancelled(generation),
            );
            let outcome = match provider {
                Some(provider) => state.accept_file_scan(&settings, generation, provider),
                None => Ok(false),
            };
            if matches!(outcome, Ok(false)) || cancelled() {
                return None;
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
                let (changed, warning) = register(watcher, &report, &cancelled)?;
                watch_warning = warning;
                // Cover changes made between traversal and watch registration.
                if changed {
                    let _ = callback_sender.try_send(Request::Changed);
                }
            }
            if cancelled() {
                return None;
            }
            warnings.extend(watch_warning.clone());
            state.files.warning_for(
                generation,
                (!warnings.is_empty()).then(|| warnings.join(" ")),
            );
            Some(())
        })();
        let obsolete = completed.is_none() || cancelled();
        if obsolete {
            // Discard partially registered watches before accounting/rest.
            drop(watcher.take());
            active_settings = None;
            watch_warning = None;
        }
        let finished = clock.now();
        let elapsed = finished.duration_since(started);
        not_before = finished + file_watch::scan_rest(elapsed);
        metrics.record(elapsed, report.visited, obsolete);
        pending = state.files.finish(generation, &rx, obsolete);
        notify(); // Includes queued replacement work, never a false idle gap.
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
    if state.files.finish_stopped() {
        notify();
    }
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::query::SearchMode;
    use file_watch::WorkerClock;
    use std::{
        cell::{Cell, RefCell},
        sync::mpsc::RecvTimeoutError,
    };

    type ClockReadHook<'a> = Box<dyn FnMut(&Cell<Instant>) + 'a>;

    struct TestClock<'a> {
        now: Cell<Instant>,
        reading: RefCell<ClockReadHook<'a>>,
        waiting: RefCell<Box<dyn FnMut() + 'a>>,
    }

    impl TestClock<'_> {
        fn new() -> Self {
            Self {
                now: Cell::new(Instant::now()),
                reading: RefCell::new(Box::new(|_| {})),
                waiting: RefCell::new(Box::new(|| {})),
            }
        }

        fn advance(&self, elapsed: Duration) {
            self.now.set(self.now.get() + elapsed);
        }
    }

    impl WorkerClock for TestClock<'_> {
        fn now(&self) -> Instant {
            (self.reading.borrow_mut())(&self.now);
            self.now.get()
        }

        fn recv_timeout(
            &self,
            rx: &Receiver<Request>,
            timeout: Duration,
        ) -> Result<Request, RecvTimeoutError> {
            (self.waiting.borrow_mut())();
            self.advance(timeout);
            rx.try_recv().map_err(|error| match error {
                std::sync::mpsc::TryRecvError::Empty => RecvTimeoutError::Timeout,
                std::sync::mpsc::TryRecvError::Disconnected => RecvTimeoutError::Disconnected,
            })
        }
    }

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
            assert!(report.visited > 40 && report.visited < 128);
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
    fn shutdown_while_publication_waits_for_settings_rejects_the_scan() {
        let state = LauncherState::new(Settings::default(), vec![]);
        let generation = state.files.generation();
        let settings = state.settings.write().unwrap();
        std::thread::scope(|scope| {
            let state = &state;
            let publish = scope.spawn(move || {
                state.accept_file_scan(
                    &Settings::default(),
                    generation,
                    files::FileProvider::default(),
                )
            });
            // Publication now takes search before settings. Hold the second
            // lock and observe the first lock acquired: this establishes the
            // publication wait without assuming thread scheduling or FS speed.
            let deadline = Instant::now() + Duration::from_secs(5);
            let publication_waiting = loop {
                if state.search.try_lock().is_err() {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::yield_now();
            };
            state.files.stop();
            drop(settings);
            let accepted = publish.join().unwrap().unwrap();
            assert!(publication_waiting, "publication did not start");
            assert!(!accepted);
        });
    }

    #[test]
    fn disposal_preserves_owed_rest_before_and_after_the_old_deadline() {
        let base = Instant::now();
        // A one-second scan initially owes three seconds of rest, through t=4.
        // Disposing from t=1..2 must retain all three and add three more.
        let mut deadline = base + Duration::from_secs(4);
        extend_rest_for_work(
            &mut deadline,
            base + Duration::from_secs(1),
            base + Duration::from_secs(2),
        );
        assert_eq!(deadline, base + Duration::from_secs(8));
        // A five-second disposal crosses the original deadline. None of those
        // five seconds count as rest: retain three and add fifteen (t=24).
        deadline = base + Duration::from_secs(4);
        extend_rest_for_work(
            &mut deadline,
            base + Duration::from_secs(1),
            base + Duration::from_secs(6),
        );
        assert_eq!(deadline, base + Duration::from_secs(24));
    }

    #[test]
    fn obsolete_warnings_cannot_restore_removed_root_warnings() {
        let files = FileScan::default();
        let old = files.generation();
        files.warning_for(old, Some("old warning".into()));
        files.invalidate(&Settings::default());
        files.warning_for(old, Some("late old warning".into()));
        assert!(files.status(0).warning.is_none());
        files.warning_for(files.generation(), Some("current warning".into()));
        assert_eq!(files.status(0).warning.as_deref(), Some("current warning"));
    }

    #[test]
    fn worker_accounts_preparation_and_registration_before_rest_and_notifies_cancellation() {
        // The production loop uses a controlled clock at OS phase boundaries.
        // No sleep or assertion depends on real filesystem speed.
        for phase in [
            "cancelled preparation",
            "registration",
            "cancelled registration",
        ] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(directory.path().join("example.txt"), "").unwrap();
            let settings = Settings {
                file_search_roots: Some(vec![directory.path().into()]),
                ..Settings::default()
            };
            let state = LauncherState::new(settings, vec![]);
            let (sender, receiver) = sync_channel(1);
            *state.files.sender.lock().unwrap() = Some(sender.clone());
            sender.try_send(Request::Refresh).unwrap();
            let mut events = Vec::new();
            let delay = Duration::from_millis(400);
            let clock = TestClock::new();
            let started = clock.now();
            *clock.waiting.borrow_mut() = Box::new(|| {
                assert_eq!(state.files.status(0).phase, FilePhase::Queued);
                // Flood both request kinds throughout rest; none may bypass it
                // or cause an idle status after wait_for_budget consumes them.
                assert!(!state.files.request(|_, _| panic!("second worker")).unwrap());
                let _ = sender.try_send(Request::Changed);
            });
            let metrics = run_worker_loop(
                &clock,
                &state,
                receiver,
                sender.clone(),
                |settings, cancelled| {
                    if cancelled() {
                        return None;
                    }
                    if phase == "cancelled preparation" {
                        clock.advance(delay);
                        state.files.invalidate(&state.settings());
                        return None;
                    }
                    Some((settings.file_search_roots.clone().unwrap(), None))
                },
                || {
                    events.push((state.files.status(0).phase, clock.now()));
                    if events.len() == 3 {
                        state.files.stop();
                    }
                },
                |watcher, report, cancelled| {
                    assert_ne!(phase, "cancelled preparation");
                    clock.advance(delay);
                    if phase == "cancelled registration" {
                        state.files.invalidate(&state.settings());
                    }
                    // Manual refresh coalesces with any new watch follow-up.
                    // Keep real watcher registration in this worker regression;
                    // only its elapsed time is controlled by the test clock.
                    let _ = sender.try_send(Request::Refresh);
                    watcher.update(report, cancelled)
                },
            );
            assert_eq!(
                events.iter().map(|event| event.0).collect::<Vec<_>>(),
                [
                    FilePhase::Scanning,
                    FilePhase::Queued,
                    FilePhase::Scanning,
                    FilePhase::Idle
                ],
                "{phase}"
            );
            assert_eq!(metrics.duration, delay, "Unaccounted {phase}");
            assert_eq!(events[0].1, started);
            assert_eq!(events[1].1, started + delay);
            assert_eq!(metrics.scans, 2);
            assert_eq!(
                metrics.cancelled,
                if phase == "registration" { 1 } else { 2 }
            );
            assert_eq!(
                events[2].1.duration_since(events[1].1),
                delay * 3,
                "Rest was consumed by {phase}"
            );
            if phase == "cancelled preparation" {
                assert_eq!(metrics.visited, 0);
            } else {
                assert!(metrics.visited > 0);
            }
        }
    }

    #[test]
    fn stop_during_preparation_counts_work_and_skips_later_phases() {
        let state = LauncherState::new(Settings::default(), vec![]);
        let (sender, receiver) = sync_channel(1);
        sender.try_send(Request::Refresh).unwrap();
        let mut events = Vec::new();
        let delay = Duration::from_millis(20);
        let clock = TestClock::new();
        let metrics = run_worker_loop(
            &clock,
            &state,
            receiver,
            sender,
            |_, _| {
                clock.advance(delay);
                state.files.stop();
                // Even a resolver returning stale roots must not start watches.
                Some((vec![PathBuf::from("/obsolete")], None))
            },
            || events.push(state.files.status(0).phase),
            |_, _, _| panic!("registration after shutdown"),
        );
        assert_eq!(events, [FilePhase::Scanning, FilePhase::Idle]);
        assert_eq!(metrics.scans, 1);
        assert_eq!(metrics.cancelled, 1);
        assert_eq!(metrics.visited, 0);
        assert_eq!(metrics.duration, delay);
    }

    #[test]
    fn lifecycle_coalesces_requests_and_rejects_stale_publication() {
        let settings = Settings::default();
        let files = FileScan::new(&settings);
        let mut receiver = None;
        files
            .request(|rx, _| {
                receiver = Some(rx);
                Ok(())
            })
            .unwrap();
        let rx = receiver.unwrap();
        let generation = files.generation();
        assert_eq!(files.status(0).phase, FilePhase::Queued);
        assert!(files.begin(generation, &rx));
        assert!(
            rx.try_recv().is_err(),
            "pre-scan signal is covered by this scan"
        );
        for _ in 0..100 {
            assert!(!files.request(|_, _| panic!("second worker")).unwrap());
        }
        assert_eq!(files.status(0).phase, FilePhase::Scanning);
        let pending = files.finish(generation, &rx, false);
        assert!(matches!(pending, Some(Request::Refresh)));
        assert_eq!(files.status(0).phase, FilePhase::Queued);
        assert!(
            rx.try_recv().is_err(),
            "queued status must outlive consumed signals"
        );
        assert!(files.begin(generation, &rx));
        assert!(files.finish(generation, &rx, false).is_none());
        assert_eq!(files.status(0).phase, FilePhase::Idle);
        assert!(files.request(|_, _| panic!("second worker")).unwrap());
        assert_eq!(files.status(0).phase, FilePhase::Queued);
        assert!(files.begin(generation, &rx));
        files.invalidate(&settings);
        files.warning_for(generation, Some("obsolete".into()));
        files.finish(generation, &rx, false);
        assert_eq!(files.status(0).phase, FilePhase::Queued);
        assert!(files.status(0).warning.is_none());
        let disabled = Settings {
            file_search_roots: Some(vec![]),
            ..settings
        };
        files.invalidate(&disabled);
        files.finish(generation, &rx, true);
        assert!(!files.begin(files.generation(), &rx));
        assert_eq!(files.status(0).phase, FilePhase::Disabled);
    }

    #[test]
    fn cooldown_root_changes_use_latest_settings_and_disable_stays_disabled() {
        let state = LauncherState::new(
            Settings {
                file_watch_enabled: false,
                ..Settings::default()
            },
            vec![],
        );
        let (sender, receiver) = sync_channel(1);
        sender.try_send(Request::Refresh).unwrap();
        let cycle = Cell::new(0);
        let changed = Cell::new(false);
        let clock = TestClock::new();
        let base = clock.now();
        let mut events = Vec::new();
        *clock.waiting.borrow_mut() = Box::new(|| {
            assert_eq!(state.files.status(0).phase, FilePhase::Queued);
            if cycle.get() == 1 && !changed.replace(true) {
                for root in ["/old", "/newest"] {
                    state.replace_settings(Settings {
                        file_search_roots: Some(vec![root.into()]),
                        file_watch_enabled: false,
                        ..Settings::default()
                    });
                    let _ = sender.try_send(Request::Refresh);
                }
            } else if cycle.get() == 2 {
                state.replace_settings(Settings {
                    file_search_roots: Some(vec![]),
                    ..state.settings()
                });
                assert_eq!(state.files.status(0).phase, FilePhase::Disabled);
                state.files.stop();
            }
        });
        let metrics = run_worker_loop(
            &clock,
            &state,
            receiver,
            sender.clone(),
            |settings, _| {
                cycle.set(cycle.get() + 1);
                if cycle.get() == 2 {
                    assert_eq!(
                        settings.file_search_roots,
                        Some(vec![PathBuf::from("/newest")])
                    );
                    assert_eq!(clock.now(), base + Duration::from_secs(4));
                }
                clock.advance(Duration::from_secs(1));
                let _ = sender.try_send(Request::Refresh);
                Some((vec![], None))
            },
            || events.push(state.files.status(0).phase),
            |_, _, _| panic!("watching is off"),
        );
        assert_eq!(
            events,
            [
                FilePhase::Scanning,
                FilePhase::Queued,
                FilePhase::Scanning,
                FilePhase::Queued
            ]
        );
        assert_eq!(metrics.scans, 2);
        assert_eq!(metrics.duration, Duration::from_secs(2));
        assert_eq!(state.files.status(0).phase, FilePhase::Disabled);
        assert_eq!(state.search.lock().unwrap().file_count(), 0);
    }

    #[test]
    fn disable_in_final_budget_gap_drops_old_watch_and_reenable_rests() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("example.txt"), "").unwrap();
        let settings = Settings {
            file_search_roots: Some(vec![directory.path().into()]),
            ..Settings::default()
        };
        let state = LauncherState::new(settings.clone(), vec![]);
        let (sender, receiver) = sync_channel(1);
        *state.files.sender.lock().unwrap() = Some(sender.clone());
        sender.try_send(Request::Refresh).unwrap();
        let registered = Cell::new(0);
        let budget_exits = Cell::new(0);
        let disabled = Cell::new(false);
        let disposal_started = Cell::new(false);
        let disposal_finished = Cell::new(false);
        let reenabled = Cell::new(false);
        let dropped = Arc::new(AtomicBool::new(false));
        let base = Instant::now();
        let work = Duration::from_secs(1);
        let disposal = Duration::from_secs(2);
        let clock = TestClock::new();
        clock.now.set(base);
        *clock.reading.borrow_mut() = Box::new(|now| {
            if !disabled.get() {
                if registered.get() == 1 && now.get() == base + work * 4 {
                    budget_exits.set(budget_exits.get() + 1);
                    if budget_exits.get() == 2 {
                        // wait_for_budget calls now AFTER keep_running. This
                        // is the final wait's successful exit, before the
                        // worker reads generation/settings or calls begin.
                        assert!(!dropped.load(Ordering::Acquire));
                        state.replace_settings(Settings {
                            file_search_roots: Some(vec![]),
                            ..settings.clone()
                        });
                        // Only the disable's own settings refresh is queued.
                        // No later request/change rescues obsolete ownership.
                        sender.try_send(Request::Refresh).unwrap();
                        disabled.set(true);
                    }
                }
            } else if !disposal_finished.get() {
                if !disposal_started.replace(true) {
                    assert_eq!(state.files.status(0).phase, FilePhase::Disabled);
                    assert_eq!(state.files.generation(), 1);
                    assert_eq!(state.search.lock().unwrap().file_count(), 0);
                } else {
                    // Original code reaches this on the next final budget
                    // check with the old watcher still alive. Fail BEFORE it
                    // can block on recv: the negative regression cannot hang.
                    assert!(
                        dropped.load(Ordering::Acquire),
                        "obsolete watcher retained after late begin rejection"
                    );
                    assert!(!reenabled.get());
                    assert_eq!(now.get(), base + work * 4);
                    now.set(now.get() + disposal);
                    disposal_finished.set(true);
                }
            }
        });
        *clock.waiting.borrow_mut() = Box::new(|| {
            if disabled.get() && !reenabled.replace(true) {
                assert!(disposal_finished.get());
                assert!(dropped.load(Ordering::Acquire));
                assert_eq!(state.files.status(0).phase, FilePhase::Disabled);
                state.replace_settings(settings.clone());
                sender.try_send(Request::Refresh).unwrap();
            }
        });
        let metrics = run_worker_loop(
            &clock,
            &state,
            receiver,
            sender.clone(),
            |settings, _| {
                if reenabled.get() {
                    assert_eq!(registered.get(), 1);
                    assert_eq!(
                        clock.now(),
                        base + work * 4 + disposal * 4,
                        "re-enable bypassed disposal rest"
                    );
                }
                Some((settings.file_search_roots.clone().unwrap(), None))
            },
            || {
                if registered.get() == 2 {
                    state.files.stop();
                }
            },
            |watcher, report, cancelled| {
                if registered.get() == 0 {
                    // Queue Refresh before registration can emit Changed, so
                    // the next cycle deterministically skips burst settling.
                    sender.try_send(Request::Refresh).unwrap();
                }
                // Real OS registration is exercised, not replaced by a stub.
                let result = watcher.update(report, cancelled);
                assert!(matches!(result, Some((true, None))), "{result:?}");
                registered.set(registered.get() + 1);
                if registered.get() == 1 {
                    watcher.drop_observer =
                        Some(file_watch::WatchDropObserver(Arc::clone(&dropped)));
                    clock.advance(work);
                }
                result
            },
        );
        assert!(disabled.get() && disposal_finished.get() && reenabled.get());
        assert_eq!(registered.get(), 2);
        assert_eq!(metrics.scans, 2);
        assert_eq!(metrics.cancelled, 0);
        assert_eq!(
            metrics.duration,
            work + disposal,
            "disposal was unaccounted"
        );
    }

    #[test]
    fn automatic_followup_remains_queued_through_rest_and_settling() {
        let state = LauncherState::new(
            Settings {
                file_watch_enabled: false,
                ..Settings::default()
            },
            vec![],
        );
        let (sender, receiver) = sync_channel(1);
        sender.try_send(Request::Refresh).unwrap();
        let clock = TestClock::new();
        let base = clock.now();
        let mut events = Vec::new();
        *clock.waiting.borrow_mut() = Box::new(|| {
            assert_eq!(state.files.status(0).phase, FilePhase::Queued);
        });
        let metrics = run_worker_loop(
            &clock,
            &state,
            receiver,
            sender.clone(),
            |_, cancelled| {
                if cancelled() {
                    return None;
                }
                clock.advance(Duration::from_secs(1));
                sender.try_send(Request::Changed).unwrap();
                Some((vec![], None))
            },
            || {
                events.push((state.files.status(0).phase, clock.now()));
                if events.len() == 3 {
                    state.files.stop();
                }
            },
            |_, _, _| panic!("watching is off"),
        );
        assert_eq!(
            events.iter().map(|event| event.0).collect::<Vec<_>>(),
            [
                FilePhase::Scanning,
                FilePhase::Queued,
                FilePhase::Scanning,
                FilePhase::Idle
            ]
        );
        assert_eq!(events[2].1, base + Duration::from_millis(4300));
        assert_eq!(metrics.duration, Duration::from_secs(1));
        assert_eq!(metrics.scans, 2);
    }

    #[test]
    fn scanner_spawn_failure_is_retryable() {
        let files = FileScan::default();
        assert!(
            files
                .request(|_, _| Err(std::io::Error::other("test failure")))
                .is_err()
        );
        assert_eq!(files.status(0).phase, FilePhase::Failed);
        assert!(files.status(0).warning.unwrap().contains("test failure"));
        let mut receiver = None;
        assert!(
            files
                .request(|rx, _| {
                    receiver = Some(rx);
                    Ok(())
                })
                .unwrap()
        );
        assert_eq!(files.status(0).phase, FilePhase::Queued);
        let rx = receiver.unwrap();
        assert!(files.begin(files.generation(), &rx));
        files.warning_for(files.generation(), None);
        files.finish(files.generation(), &rx, false);
        assert_eq!(files.status(0).phase, FilePhase::Idle);
        assert!(files.status(0).warning.is_none());
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
