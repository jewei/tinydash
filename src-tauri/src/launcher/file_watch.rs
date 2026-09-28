use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind,
};

use crate::providers::files::ScanReport;

#[derive(Clone, Copy, Debug)]
pub enum Request {
    Refresh,
    Changed,
    Stop,
}

/// Rest time is at least one second and three times the preceding worker cycle's
/// wall time, including root preparation, registration and cancelled work.
/// Sustained churn spends at most 25% of work + rest time doing filesystem work.
pub fn scan_rest(elapsed: Duration) -> Duration {
    Duration::from_secs(1).max(elapsed.saturating_mul(3))
}

// One clock covers work, rest, and settling. Tests advance it at phase
// boundaries without sleeps or assumptions about filesystem speed.
pub(super) trait WorkerClock {
    fn now(&self) -> Instant;
    fn recv_timeout(
        &self,
        receiver: &Receiver<Request>,
        timeout: Duration,
    ) -> Result<Request, RecvTimeoutError>;
}

pub(super) struct RealClock;

impl WorkerClock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn recv_timeout(
        &self,
        receiver: &Receiver<Request>,
        timeout: Duration,
    ) -> Result<Request, RecvTimeoutError> {
        receiver.recv_timeout(timeout)
    }
}

/// Coalesce requests without letting manual refresh or settings churn bypass
/// the work budget. The callback also drops obsolete watches during long rests.
/// Polling here checks only in-memory control state, never the filesystem.
pub(super) fn wait_for_budget(
    clock: &impl WorkerClock,
    receiver: &Receiver<Request>,
    deadline: &mut Instant,
    mut keep_running: impl FnMut(&mut Instant) -> bool,
) -> bool {
    loop {
        if !keep_running(deadline) {
            return false;
        }
        let remaining = deadline.saturating_duration_since(clock.now());
        if remaining.is_zero() {
            return true;
        }
        match clock.recv_timeout(receiver, remaining.min(Duration::from_millis(50))) {
            Ok(Request::Changed | Request::Refresh) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Request::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

// Capacity one retains a change that arrives during a scan without retaining
// thousands of filesystem paths. Settling combines bursts; scan_rest separately
// bounds the scan workload under sustained events.
pub fn settle(receiver: &Receiver<Request>, quiet: Duration, maximum: Duration) -> bool {
    settle_cancellable(&RealClock, receiver, quiet, maximum, || true)
}

pub(super) fn settle_cancellable(
    clock: &impl WorkerClock,
    receiver: &Receiver<Request>,
    quiet: Duration,
    maximum: Duration,
    mut keep_running: impl FnMut() -> bool,
) -> bool {
    let deadline = clock.now() + maximum;
    let mut quiet_until = clock.now() + quiet;
    loop {
        if !keep_running() {
            return false;
        }
        let remaining = deadline
            .min(quiet_until)
            .saturating_duration_since(clock.now());
        if remaining.is_zero() {
            return true;
        }
        match clock.recv_timeout(receiver, remaining.min(Duration::from_millis(50))) {
            Ok(Request::Changed) => quiet_until = clock.now() + quiet,
            Ok(Request::Refresh) => return true,
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Request::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

#[derive(Default)]
struct Changes {
    rearm: AtomicBool,
    error: Mutex<Option<String>>,
}

pub struct FileWatcher<W = RecommendedWatcher> {
    watcher: W,
    watched: BTreeMap<PathBuf, RecursiveMode>,
    roots: Vec<PathBuf>,
    changes: Arc<Changes>,
}

impl FileWatcher {
    pub fn new(
        roots: Vec<PathBuf>,
        excluded: Vec<String>,
        sender: SyncSender<Request>,
    ) -> notify::Result<Self> {
        let changes = Arc::new(Changes::default());
        let callback_changes = Arc::clone(&changes);
        let filter_roots = roots.clone();
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                match event {
                    Ok(event) if relevant(&event, &filter_roots, &excluded) => {
                        if event.need_rescan()
                            || matches!(
                                event.kind,
                                EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
                            )
                        {
                            callback_changes.rearm.store(true, Ordering::Release);
                        }
                    }
                    Ok(_) => return,
                    Err(error) => {
                        callback_changes.rearm.store(true, Ordering::Release);
                        if let Ok(mut warning) = callback_changes.error.lock() {
                            *warning = Some(format!(
                                "Automatic file updates failed: {error}. Use Refresh files if results are old."
                            ));
                        }
                    }
                }
                let _ = sender.try_send(Request::Changed);
            },
            Config::default().with_follow_symlinks(false),
        )?;
        Ok(Self {
            watcher,
            watched: BTreeMap::new(),
            roots,
            changes,
        })
    }
}

impl<W: Watcher> FileWatcher<W> {
    // None means cancelled: the caller must discard this watcher, including
    // any partially applied batch. Never commit an obsolete FSEvents batch.
    pub fn update(
        &mut self,
        report: &ScanReport,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<(bool, Option<String>)> {
        let mut desired = BTreeMap::new();
        for root in &self.roots {
            // A parent watch detects root removal and recreation. Resolve the
            // nearest existing parent for roots that have not been created yet.
            for parent in root.ancestors().skip(1) {
                if cancelled() {
                    return None;
                }
                if parent.is_dir() {
                    desired.insert(parent.to_owned(), RecursiveMode::NonRecursive);
                    break;
                }
            }
        }
        #[cfg(target_os = "linux")]
        for directory in &report.directories {
            if cancelled() {
                return None;
            }
            desired.insert(directory.clone(), RecursiveMode::NonRecursive);
        }
        #[cfg(not(target_os = "linux"))]
        for root in &self.roots {
            if cancelled() {
                return None;
            }
            if std::fs::symlink_metadata(root)
                .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
            {
                desired.insert(root.clone(), RecursiveMode::Recursive);
            }
        }
        if cancelled() {
            return None;
        }
        // inotify recursively walks excluded trees itself. On Linux, register
        // only the bounded directories already accepted by the scanner.
        let reset = self.changes.rearm.swap(false, Ordering::AcqRel);
        let removed: Vec<_> = self
            .watched
            .iter()
            .filter(|(path, mode)| reset || desired.get(*path) != Some(mode))
            .map(|(path, _)| path.clone())
            .collect();
        let added: Vec<_> = desired
            .iter()
            .filter(|(path, mode)| reset || self.watched.get(*path) != Some(mode))
            .map(|(path, mode)| (path.clone(), *mode))
            .collect();
        let mut warning = self
            .changes
            .error
            .lock()
            .ok()
            .and_then(|mut error| error.take());
        #[cfg(target_os = "linux")]
        if report.watches_limited {
            warning = Some("Automatic file updates reached the 8192-folder limit. Use smaller search roots or Refresh files.".into());
        }
        #[cfg(not(target_os = "linux"))]
        let _ = report;
        if removed.is_empty() && added.is_empty() {
            return Some((false, warning));
        }
        let mut registered = false;
        // FSEvents can apply the entire batch with one stream restart.
        if cancelled() {
            return None;
        }
        let mut paths = self.watcher.paths_mut();
        for path in removed {
            if cancelled() {
                return None;
            }
            let _ = paths.remove(&path); // The OS may have removed its watch already.
            self.watched.remove(&path);
        }
        for (path, mode) in added {
            if cancelled() {
                return None;
            }
            match paths.add(&path, mode) {
                Ok(()) => {
                    self.watched.insert(path, mode);
                    registered = true;
                }
                Err(error) => {
                    warning.get_or_insert_with(|| {
                        format!(
                            "Cannot watch {}: {error}. Use Refresh files if results are old.",
                            path.display()
                        )
                    });
                }
            };
        }
        if cancelled() {
            return None;
        }
        if let Err(error) = paths.commit() {
            self.watched.clear();
            // A failed registration must wait for a later event or manual retry.
            // Retrying through the follow-up scan would create a busy loop.
            registered = false;
            warning = Some(format!(
                "Cannot start automatic file updates: {error}. Use Refresh files."
            ));
        }
        (!cancelled()).then_some((registered, warning))
    }
}

fn relevant(event: &Event, roots: &[PathBuf], excluded: &[String]) -> bool {
    if event.need_rescan() {
        return true;
    }
    if matches!(
        event.kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Data(_))
    ) {
        return false; // Filename search does not depend on file contents or reads.
    }
    event.paths.is_empty()
        || event.paths.iter().any(|path| {
            roots.iter().any(|root| {
                if root.starts_with(path) {
                    return true;
                }
                let Ok(relative) = path.strip_prefix(root) else {
                    return false;
                };
                let renamed = matches!(event.kind, EventKind::Modify(ModifyKind::Name(_)));
                let mut components = relative.components().peekable();
                while let Some(component) = components.next() {
                    let name = component.as_os_str().to_string_lossy();
                    // A folder renamed to a hidden name changes the index. Work
                    // inside an already excluded tree does not need a rescan.
                    if (name.starts_with('.') && (components.peek().is_some() || !renamed))
                        || (components.peek().is_some()
                            && excluded.iter().any(|excluded| *excluded == name))
                    {
                        return false;
                    }
                }
                true
            })
        })
}

// Preserve a not-yet-existing suffix while resolving aliases in its parent.
pub fn resolve_root(path: &Path, cancelled: &dyn Fn() -> bool) -> Option<PathBuf> {
    if cancelled() {
        return None;
    }
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return (!cancelled()).then(|| path.to_owned());
    }
    for parent in path.ancestors() {
        if cancelled() {
            return None;
        }
        if let Ok(resolved) = parent.canonicalize()
            && let Ok(suffix) = path.strip_prefix(parent)
        {
            return (!cancelled()).then(|| resolved.join(suffix));
        }
    }
    (!cancelled()).then(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, RenameMode};
    use std::sync::mpsc::sync_channel;

    #[test]
    fn ignores_reads_content_and_excluded_trees_but_keeps_renames_and_overflow() {
        let roots = vec![PathBuf::from("/home/test/Documents")];
        let excluded = vec!["node_modules".into()];
        for path in [
            "/home/test/Other/a.txt",
            "/home/test/Documents/.hidden",
            "/home/test/Documents/node_modules/pkg/a.txt",
        ] {
            assert!(!relevant(
                &Event::new(EventKind::Create(CreateKind::File)).add_path(path.into()),
                &roots,
                &excluded
            ));
        }
        assert!(!relevant(
            &Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
                .add_path(roots[0].join("a.txt")),
            &roots,
            &excluded
        ));
        assert!(relevant(
            &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
                .add_path(roots[0].join(".hidden")),
            &roots,
            &excluded
        ));
        for path in ["node_modules/pkg/.cache", ".git/objects/renamed"] {
            assert!(!relevant(
                &Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Any)))
                    .add_path(roots[0].join(path)),
                &roots,
                &excluded
            ));
        }
        let mut overflow = Event::new(EventKind::Other);
        overflow.attrs.set_flag(notify::event::Flag::Rescan);
        assert!(relevant(&overflow, &roots, &excluded));
    }

    #[test]
    fn coalesces_bursts_and_preserves_a_change_during_a_scan() {
        let (sender, receiver) = sync_channel(1);
        for _ in 0..10_000 {
            let _ = sender.try_send(Request::Changed);
        }
        assert!(matches!(receiver.recv(), Ok(Request::Changed)));
        assert!(settle(
            &receiver,
            Duration::from_millis(1),
            Duration::from_millis(10)
        ));
        sender
            .try_send(Request::Changed)
            .expect("change while scanning");
        assert!(matches!(receiver.recv(), Ok(Request::Changed)));
        sender.try_send(Request::Stop).expect("stop");
        assert!(!settle(
            &receiver,
            Duration::from_millis(1),
            Duration::from_millis(10)
        ));
    }

    #[test]
    fn sustained_churn_has_a_bounded_scan_duty_and_frequency() {
        let mut work = Duration::ZERO;
        let mut rest = Duration::ZERO;
        // Include long scans and short cancelled scans. No upper cooldown cap
        // may turn a slow tree into a near-100%-duty scan loop.
        for millis in [0, 1, 20, 300, 2000, 60_000].into_iter().cycle().take(120) {
            let elapsed = Duration::from_millis(millis);
            let pause = scan_rest(elapsed);
            assert!(pause >= Duration::from_secs(1));
            work += elapsed;
            rest += pause;
        }
        assert!(rest >= work * 3);
        assert!(work + rest >= Duration::from_secs(120));
    }

    #[test]
    fn refresh_flood_cannot_bypass_budget_and_stop_is_not_lost_to_a_full_queue() {
        let (sender, receiver) = sync_channel(1);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(80);
        std::thread::scope(|scope| {
            let sender = &sender;
            scope.spawn(move || {
                while Instant::now() < deadline {
                    let _ = sender.try_send(Request::Refresh);
                    let _ = sender.try_send(Request::Changed);
                    std::thread::sleep(Duration::from_millis(1));
                }
            });
            assert!(wait_for_budget(
                &RealClock,
                &receiver,
                &mut { deadline },
                |_| true
            ));
        });
        assert!(started.elapsed() >= Duration::from_millis(80));
        let _ = sender.try_send(Request::Changed);
        assert!(!wait_for_budget(
            &RealClock,
            &receiver,
            &mut (Instant::now() + Duration::from_secs(60)),
            |_| false
        ));
    }

    struct ControlledWatcher {
        calls: Vec<&'static str>,
        cancel_at: &'static str,
        cancelled: Arc<AtomicBool>,
    }

    impl ControlledWatcher {
        fn operation(&mut self, phase: &'static str) {
            self.calls.push(phase);
            // Models cancellation while an OS operation was blocked. The next
            // operation must not begin after this one returns.
            if phase == self.cancel_at {
                self.cancelled.store(true, Ordering::Release);
            }
        }
    }

    impl Watcher for ControlledWatcher {
        fn new<F: notify::EventHandler>(_: F, _: Config) -> notify::Result<Self> {
            unreachable!()
        }
        fn watch(&mut self, _: &Path, _: RecursiveMode) -> notify::Result<()> {
            self.operation("add");
            Ok(())
        }
        fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
            self.operation("remove");
            Ok(())
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::NullWatcher
        }
        fn paths_mut(&mut self) -> Box<dyn notify::PathsMut + '_> {
            struct Batch<'a>(&'a mut ControlledWatcher);
            impl notify::PathsMut for Batch<'_> {
                fn add(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
                    self.0.watch(path, mode)
                }
                fn remove(&mut self, path: &Path) -> notify::Result<()> {
                    self.0.unwatch(path)
                }
                fn commit(self: Box<Self>) -> notify::Result<()> {
                    self.0.operation("commit");
                    Ok(())
                }
            }
            self.operation("begin");
            Box::new(Batch(self))
        }
    }

    #[test]
    fn cancellation_between_registration_operations_abandons_the_remaining_batch() {
        let directory = tempfile::tempdir().unwrap();
        let roots: Vec<_> = ["first", "second"]
            .map(|name| directory.path().join(name))
            .into();
        for root in &roots {
            std::fs::create_dir(root).unwrap();
        }
        let report = ScanReport {
            directories: roots.clone(),
            ..ScanReport::default()
        };
        for phase in ["begin", "remove", "add", "commit"] {
            let cancelled = Arc::new(AtomicBool::new(false));
            let mut watcher = FileWatcher {
                watcher: ControlledWatcher {
                    calls: vec![],
                    cancel_at: phase,
                    cancelled: Arc::clone(&cancelled),
                },
                watched: ["old-first", "old-second"]
                    .map(|name| (directory.path().join(name), RecursiveMode::NonRecursive))
                    .into(),
                roots: roots.clone(),
                changes: Arc::default(),
            };
            assert!(
                watcher
                    .update(&report, &|| cancelled.load(Ordering::Acquire))
                    .is_none()
            );
            let calls = &watcher.watcher.calls;
            assert_eq!(calls.last(), Some(&phase));
            assert_eq!(
                calls.iter().filter(|call| **call == phase).count(),
                1,
                "Obsolete {phase} batch continued: {calls:?}"
            );
            if phase != "commit" {
                assert!(!calls.contains(&"commit"));
            }
        }
    }

    #[test]
    fn root_resolution_checks_between_metadata_and_each_ancestor_lookup() {
        use std::cell::Cell;
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing/child");
        // Metadata, then missing child canonicalization, then missing parent.
        for cancel_at in 1..=4 {
            let checks = Cell::new(0);
            assert!(
                resolve_root(&missing, &|| {
                    checks.set(checks.get() + 1);
                    checks.get() == cancel_at
                })
                .is_none()
            );
            assert_eq!(checks.get(), cancel_at);
        }
        assert_eq!(
            resolve_root(&missing, &|| false),
            Some(
                directory
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join("missing/child")
            )
        );
    }

    #[test]
    fn native_watcher_detects_a_new_file_without_polling() {
        let directory = tempfile::tempdir().expect("directory");
        let root = directory.path().canonicalize().expect("root");
        let (sender, receiver) = sync_channel(1);
        let mut watcher = FileWatcher::new(vec![root.clone()], vec![], sender).expect("watcher");
        let report = ScanReport {
            directories: vec![root.clone()],
            ..ScanReport::default()
        };
        let (changed, warning) = watcher.update(&report, &|| false).expect("current");
        assert!(changed);
        assert!(warning.is_none(), "{warning:?}");
        std::fs::write(root.join("added.txt"), "local fixture").expect("write");
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Ok(Request::Changed)
        ));
    }
}
