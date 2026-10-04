//! Events sent to the windows. Names match `src/lib/ipc.ts`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::{search::Category, settings::Settings};

pub const LAUNCHER_SHOWN: &str = "launcher:shown";
pub const RESULTS_STALE: &str = "results:stale";
pub const SETTINGS_CHANGED: &str = "settings:changed";

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LauncherShown {
    /// Open this category with an empty query; `None` opens All.
    pub category: Option<Category>,
}

pub fn launcher_shown(app: &AppHandle, category: Option<Category>) {
    if let Err(error) = app.emit_to(
        crate::window::LAUNCHER,
        LAUNCHER_SHOWN,
        LauncherShown { category },
    ) {
        tracing::debug!(%error, "The launcher did not receive the show event");
    }
}

/// Indexed data changed; the launcher should run its search again.
pub fn results_stale(app: &AppHandle) {
    if let Err(error) = app.emit(RESULTS_STALE, ()) {
        tracing::debug!(%error, "No window received the update");
    }
}

pub fn settings_changed(app: &AppHandle, settings: &Settings) {
    if let Err(error) = app.emit(SETTINGS_CHANGED, settings) {
        tracing::debug!(%error, "No window received the settings");
    }
}
