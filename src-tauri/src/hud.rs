//! A short message that confirms a copy after the launcher hides, such as
//! "Copied". Its window never takes focus, so the app the user returned to
//! keeps it, and the pointer passes through.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalPosition, PhysicalRect, Position, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::{error::Result, events, platform, window};

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

/// Centered, four fifths down the work area of the launcher's screen, taken
/// from that screen, not from the screen the message was on last.
fn place(app: &AppHandle, hud: &WebviewWindow) -> Result<()> {
    let monitor = app
        .get_webview_window(window::LAUNCHER)
        .and_then(|launcher| launcher.current_monitor().ok().flatten())
        .or_else(|| hud.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };
    hud.set_position(target(
        monitor.work_area(),
        monitor.scale_factor(),
        platform::PHYSICAL_PLACEMENT,
    ))?;
    Ok(())
}

/// Where the message goes on a screen with work area `area` and `scale`.
/// When `physical`, the position is in that screen's pixels, so the OS does
/// not convert it with the scale of the screen the message was on last.
/// Otherwise it is in logical points.
fn target(area: &PhysicalRect<i32, u32>, scale: f64, physical: bool) -> Position {
    if physical {
        let (x, y) = spot(
            (area.position.x.into(), area.position.y.into()),
            (area.size.width.into(), area.size.height.into()),
            (WIDTH * scale, HEIGHT * scale),
        );
        // `as` saturates, and the spot lies inside the work area.
        PhysicalPosition::new(x.round() as i32, y.round() as i32).into()
    } else {
        let position = area.position.to_logical::<f64>(scale);
        let size = area.size.to_logical::<f64>(scale);
        let (x, y) = spot(
            (position.x, position.y),
            (size.width, size.height),
            (WIDTH, HEIGHT),
        );
        LogicalPosition::new(x, y).into()
    }
}

/// The top-left corner of a window of `size`, centered across a work area
/// at `origin` of `area` size, with its middle four fifths down. Every
/// value is in one unit.
fn spot(origin: (f64, f64), area: (f64, f64), size: (f64, f64)) -> (f64, f64) {
    (
        origin.0 + (area.0 - size.0) / 2.0,
        origin.1 + area.1 * 0.8 - size.1 / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use tauri::PhysicalSize;

    use super::*;

    /// A 2560x1440 work area at 150%, to the right of a 1920-pixel screen.
    fn scaled_area() -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(1920, 0),
            size: PhysicalSize::new(2560, 1440),
        }
    }

    #[test]
    fn places_the_message_in_the_screens_own_pixels() {
        let position = target(&scaled_area(), 1.5, true);
        let expected =
            PhysicalPosition::new(1920 + (2560 - 240) / 2, (1440.0 * 0.8 - 66.0 / 2.0) as i32);
        assert!(matches!(position, Position::Physical(p) if p == expected));
    }

    #[test]
    fn places_the_message_in_logical_points_when_the_os_uses_them() {
        let position = target(&scaled_area(), 1.5, false);
        // 1920 / 1.5 = 1280, 2560 / 1.5 = 1706.67, 1440 / 1.5 = 960.
        let expected = LogicalPosition::new(
            1280.0 + (2560.0 / 1.5 - WIDTH) / 2.0,
            960.0 * 0.8 - HEIGHT / 2.0,
        );
        assert!(matches!(position, Position::Logical(p) if p == expected));
    }

    #[test]
    fn places_the_message_on_a_screen_left_of_the_primary() {
        let area = PhysicalRect {
            position: PhysicalPosition::new(-1920, 0),
            size: PhysicalSize::new(1920, 1080),
        };
        let position = target(&area, 1.0, true);
        let expected = PhysicalPosition::new(-1920 + (1920 - 160) / 2, 864 - 22);
        assert!(matches!(position, Position::Physical(p) if p == expected));
    }
}
