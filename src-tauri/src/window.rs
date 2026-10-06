//! The launcher and Settings windows.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::{
    error::{Error, Result},
    events, platform, refresh,
    search::Category,
    shortcut,
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
    if let Err(error) = platform::place_launcher(app, &window) {
        tracing::debug!(%error, "Could not place the launcher");
    }
    // Reset the view before it becomes visible, so the old query never flashes.
    *app.state::<State>()
        .shown_category
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = category;
    events::launcher_shown(app, category);
    window.show()?;
    window.set_focus()?;
    refresh::on_launcher_shown(app);
    Ok(())
}

/// Show the launcher again as it was, without a reset, so the error of an
/// action that hid it shows where the user ran it.
pub fn show_again(app: &AppHandle) -> Result<()> {
    let window = launcher(app)?;
    window.show()?;
    window.set_focus()?;
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

pub fn open_settings(app: &AppHandle) -> Result<()> {
    hide(app)?;
    if let Some(window) = app.get_webview_window(SETTINGS) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("TinyDash Settings")
        .inner_size(760.0, 560.0)
        .min_inner_size(640.0, 440.0)
        .center()
        .build()?;
    // TinyDash has no Dock icon, so it must bring itself to the front.
    window.set_focus()?;
    Ok(())
}

pub fn on_event(window: &tauri::Window, event: &WindowEvent) {
    let app = window.app_handle();
    if window.label() == SETTINGS {
        // Settings may close while it records a shortcut; turn it back on.
        // Off the main thread: a settings change may hold the lock while it
        // waits for the main thread.
        if let WindowEvent::Destroyed = event {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = shortcut::pause(&app, false) {
                    tracing::warn!(%error, "Could not restore the shortcut");
                }
            });
        }
        return;
    }
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
