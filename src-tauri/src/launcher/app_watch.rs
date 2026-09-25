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
        path::PathBuf,
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
        },
        platform::{self, AppChange},
    };

    // Bound memory during a large install. More paths cause a full scan.
    const PATH_LIMIT: usize = 256;

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

        // Compare each changed application with the index. An app update
        // or launch changes files inside a bundle but not the result list.
        pub fn needs_scan(&self, indexed: impl Fn(&str) -> bool) -> bool {
            self.scan
                || self.apps.iter().any(|path| {
                    let exists = std::fs::symlink_metadata(path).is_ok();
                    exists != indexed(&format!("app:{}", path.to_string_lossy()))
                })
        }
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
                    let needed = app
                        .state::<LauncherState>()
                        .search
                        .lock()
                        .map(|search| changes.needs_scan(|id| search.has_app(id)))
                        .unwrap_or(false);
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

    use tauri::AppHandle;

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

    use super::folders::Pending;

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
        assert!(!pending.needs_scan(indexed(true)));
        // A new application, or one that has not been indexed yet, needs a scan.
        assert!(pending.needs_scan(indexed(false)));
        // So does a removed application that is still indexed.
        std::fs::remove_dir(&app).expect("remove");
        assert!(pending.needs_scan(indexed(true)));

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
        assert!(!pending.needs_scan(|_| false));

        let folder = event(EventKind::Remove(RemoveKind::Folder), root.join("Tools"));
        assert!(pending.record(&folder, &roots));
        assert!(pending.needs_scan(|_| true));

        let mut pending = Pending::default();
        let dropped = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert!(pending.record(&dropped, &roots));
        assert!(pending.needs_scan(|_| true));
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
        assert!(pending.lock().expect("pending").needs_scan(|_| false));
    }
}
