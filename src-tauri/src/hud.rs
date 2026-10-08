//! A short message that confirms a copy after the launcher hides, such as
//! "Copied". Its window never takes focus, so the app the user returned to
//! keeps it, and the pointer passes through.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use tauri::{AppHandle, LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{error::Result, events, window};

pub const HUD: &str = "hud";
const WIDTH: f64 = 160.0;
const HEIGHT: f64 = 44.0;
/// How long a message stays on screen.
const SHOWN_FOR: Duration = Duration::from_millis(1200);
/// Bumped by each message, so only the newest one hides the window.
static SHOWN: AtomicU64 = AtomicU64::new(0);

/// Create the hidden window at startup, so the first message shows at once.
pub fn create(app: &AppHandle) -> Result<()> {
    WebviewWindowBuilder::new(app, HUD, WebviewUrl::App("hud.html".into()))
        .title("TinyDash")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    Ok(())
}

/// Show `message` near the bottom of the screen the launcher was on, then
/// hide it. A problem only loses the message.
pub fn show(app: &AppHandle, message: &str) {
    let Some(hud) = app.get_webview_window(HUD) else {
        return;
    };
    let shown = SHOWN.fetch_add(1, Ordering::SeqCst) + 1;
    let result = place(app, &hud).and_then(|()| {
        events::hud(app, message);
        hud.show()?;
        hud.set_ignore_cursor_events(true)?;
        Ok(())
    });
    if let Err(error) = result {
        tracing::debug!(%error, "Could not show the message");
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SHOWN_FOR);
        if SHOWN.load(Ordering::SeqCst) == shown
            && let Some(hud) = app.get_webview_window(HUD)
        {
            hud.hide().ok();
        }
    });
}

/// Centered, four fifths down the work area of the launcher's screen, in
/// that screen's points.
fn place(app: &AppHandle, hud: &WebviewWindow) -> Result<()> {
    let monitor = app
        .get_webview_window(window::LAUNCHER)
        .and_then(|launcher| launcher.current_monitor().ok().flatten())
        .or_else(|| hud.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let area = monitor.work_area();
    let position = area.position.to_logical::<f64>(monitor.scale_factor());
    let size = area.size.to_logical::<f64>(monitor.scale_factor());
    hud.set_position(LogicalPosition::new(
        position.x + (size.width - WIDTH) / 2.0,
        position.y + size.height * 0.8 - HEIGHT / 2.0,
    ))?;
    Ok(())
}
