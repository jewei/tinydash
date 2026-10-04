//! Watches app folders and indexed folders. Events only mark an index dirty;
//! `refresh.rs` rebuilds it the next time the launcher opens.

use std::{path::PathBuf, sync::Mutex};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _, event::ModifyKind};
use tauri::{AppHandle, Manager};

use crate::{platform, state::State};

/// Holds the OS watcher; dropping it stops watching.
#[derive(Default)]
pub struct Watcher(Mutex<Option<RecommendedWatcher>>);

/// Start watching, or restart after the indexed folders change.
pub fn watch(app: &AppHandle) {
    let state = app.state::<State>();
    let app_folders = platform::app_folders();
    let file_folders = state.settings.get().file_folders(&state.home_dir);
    let handle = app.clone();
    let (apps, files) = (app_folders.clone(), file_folders.clone());
    let created = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if !changes_names(&event.kind) && !event.need_rescan() {
            return;
        }
        let state = handle.state::<State>();
        let under = |roots: &[PathBuf]| {
            event
                .paths
                .iter()
                .any(|path| roots.iter().any(|root| path.starts_with(root)))
        };
        if under(&apps) || event.need_rescan() {
            state.freshness.apps.mark_dirty();
        }
        if under(&files) || event.need_rescan() {
            state.freshness.files.mark_dirty();
        }
    });
    let mut watcher = match created {
        Ok(watcher) => watcher,
        Err(error) => {
            tracing::warn!(%error, "File watching is unavailable; indexes refresh every 15 minutes");
            return;
        }
    };
    for folder in app_folders
        .iter()
        .chain(&file_folders)
        .filter(|f| f.is_dir())
    {
        if let Err(error) = watcher.watch(folder, RecursiveMode::Recursive) {
            tracing::warn!(%error, folder = %folder.display(), "Could not watch folder");
        }
    }
    *app.state::<Watcher>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(watcher);
}

/// Creating, removing, or renaming changes names; writing content does not.
fn changes_names(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Any
    )
}
