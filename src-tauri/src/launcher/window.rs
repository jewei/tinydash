use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow,
};

use super::LauncherState;
use crate::error::{Error, Result};

pub fn show(app: &AppHandle) -> Result<()> {
    show_in_category(app, None)
}

pub fn show_category(app: &AppHandle, mode: super::query::SearchMode) -> Result<()> {
    show_in_category(app, Some(mode))
}

fn show_in_category(app: &AppHandle, mode: Option<super::query::SearchMode>) -> Result<()> {
    let started = std::time::Instant::now();
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| Error::Launch("Launcher window is unavailable".into()))?;
    #[cfg(target_os = "macos")]
    if let Some(state) = app.try_state::<LauncherState>() {
        state.focus.remember();
    }
    window.unminimize()?;
    // A resident window can retain coordinates from a disconnected or resized display.
    // Placement failure must not prevent the launcher from opening.
    if let Err(error) = ensure_visible(&window) {
        tracing::debug!(%error, "Could not restore launcher position");
    }
    window.show()?;
    window.set_focus()?;
    let clear = app
        .try_state::<LauncherState>()
        .is_none_or(|state| state.settings().clear_query_on_open);
    window.emit("launcher-opened", clear || mode.is_some())?;
    if let Some(mode) = mode {
        window.emit("launcher-category", mode)?;
    }
    super::clipboard::refresh(app);
    super::currency::refresh(app, false);
    tracing::debug!(elapsed_us = started.elapsed().as_micros(), "Launcher shown");
    Ok(())
}

fn ensure_visible(window: &WebviewWindow) -> Result<()> {
    // Wayland gives placement control to the compositor. Native dragging still works.
    if crate::platform::is_wayland() {
        return Ok(());
    }
    let Some(monitor) = window.current_monitor()? else {
        window.center()?;
        return Ok(());
    };
    // Use the window's current monitor: on macOS its physical coordinates use
    // the same backing scale as the window, including on mixed-DPI desktops.
    if let Some(position) = visible_position(
        window.outer_position()?,
        window.outer_size()?,
        monitor.work_area(),
    ) {
        window.set_position(position)?;
    }
    Ok(())
}

fn visible_position(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    area: &PhysicalRect<i32, u32>,
) -> Option<PhysicalPosition<i32>> {
    if area.size.width == 0 || area.size.height == 0 {
        return None;
    }
    let clamp = |value: i32, start: i32, available: u32, length: u32| {
        // Wide arithmetic supports negative monitor origins without overflow.
        // If the display is too small, keep the drag handle at its top-left.
        let end = (i64::from(start) + i64::from(available.saturating_sub(length)))
            .min(i64::from(i32::MAX));
        i64::from(value).clamp(i64::from(start), end) as i32
    };
    let adjusted = PhysicalPosition::new(
        clamp(position.x, area.position.x, area.size.width, size.width),
        clamp(position.y, area.position.y, area.size.height, size.height),
    );
    (adjusted != position).then_some(adjusted)
}

pub fn toggle(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| Error::Launch("Launcher window is unavailable".into()))?;
    if window.is_visible()? && window.is_focused()? {
        dismiss(app)
    } else {
        show(app)
    }
}

pub fn hide(app: &AppHandle) -> Result<()> {
    hide_window(app, false)
}

pub fn dismiss(app: &AppHandle) -> Result<()> {
    hide_window(app, true)
}

fn hide_window(app: &AppHandle, _restore_focus: bool) -> Result<()> {
    // Take the saved app before hiding. The resulting blur event can call hide again.
    #[cfg(target_os = "macos")]
    let previous = app
        .try_state::<LauncherState>()
        .and_then(|state| state.focus.take());
    if let Some(window) = app.get_webview_window("main")
        && window.is_visible()?
    {
        #[cfg(target_os = "macos")]
        let restore_focus = _restore_focus && window.is_focused()?;
        window.hide()?;
        if let Some(state) = app.try_state::<LauncherState>() {
            state.icons.cancel_window("main", || {
                let _ = app.emit("app-icons-ready", ());
            });
        }
        #[cfg(target_os = "macos")]
        if restore_focus && let Some(previous) = previous {
            previous.restore();
        }
        window.emit("launcher-hidden", ())?;
    }
    Ok(())
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> std::result::Result<(), String> {
    dismiss(&app).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, width: u32, height: u32) -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    #[test]
    fn restores_the_reported_window_on_a_retina_display() {
        assert_eq!(
            visible_position(
                PhysicalPosition::new(2288, 68),
                PhysicalSize::new(1440, 1102),
                &area(0, 68, 2940, 1844),
            ),
            Some(PhysicalPosition::new(1500, 68)),
        );
    }

    #[test]
    fn keeps_a_user_position_that_fits_in_the_work_area() {
        let screen = area(0, 34, 1470, 922);
        for position in [(200, 200), (0, 34), (750, 405)] {
            assert_eq!(
                visible_position(position.into(), (720, 551).into(), &screen),
                None,
            );
        }
    }

    #[test]
    fn respects_negative_origins_and_screen_edges() {
        let screen = area(-1920, -1080, 1920, 1040);
        for (position, expected) in [
            ((-500, -300), (-720, -591)),
            ((-2500, -1500), (-1920, -1080)),
            ((3000, 2000), (-720, -591)),
        ] {
            assert_eq!(
                visible_position(position.into(), (720, 551).into(), &screen),
                Some(expected.into()),
            );
        }
    }

    #[test]
    fn keeps_the_handle_accessible_when_the_screen_is_smaller_than_the_window() {
        assert_eq!(
            visible_position((100, 100).into(), (720, 551).into(), &area(0, 24, 640, 480)),
            Some((0, 24).into()),
        );
    }

    #[test]
    fn ignores_an_unavailable_work_area() {
        assert_eq!(
            visible_position((100, 100).into(), (720, 551).into(), &area(0, 0, 0, 0)),
            None,
        );
    }
}
