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

// Capacity one retains a change that arrives during a scan without retaining
// thousands of filesystem paths. The worker only scans after a burst settles.
pub fn settle(receiver: &Receiver<Request>, quiet: Duration, maximum: Duration) -> bool {
    let deadline = Instant::now() + maximum;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return true;
        }
        match receiver.recv_timeout(quiet.min(remaining)) {
            Ok(Request::Changed) => {}
            Ok(Request::Refresh) | Err(RecvTimeoutError::Timeout) => return true,
            Ok(Request::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

#[derive(Default)]
struct Changes {
    rearm: AtomicBool,
    error: Mutex<Option<String>>,
}

pub struct FileWatcher {
    watcher: RecommendedWatcher,
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

    // Return whether a second scan is needed to cover registration gaps.
    pub fn update(&mut self, report: &ScanReport) -> (bool, Option<String>) {
        let mut desired = BTreeMap::new();
        for root in &self.roots {
            // A parent watch detects root removal and recreation. Resolve the
            // nearest existing parent for roots that have not been created yet.
            if let Some(parent) = root.ancestors().skip(1).find(|path| path.is_dir()) {
                desired.insert(parent.to_owned(), RecursiveMode::NonRecursive);
            }
        }
        #[cfg(target_os = "linux")]
        for directory in &report.directories {
            desired.insert(directory.clone(), RecursiveMode::NonRecursive);
        }
        #[cfg(not(target_os = "linux"))]
        for root in &self.roots {
            if std::fs::symlink_metadata(root)
                .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink())
            {
                desired.insert(root.clone(), RecursiveMode::Recursive);
            }
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
            return (false, warning);
        }
        let mut registered = false;
        // FSEvents can apply the entire batch with one stream restart.
        let mut paths = self.watcher.paths_mut();
        for path in removed {
            let _ = paths.remove(&path); // The OS may have removed its watch already.
            self.watched.remove(&path);
        }
        for (path, mode) in added {
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
        if let Err(error) = paths.commit() {
            self.watched.clear();
            // A failed registration must wait for a later event or manual retry.
            // Retrying through the follow-up scan would create a busy loop.
            registered = false;
            warning = Some(format!(
                "Cannot start automatic file updates: {error}. Use Refresh files."
            ));
        }
        (registered, warning)
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
pub fn resolve_root(path: &Path) -> PathBuf {
    if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return path.to_owned();
    }
    for parent in path.ancestors() {
        if let Ok(resolved) = parent.canonicalize()
            && let Ok(suffix) = path.strip_prefix(parent)
        {
            return resolved.join(suffix);
        }
    }
    path.to_owned()
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
    fn native_watcher_detects_a_new_file_without_polling() {
        let directory = tempfile::tempdir().expect("directory");
        let root = directory.path().canonicalize().expect("root");
        let (sender, receiver) = sync_channel(1);
        let mut watcher = FileWatcher::new(vec![root.clone()], vec![], sender).expect("watcher");
        let report = ScanReport {
            directories: vec![root.clone()],
            ..ScanReport::default()
        };
        let (changed, warning) = watcher.update(&report);
        assert!(changed);
        assert!(warning.is_none(), "{warning:?}");
        std::fs::write(root.join("added.txt"), "local fixture").expect("write");
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Ok(Request::Changed)
        ));
    }
}
