//! Watches app folders and indexed folders. Events only mark an index dirty;
//! `refresh.rs` rebuilds it the next time the launcher opens.

use std::{path::PathBuf, sync::Mutex};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _, event::ModifyKind};
use tauri::{AppHandle, Manager};

use crate::{features::files, platform, state::State};

/// Holds the OS watcher; dropping it stops watching.
#[derive(Default)]
pub struct Watcher(Mutex<Option<RecommendedWatcher>>);

/// Start watching, or restart after the indexed folders change.
pub fn watch(app: &AppHandle) {
    let state = app.state::<State>();
    let app_folders = platform::app_folders();
    let settings = state.settings.get();
    let file_folders = settings.file_folders(&state.dirs.home);
    let excluded = settings.file_search_excluded_dirs.clone();
    let handle = app.clone();
    // FSEvents reports real paths (links resolved, case as on disk) and
    // inotify reports paths as watched, so match both forms.
    let (apps, roots) = (
        with_real_paths(&app_folders),
        with_real_paths(&file_folders),
    );
    let created = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if !changes_names(&event.kind) && !event.need_rescan() {
            return;
        }
        let state = handle.state::<State>();
        let in_apps = |path: &PathBuf| apps.iter().any(|root| path.starts_with(root));
        if event.need_rescan() || event.paths.iter().any(in_apps) {
            state.freshness.apps.mark_dirty();
        }
        let indexed = |path: &PathBuf| {
            let packages = platform::PACKAGE_EXTENSIONS;
            // The watcher reads the disk for `lists`, which stays free of it.
            files::lists(path, path.is_dir(), &roots, &excluded, packages, |folder| {
                platform::HIDDEN.is_some_and(|hidden| {
                    std::fs::symlink_metadata(folder).is_ok_and(|metadata| hidden(&metadata))
                })
            })
        };
        if event.need_rescan() || event.paths.iter().any(indexed) {
            state.freshness.files.mark_dirty();
        }
    });
    let mut watcher = match created {
        Ok(watcher) => watcher,
        Err(error) => {
            tracing::warn!(
                %error,
                "File watching is unavailable; indexes rescan when the launcher opens and they are more than 15 minutes old"
            );
            return;
        }
    };
    for folder in app_folders
        .iter()
        .chain(&file_folders)
        .filter(|f| f.is_dir())
    {
        let mode = if platform::RECURSIVE_WATCH {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if let Err(error) = watcher.watch(folder, mode) {
            tracing::warn!(%error, folder = %folder.display(), "Could not watch folder");
        }
    }
    *app.state::<Watcher>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(watcher);
}

/// Each folder, and its real path when that differs.
fn with_real_paths(folders: &[PathBuf]) -> Vec<PathBuf> {
    folders
        .iter()
        .flat_map(|folder| {
            let real = std::fs::canonicalize(folder).ok();
            std::iter::once(folder.clone()).chain(real.filter(|real| real != folder))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_the_real_path_of_a_folder() {
        let temp = std::env::temp_dir();
        let roundabout = temp.join("..").join(temp.file_name().unwrap());
        let real = std::fs::canonicalize(&temp).unwrap();
        let folders = [roundabout];
        assert_eq!(with_real_paths(&folders), [folders[0].clone(), real]);
    }
}
