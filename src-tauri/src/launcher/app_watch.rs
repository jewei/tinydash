//! Rescans applications after an install, a removal, or a rename.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::LauncherState;

// Installers write in bursts. Scan once after the burst settles.
const QUIET: Duration = Duration::from_secs(2);
const MAXIMUM: Duration = Duration::from_secs(10);

pub fn start(app: &AppHandle) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    folders::start(app);
    #[cfg(target_os = "linux")]
    desktop_entries::start(app);
}

// A running scan can predate the change. Wait until it finishes.
fn wait_for_scan(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    while state.scanning.load(std::sync::atomic::Ordering::Acquire) {
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod folders {
    use std::{
        collections::BTreeSet,
        fs::Metadata,
        io,
        path::{Path, PathBuf},
        sync::{Arc, Mutex, mpsc},
    };

    use notify::{
        Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind,
    };
    use tauri::{AppHandle, Manager};

    use super::{MAXIMUM, QUIET, wait_for_scan};
    use crate::{
        launcher::{
            LauncherState,
            file_watch::{Request, settle},
            scan_apps,
            search::SearchManager,
        },
        platform::{self, AppChange},
    };

    // Bound memory during a large install. More paths cause a full scan.
    pub(super) const PATH_LIMIT: usize = 256;

    #[derive(Default)]
    pub(super) struct Pending {
        pub apps: BTreeSet<PathBuf>,
        pub scan: bool,
    }

    impl Pending {
        pub fn record(&mut self, event: &Event, roots: &[PathBuf]) -> bool {
            if event.need_rescan() {
                self.scan = true;
                return true;
            }
            if matches!(event.kind, EventKind::Access(_)) {
                return false;
            }
            // Only these kinds can add or remove a folder of applications.
            let structural = matches!(
                event.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(_))
                    | EventKind::Any
                    | EventKind::Other
            );
            let mut relevant = false;
            for path in &event.paths {
                match platform::app_change(path, roots) {
                    Some(AppChange::App(app)) => {
                        relevant = true;
                        if self.apps.len() < PATH_LIMIT {
                            self.apps.insert(app);
                        } else {
                            self.scan = true;
                        }
                    }
                    Some(AppChange::Folder) if structural => {
                        relevant = true;
                        self.scan = true;
                    }
                    _ => {}
                }
            }
            relevant
        }

        // Observe the filesystem before acquiring search. Metadata can block,
        // even for one path; a failed probe is not evidence of a removal.
        pub fn observe(&self, mut metadata: impl FnMut(&Path) -> io::Result<Metadata>) -> Observed {
            let mut observed = Observed {
                apps: Vec::new(),
                scan: self.scan,
            };
            if !self.scan {
                for path in &self.apps {
                    let exists = match metadata(path) {
                        Ok(_) => true,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                        Err(error) => {
                            tracing::debug!(%error, "Cannot observe application change");
                            continue;
                        }
                    };
                    observed
                        .apps
                        .push((format!("app:{}", path.to_string_lossy()), exists));
                }
            }
            observed
        }
    }

    pub(super) struct Observed {
        apps: Vec<(String, bool)>,
        scan: bool,
    }

    impl Observed {
        // Only in-memory comparisons while search is locked. An update or
        // launch changes bundle contents, but not membership in the index.
        pub fn needs_scan(&self, indexed: impl Fn(&str) -> bool) -> bool {
            self.scan || self.apps.iter().any(|(id, exists)| *exists != indexed(id))
        }
    }

    pub(super) fn scan_needed(
        changes: &Pending,
        search: &Mutex<SearchManager>,
        metadata: impl FnMut(&Path) -> io::Result<Metadata>,
    ) -> bool {
        let observed = changes.observe(metadata);
        search
            .lock()
            .map(|search| observed.needs_scan(|id| search.has_app(id)))
            .unwrap_or(false)
    }

    pub fn start(app: &AppHandle) {
        let roots: Vec<PathBuf> = platform::app_folders()
            .into_iter()
            .filter(|root| root.is_dir())
            .collect();
        let (sender, receiver) = mpsc::sync_channel(1);
        let pending = Arc::new(Mutex::new(Pending::default()));
        let callback_pending = Arc::clone(&pending);
        let callback_roots = roots.clone();
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                let Ok(mut pending) = callback_pending.lock() else {
                    return;
                };
                let relevant = match event {
                    Ok(event) => pending.record(&event, &callback_roots),
                    Err(error) => {
                        tracing::debug!(%error, "Application folder watch failed");
                        pending.scan = true;
                        true
                    }
                };
                drop(pending);
                if relevant {
                    let _ = sender.try_send(Request::Changed);
                }
            },
            Config::default().with_follow_symlinks(false),
        );
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::warn!(%error, "Cannot watch application folders");
                return;
            }
        };
        for root in &roots {
            if let Err(error) = watcher.watch(root, RecursiveMode::Recursive) {
                tracing::warn!(%error, path = %root.display(), "Cannot watch an application folder");
            }
        }
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("app-watch".into())
            .spawn(move || {
                let _watcher = watcher;
                while let Ok(Request::Changed) = receiver.recv() {
                    if !settle(&receiver, QUIET, MAXIMUM) {
                        break;
                    }
                    let changes = std::mem::take(
                        &mut *pending.lock().unwrap_or_else(|error| error.into_inner()),
                    );
                    wait_for_scan(&app);
                    let needed =
                        scan_needed(&changes, &app.state::<LauncherState>().search, |path| {
                            std::fs::symlink_metadata(path)
                        });
                    if needed {
                        scan_apps(&app);
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "Cannot start application folder updates");
        }
    }
}

