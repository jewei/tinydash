use super::{Capabilities, MediaAction, UtilityResult, WindowAction, effects_allowed};
#[cfg(target_os = "linux")]
#[path = "desktop/linux.rs"]
mod linux;
#[cfg(target_os = "macos")]
#[path = "desktop/macos.rs"]
pub(super) mod macos;
#[cfg(target_os = "windows")]
#[path = "desktop/windows.rs"]
mod windows;
#[cfg(target_os = "linux")]
use super::run;
#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use macos as native;
pub use native::{WindowTarget, capture_target, capture_window, media, window};
#[cfg(target_os = "windows")]
use windows as native;

pub fn capabilities() -> Capabilities {
    Capabilities {
        processes: "Your non-system processes only. Quit requests SIGTERM on Unix or closes the main window on Windows; force kill can lose unsaved work. Refresh manually.".into(),
        native_eyedropper: cfg!(target_os = "macos"),
        eyedropper: if cfg!(target_os = "macos") { "Uses the macOS system color sampler. Click a pixel or press Escape to cancel. Results are converted to 8-bit sRGB. The system sampler remains open if the panel closes; Escape dismisses it." } else { "Screen sampling requires the secure WebView EyeDropper API. If unavailable, enter a color; there is no native fallback on this platform." }.into(),
        awake: if cfg!(target_os="linux") {"Requires logind and permission to inhibit idle/sleep. Does not keep the display lit; explicit sleep may be blocked by logind policy."} else {"Prevents idle system sleep only; display sleep, manual sleep and lid closing remain OS-controlled."}.into(),
        media: if cfg!(target_os="linux") {"Requires playerctl for the current MPRIS player; volume uses wpctl (default output)."} else {"Sends generic media keys to the OS-selected player; volume controls default system output. Some players/devices ignore requests."}.into(),
        windows: if cfg!(target_os="macos") {"Requires Accessibility access. Capture the target before showing TinyDash, or use the 3-second capture. Uses the target display's current work area, excluding the Dock and menu bar."} else if cfg!(target_os="windows") {"Capture the target before showing TinyDash, or use the 3-second capture. Uses the target monitor work area. Elevated windows may refuse changes."} else {"X11 only, requires xdotool, xprop and wmctrl. Uses the desktop work area; multi-monitor tiling is not implemented. Native Wayland is unsupported."}.into(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
fn placement(original: Rect, area: Rect, action: WindowAction) -> Rect {
    match action {
        WindowAction::Restore => original,
        WindowAction::Maximize => area,
        WindowAction::Left => Rect {
            width: area.width / 2,
            ..area
        },
        WindowAction::Right => Rect {
            x: area.x + area.width / 2,
            width: area.width - area.width / 2,
            ..area
        },
        WindowAction::Center => {
            let width = original.width.min(area.width);
            let height = original.height.min(area.height);
            Rect {
                x: area.x + (area.width - width) / 2,
                y: area.y + (area.height - height) / 2,
                width,
                height,
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_handles_negative_origins_and_odd_widths() {
        let area = Rect {
            x: -1001,
            y: 30,
            width: 1001,
            height: 700,
        };
        let original = Rect {
            x: 50,
            y: 50,
            width: 400,
            height: 300,
        };
        let right = placement(original, area, WindowAction::Right);
        assert_eq!((right.x, right.width), (-501, 501));
        let center = placement(original, area, WindowAction::Center);
        assert_eq!((center.x, center.y), (-701, 230));
        assert_eq!(placement(original, area, WindowAction::Restore).x, 50);
    }
}
