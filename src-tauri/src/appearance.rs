use tauri::{AppHandle, Emitter, EventTarget, Runtime};

use crate::launcher::window::LauncherAppearance;

#[derive(serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum AppearanceChange {
    Appearance(LauncherAppearance),
    Compact(bool),
    SystemGlass(bool),
}

/// Relay only validated appearance changes, never caller-selected event names.
/// Rust-originated settings/search/update events must not be spoofable via IPC.
#[tauri::command]
pub fn sync_appearance<R: Runtime>(
    app: AppHandle<R>,
    change: AppearanceChange,
) -> Result<(), String> {
    for label in ["main", "settings"] {
        let target = EventTarget::webview_window(label);
        let result = match change {
            AppearanceChange::Appearance(value) => app.emit_to(target, "appearance-changed", value),
            AppearanceChange::Compact(value) => app.emit_to(target, "compact-changed", value),
            AppearanceChange::SystemGlass(value) => {
                app.emit_to(target, "system-glass-changed", value)
            }
        };
        result.map_err(|error| error.to_string())?;
    }
    Ok(())
}