#[cfg(target_os = "linux")]
mod desktop_entries {
    use std::{cell::RefCell, sync::mpsc};

    use tauri::{AppHandle, Manager};

    use super::{MAXIMUM, QUIET, wait_for_scan};
    use crate::launcher::{
        file_watch::{Request, settle},
        scan_apps,
    };

    thread_local! {
        // GIO emits signals only while the monitor is alive.
        static MONITOR: RefCell<Option<gio::AppInfoMonitor>> = const { RefCell::new(None) };
    }

    // GIO reports changes to desktop entries in every XDG data folder,
    // including Flatpak and Snap exports. It runs on the GTK main thread.
    pub fn start(app: &AppHandle) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let registered = app.run_on_main_thread(move || {
            let monitor = gio::AppInfoMonitor::get();
            monitor.connect_changed(move |_| {
                let _ = sender.try_send(Request::Changed);
            });
            MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
        });
        if let Err(error) = registered {
            tracing::warn!(%error, "Cannot watch application entries");
            return;
        }
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("app-watch".into())
            .spawn(move || {
                while let Ok(Request::Changed) = receiver.recv() {
                    if !settle(&receiver, QUIET, MAXIMUM) {
                        break;
                    }
                    wait_for_scan(&app);
                    scan_apps(&app);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "Cannot start application entry updates");
        }
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use notify::{
        Event, EventKind,
        event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind},
    };

    use std::{
        io,
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    };

    use super::folders::{PATH_LIMIT, Pending, scan_needed};
    use crate::{
        launcher::{query::SearchMode, search::SearchManager},
        providers::apps::{AppEntry, AppProvider},
    };

    fn needs_scan(pending: &Pending, indexed: impl Fn(&str) -> bool) -> bool {
        pending
            .observe(|path| std::fs::symlink_metadata(path))
            .needs_scan(indexed)
    }

    const APP: &str = if cfg!(windows) {
        "Editor.lnk"
    } else {
        "Editor.app"
    };

    fn event(kind: EventKind, path: std::path::PathBuf) -> Event {
        Event::new(kind).add_path(path)
    }

    #[test]
    fn scans_only_when_an_application_or_its_folder_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let roots = [root.clone()];
        let app = root.join(APP);
        std::fs::create_dir(&app).expect("app");
        let id = format!("app:{}", app.to_string_lossy());
        let indexed = |present: bool| {
            let id = id.clone();
            move |candidate: &str| present && candidate == id
        };

