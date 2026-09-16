use tauri::{AppHandle, Emitter, Manager};

use super::LauncherState;
use crate::error::{Error, Result};

pub fn show(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| Error::Launch("Launcher window is unavailable".into()))?;
    window.unminimize()?;
    window.show()?;
    window.set_focus()?;
    let clear = app
        .try_state::<LauncherState>()
        .is_none_or(|state| state.settings.clear_query_on_open);
    window.emit("launcher-opened", clear)?;
    tracing::debug!("Launcher shown");
    Ok(())
}

pub fn toggle(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| Error::Launch("Launcher window is unavailable".into()))?;
    if window.is_visible()? && window.is_focused()? {
        window.hide()?;
        Ok(())
    } else {
        show(app)
    }
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> std::result::Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}
