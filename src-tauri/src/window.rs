//! The launcher and Settings windows.

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::{
    error::{Error, Result},
    events::{self, LauncherShown},
    platform, refresh,
    search::Category,
    state::State,
};

pub const LAUNCHER: &str = "launcher";
pub const SETTINGS: &str = "settings";

fn launcher(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window(LAUNCHER)
        .ok_or_else(|| Error::msg("The launcher window is missing."))
}

/// Show the launcher on the screen with the pointer. A category opens it
/// with an empty query in that category.
pub fn show(app: &AppHandle, category: Option<Category>) -> Result<()> {
    let window = launcher(app)?;
    platform::remember_frontmost_app();
    if let Err(error) = place_on_active_screen(app, &window) {
        tracing::debug!(%error, "Could not place the launcher");
    }
    window.show()?;
    window.set_focus()?;
    app.emit_to(LAUNCHER, events::LAUNCHER_SHOWN, LauncherShown { category })?;
    refresh::on_launcher_shown(app);
    Ok(())
}

/// Hide without moving focus, for example when another app was clicked.
pub fn hide(app: &AppHandle) -> Result<()> {
    launcher(app)?.hide()?;
    Ok(())
}

/// Hide and give focus back to the app the user came from.
pub fn dismiss(app: &AppHandle) -> Result<()> {
    let window = launcher(app)?;
    if window.is_visible()? {
        window.hide()?;
        platform::restore_frontmost_app();
    }
    Ok(())
}

pub fn toggle(app: &AppHandle) -> Result<()> {
    let window = launcher(app)?;
    if window.is_visible()? && window.is_focused()? {
        dismiss(app)
    } else {
        show(app, None)
    }
}

/// Horizontally centered, a fifth of the way down the work area.
fn place_on_active_screen(app: &AppHandle, window: &WebviewWindow) -> Result<()> {
    let cursor = app.cursor_position()?;
    let Some(monitor) = app.monitor_from_point(cursor.x, cursor.y)? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let size = window.outer_size()?;
    let free_width = i64::from(area.size.width.saturating_sub(size.width));
    let free_height = i64::from(area.size.height.saturating_sub(size.height));
    let x = i64::from(area.position.x) + free_width / 2;
    let y = i64::from(area.position.y) + free_height / 5;
    window.set_position(PhysicalPosition::new(
        i32::try_from(x).unwrap_or(area.position.x),
        i32::try_from(y).unwrap_or(area.position.y),
    ))?;
    Ok(())
}

pub fn open_settings(app: &AppHandle) -> Result<()> {
    hide(app)?;
    if let Some(window) = app.get_webview_window(SETTINGS) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("TinyDash Settings")
        .inner_size(760.0, 560.0)
        .min_inner_size(640.0, 440.0)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}

pub fn on_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != LAUNCHER {
        return;
    }
    let app = window.app_handle();
    let result = match event {
        // The launcher stays resident: closing only hides it.
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(app)
        }
        WindowEvent::Focused(false) if app.state::<State>().settings.get().hide_on_blur => {
            hide(app)
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "Could not hide the launcher");
    }
}