        // An update or launch of an indexed application changes nothing.
        let mut pending = Pending::default();
        let modified = EventKind::Modify(ModifyKind::Data(DataChange::Content));
        assert!(pending.record(&event(modified, app.clone()), &roots));
        assert!(!needs_scan(&pending, indexed(true)));
        // A new application, or one that has not been indexed yet, needs a scan.
        assert!(needs_scan(&pending, indexed(false)));
        // So does a removed application that is still indexed.
        std::fs::remove_dir(&app).expect("remove");
        assert!(needs_scan(&pending, indexed(true)));
        assert!(!needs_scan(&pending, indexed(false)));

        let mut pending = Pending::default();
        for ignored in [
            event(EventKind::Access(AccessKind::Any), app.clone()),
            event(EventKind::Create(CreateKind::File), root.join("notes.txt")),
            event(EventKind::Create(CreateKind::Folder), root.join(".hidden")),
            event(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                root.join("Tools"),
            ),
        ] {
            assert!(!pending.record(&ignored, &roots), "{ignored:?}");
        }
        assert!(!needs_scan(&pending, |_| false));

        let folder = event(EventKind::Remove(RemoveKind::Folder), root.join("Tools"));
        assert!(pending.record(&folder, &roots));
        assert!(needs_scan(&pending, |_| true));

        let mut pending = Pending::default();
        let dropped = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(pending.record(&dropped, &roots));
        assert!(needs_scan(&pending, |_| true));
    }

    #[test]
    fn metadata_errors_are_not_confirmed_removals() {
        let pending = Pending {
            apps: [std::path::PathBuf::from(APP)].into(),
            scan: false,
        };
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Interrupted,
            io::ErrorKind::Other,
        ] {
            let observed = pending.observe(|_| Err(io::Error::from(kind)));
            // Unknown membership cannot imply either an install or a removal.
            assert!(!observed.needs_scan(|_| true), "{kind:?}");
            assert!(!observed.needs_scan(|_| false), "{kind:?}");
        }
        let absent = pending.observe(|_| Err(io::Error::from(io::ErrorKind::NotFound)));
        assert!(absent.needs_scan(|_| true));
        assert!(!absent.needs_scan(|_| false));

        // One failed probe must not hide another confirmed change in the burst.
        let mut mixed = pending;
        let removed = std::path::PathBuf::from(format!("Removed-{APP}"));
        mixed.apps.insert(removed.clone());
        let observed = mixed.observe(|path| {
            Err(io::Error::from(if path == removed.as_path() {
                io::ErrorKind::NotFound
            } else {
                io::ErrorKind::PermissionDenied
            }))
        });
        assert!(observed.needs_scan(|_| true));
    }

    #[test]
    fn rename_observes_both_old_and_new_application_ids() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let old = root.join(APP);
        let new = root.join(format!("Renamed-{APP}"));
        std::fs::write(&old, "").unwrap();
        let search = Mutex::new(SearchManager::default());
        search
            .lock()
            .unwrap()
            .replace_apps(AppProvider::new(vec![AppEntry::new(
                "Editor".into(),
                old.clone(),
                vec![],
            )]));
        std::fs::rename(&old, &new).unwrap();
        let mut pending = Pending::default();
        assert!(
            pending.record(
                &Event::new(EventKind::Modify(ModifyKind::Name(
                    notify::event::RenameMode::Both,
                )))
                .add_path(old)
                .add_path(new.clone()),
                &[root],
            )
        );
        assert_eq!(pending.apps.len(), 2);
        assert!(scan_needed(&pending, &search, |path| {
            std::fs::symlink_metadata(path)
        }));
        search
            .lock()
            .unwrap()
            .replace_apps(AppProvider::new(vec![AppEntry::new(
                "Editor".into(),
                new,
                vec![],
            )]));
        assert!(!scan_needed(&pending, &search, |path| {
            std::fs::symlink_metadata(path)
        }));
    }

    #[test]
    fn event_bursts_deduplicate_paths_and_overflow_to_one_scan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let roots = [root.clone()];
        let mut pending = Pending::default();
        let changed = event(EventKind::Create(CreateKind::Any), root.join(APP));
        for _ in 0..PATH_LIMIT * 2 {
            assert!(pending.record(&changed, &roots));
        }
        assert_eq!(pending.apps.len(), 1);
        assert!(!pending.scan);
        let mut probes = 0;
        pending.observe(|_| {
            probes += 1;
            Err(io::Error::from(io::ErrorKind::NotFound))
        });
        assert_eq!(probes, 1);
        for index in 0..PATH_LIMIT * 2 {
            assert!(pending.record(
                &event(
                    EventKind::Create(CreateKind::Any),
                    root.join(format!("{index}-{APP}")),
                ),
                &roots,
            ));
        }
        assert_eq!(pending.apps.len(), PATH_LIMIT);
        assert!(pending.scan);
        assert!(
            pending
                .observe(|_| panic!("a full scan needs no metadata probes"))
                .needs_scan(|_| false)
        );
    }

    #[test]
    fn search_completes_while_application_metadata_probe_is_blocked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = dir.path().join(APP);
        std::fs::write(&app, "").unwrap();
        let entry = AppEntry::new("Editor".into(), app.clone(), vec![]);
        let id = entry.id.clone();
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(vec![entry]));
        let search = Arc::new(Mutex::new(manager));
        let pending = Pending {
            apps: [app].into(),
            scan: false,
        };
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let watcher_search = search.clone();
        let watcher = std::thread::spawn(move || {
            // Exercise the same observation/lock orchestration used by start.
            scan_needed(&pending, &watcher_search, |path| {
                let _ = entered_tx.send(());
                release_rx
                    .recv_timeout(Duration::from_secs(15))
                    .expect("test must release the probe before its safety timeout");
                std::fs::symlink_metadata(path)
            })
        });
        let entered = entered_rx.recv_timeout(Duration::from_secs(5));
        let (done_tx, done_rx) = mpsc::channel();
        let searching = std::thread::spawn(move || {
            let result = search.lock().unwrap().search("Editor", SearchMode::Apps);
            let _ = done_tx.send(result);
        });
        let responsive = done_rx.recv_timeout(Duration::from_secs(5));
        // Always release and join before asserting. Moving observe inside the
        // search lock must fail, not strand either worker on a latch or mutex.
        let _ = release_tx.send(());
        let needed = watcher.join();
        let searched = searching.join();
        entered.expect("watcher must reach the controlled metadata probe");
        assert!(!needed.expect("watcher thread"));
        searched.expect("search thread");
        let outcome = responsive
            .expect("search must complete while the metadata probe is blocked")
            .expect("search outcome");
        assert!(outcome.notice.is_none());
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].id, id);
    }

    #[test]
    #[ignore = "Uses the native file watcher. Run with --ignored."]
    fn native_events_map_to_a_new_application() {
        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::{Arc, Mutex};
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("root");
        let roots = vec![root.clone()];
        let pending = Arc::new(Mutex::new(Pending::default()));
        let callback = Arc::clone(&pending);
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                if let Ok(event) = event {
                    callback.lock().expect("pending").record(&event, &roots);
                }
            },
            Config::default(),
        )
        .expect("watcher");
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .expect("watch");
        std::thread::sleep(std::time::Duration::from_millis(500));
        let app = root.join(APP);
        if cfg!(windows) {
            std::fs::write(&app, "").expect("shortcut");
        } else {
            std::fs::create_dir_all(app.join("Contents")).expect("bundle");
            std::fs::write(app.join("Contents/Info.plist"), "").expect("plist");
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !pending.lock().expect("pending").apps.contains(&app) {
            assert!(std::time::Instant::now() < deadline, "No event for {app:?}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(needs_scan(&pending.lock().expect("pending"), |_| false));
    }
}
