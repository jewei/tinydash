//! Everything that differs between macOS, Windows, and Linux.
//!
//! Each OS module provides the same items:
//!
//! - `FILE_MANAGER`: name used in "Show in …" labels.
//! - `PACKAGE_EXTENSIONS`: folder extensions the OS shows as one file; the
//!   file index lists such a folder but not its contents.
//! - `RECURSIVE_WATCH`: whether watching a folder also watches its subfolders.
//! - `TEMPLATE_TRAY_ICON`: the OS tints a black tray icon (macOS menu bar).
//! - `prepare_app(app)`: app-wide setup at startup, such as the Dock policy
//!   or the notification plugin.
//! - `is_reopen(event)`: the user opened the running app again, and the OS
//!   sent an event instead of starting a second process.
//! - `restrict_to_owner(path)`: make a file readable only by the user.
//! - `HIDDEN`: the OS's check for a hidden file or folder besides a name
//!   that starts with a dot (the Windows hidden attribute, the macOS hidden
//!   flag), or `None` when it has none, so the scan reads no metadata.
//! - `NATIVE_ICONS`: whether `app_icon` returns images.
//! - `PHYSICAL_PLACEMENT`: whether windows are placed in physical pixels,
//!   which Windows and X11 use the same way across monitors. When false
//!   (macOS), they are placed in points: tao turns a logical position into
//!   pixels with the window's own scale, which is right only there.
//! - `SELF_UPDATE`: whether an installed copy can replace itself with a new
//!   version (not on Linux, where installing a .deb needs root).
//! - `RICH_CLIPBOARD`: whether clipboard history can save images and copied
//!   files; `read_clipboard` ignores both settings when it cannot.
//! - `app_folders()`: folders to watch for installed or removed apps.
//! - `discover_apps()`: installed apps.
//! - `launch_app(path)`: start an app found by `discover_apps`.
//! - `app_icon(path, pixels)`: PNG bytes of a file's system icon.
//! - `run_system_command(command)`. Sleep Displays calls
//!   `wait_for_key_release` first, only where it is supported.
//! - `run_in_terminal(command, data)`: open the user's terminal app in the
//!   home folder, run `command` there as typed, and leave the user's login
//!   shell open after it. The shell goes to the home folder itself, before
//!   the command, because a terminal or a profile can start it elsewhere.
//!   The command never goes into a string that another parser reads with
//!   other quoting rules (see `terminal.rs`). macOS writes a script under
//!   `data` (zsh, in the app for `.command` files); Windows runs PowerShell
//!   in a new console; Linux writes the command to a file under `data` and
//!   runs `sh` in `$TERMINAL`, or in the first terminal it finds, and says
//!   to set `$TERMINAL` when it finds none.
//! - `notify(app, title, body)`: show a system notification.
//! - `disk_space(path)`: the [`Volume`] that holds `path`.
//! - `find_files(name, folder, limit)`: up to `limit` paths under `folder`
//!   whose names contain `name`, from the OS's own file index (Spotlight on
//!   macOS), or none where TinyDash uses no such index. Blocks up to a second.
//! - `clipboard_change()`: a counter that changes with the clipboard content.
//! - `note_clipboard_change(change)`: the monitor looked at the counter, at
//!   every tick; for a new change, remember what makes it secret but the
//!   content does not show (on macOS, a copy made in Passwords or Keychain
//!   Access, which was in front now or at the previous look). Reads no
//!   content. On macOS, `remember_frontmost_app` also calls it, for a copy
//!   made just before the launcher opened, and `read_clipboard` takes the
//!   same look before it reads, for a copy newer than the monitor's last
//!   tick.
//! - `read_clipboard(images, files)`: the content for the latest change, or
//!   `None` when its source marked it secret or it cannot be read.
//! - `clipboard_image()`: the clipboard image as RGBA, or `None` when there
//!   is none, it cannot be decoded, or it has more pixels than the history
//!   keeps. The size comes from the image header before the image is
//!   decoded. On macOS the encoded bytes are copied in first. Only macOS
//!   and Windows, where `read_clipboard` saves images; call it with the
//!   clipboard reader lock held.
//! - `exclude_from_history(set)`: mark a copy secret for clipboard managers.
//! - `clipboard_text(app)`: the clipboard's text now, or `None` when there
//!   is none or its source marked it secret. History need not be on. Call
//!   it off the main thread, which Linux waits for.
//! - `watch_clipboard(capturing)`: start change notifications; call on the
//!   main thread. `capturing` says whether history is on, so nothing is read
//!   while it is off.
//! - `prepare_launcher(window)`: native window tweaks; call on the main thread.
//! - `place_launcher(app, window, saved)`: move the launcher to `saved` when
//!   that spot is still on a screen, else to the center of the screen with
//!   the pointer.
//! - `launcher_position(window)`: where the launcher is now, in the units
//!   `place_launcher` takes.
//! - `remember_frontmost_app()` / `restore_frontmost_app()`: return focus after Escape.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod terminal;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(target_os = "windows")]
pub use windows::*;

use crate::error::{Error, Result};
#[cfg(not(target_os = "linux"))]
use crate::features::clipboard::{Content, MAX_IMAGE_BYTES};
#[cfg(not(target_os = "macos"))]
use crate::settings::LauncherPosition;

/// A disk, as the file manager shows it.
pub struct Volume {
    /// Its name: the Finder name on macOS, the drive on Windows (`C:`), the
    /// mount point on Linux (`/`).
    pub name: String,
    pub total_bytes: u64,
    /// Space left to fill. On macOS this counts purgeable space, as Finder does.
    pub free_bytes: u64,
}

/// Clipboard formats that password managers set to ask history tools to skip
/// a copy. See <http://nspasteboard.org> and the Windows clipboard docs.
pub const SECRET_FORMATS: &[&str] = &[
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "org.nspasteboard.AutoGeneratedType",
    "com.apple.is-sensitive",
    "ExcludeClipboardContentFromMonitorProcessing",
    "Clipboard Viewer Ignore",
    "x-kde-passwordManagerHint",
];

/// Move the launcher to `saved` when the middle of its top edge would be on
/// a screen, so it can still be dragged; else center it in the work area of
/// the monitor under the pointer. Positions are in physical pixels, which
/// Windows and X11 use consistently across monitors; the window's size is
/// counted at the target monitor's scale (see `centered`).
///
/// It checks the monitors itself rather than with `monitor_from_point`: on
/// Linux, that one takes logical pixels and returns the nearest monitor for
/// a point on none, so a spot on an unplugged monitor would still pass.
#[cfg(not(target_os = "macos"))]
fn place_in_physical_pixels(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    saved: Option<LauncherPosition>,
) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    let bounds: Vec<_> = monitors
        .iter()
        .map(|monitor| (*monitor.position(), *monitor.size()))
        .collect();
    let size = window.outer_size()?;
    if let Some(saved) = saved
        && monitor_containing(
            &bounds,
            saved.x + f64::from(size.width) / 2.0,
            saved.y + 1.0,
        )
        .is_some()
    {
        // `as` saturates, and the check above keeps the spot on a screen.
        return window.set_position(tauri::PhysicalPosition::new(saved.x as i32, saved.y as i32));
    }
    let cursor = app.cursor_position()?;
    let Some(monitor) =
        monitor_containing(&bounds, cursor.x, cursor.y).and_then(|index| monitors.get(index))
    else {
        return Ok(());
    };
    let area = monitor.work_area();
    window.set_position(centered(
        area.position,
        area.size,
        size.to_logical(window.scale_factor()?),
        monitor.scale_factor(),
    ))
}

/// Where a window of `size` sits in the center of a work area. Windows keeps
/// a window's logical size when it moves to a monitor with another scale, so
/// the size counts in the target monitor's scale, not the one it is on now.
#[cfg(any(not(target_os = "macos"), test))]
fn centered(
    area_position: tauri::PhysicalPosition<i32>,
    area_size: tauri::PhysicalSize<u32>,
    size: tauri::LogicalSize<f64>,
    monitor_scale: f64,
) -> tauri::PhysicalPosition<i32> {
    let size = size.to_physical::<u32>(monitor_scale);
    let free_width = i64::from(area_size.width.saturating_sub(size.width));
    let free_height = i64::from(area_size.height.saturating_sub(size.height));
    // The window has a fixed height and never grows, so it sits in the
    // exact center rather than high up, as launchers that grow do.
    let x = i64::from(area_position.x) + free_width / 2;
    let y = i64::from(area_position.y) + free_height / 2;
    tauri::PhysicalPosition::new(
        i32::try_from(x).unwrap_or(area_position.x),
        i32::try_from(y).unwrap_or(area_position.y),
    )
}

/// The index of the monitor whose bounds hold a point, all in physical
/// pixels. Each monitor holds its left and top edges but not its right and
/// bottom ones, so a point on the line between two monitors has one owner.
#[cfg(any(not(target_os = "macos"), test))]
fn monitor_containing(
    bounds: &[(tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>)],
    x: f64,
    y: f64,
) -> Option<usize> {
    bounds.iter().position(|(position, size)| {
        let (left, top) = (f64::from(position.x), f64::from(position.y));
        (left..left + f64::from(size.width)).contains(&x)
            && (top..top + f64::from(size.height)).contains(&y)
    })
}

#[cfg(not(target_os = "macos"))]
fn physical_position(window: &tauri::WebviewWindow) -> tauri::Result<LauncherPosition> {
    let position = window.outer_position()?;
    Ok(LauncherPosition {
        x: position.x.into(),
        y: position.y.into(),
    })
}

/// Held while reading the clipboard. AppKit's pasteboard is not safe to
/// read from two threads at once, and Windows lets one thread open the
/// clipboard at a time; the capture thread and the widget pane both read.
#[cfg(not(target_os = "linux"))]
static CLIPBOARD_READ: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(not(target_os = "linux"))]
fn one_clipboard_reader() -> std::sync::MutexGuard<'static, ()> {
    CLIPBOARD_READ.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(not(target_os = "linux"))]
/// Read what the clipboard holds, in the order apps expect: copied files
/// first (Finder also offers their names as text), then text, then images.
/// Kinds the user did not opt into are skipped, not saved as text.
fn read_with_arboard(images: bool, files: bool) -> Option<Content> {
    let mut reader = arboard::Clipboard::new()
        .inspect_err(|error| tracing::warn!(%error, "Cannot read the clipboard"))
        .ok()?;
    if let Ok(paths) = reader.get().file_list()
        && !paths.is_empty()
    {
        return files.then_some(Content::Files(paths));
    }
    if let Ok(text) = reader.get_text() {
        return Some(Content::Text(text));
    }
    if !images {
        return None;
    }
    let buffer = clipboard_image()?;
    let (width, height) = buffer.dimensions();
    let mut png = Vec::new();
    buffer
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    (png.len() <= MAX_IMAGE_BYTES).then_some(Content::Image { png, width, height })
}

/// Start a GUI program and return at once; it keeps running on its own.
#[cfg(target_os = "linux")]
fn launch(program: &str) -> Result<()> {
    let mut child = std::process::Command::new(program)
        .spawn()
        .map_err(|error| Error::msg(format!("Could not open {program}: {error}")))?;
    // Reap it when it exits, so no zombie process stays behind.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Wait until the user lets go of the key that ran Sleep Displays, which
/// would wake the displays again.
fn wait_for_key_release() {
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Run a helper program with fixed arguments. Never pass user text here.
fn run(program: &str, args: &[&str]) -> Result<()> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .map_err(|error| Error::msg(format!("Could not run {program}: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    Err(Error::msg(format!("{program} failed: {}", detail.trim())))
}

#[cfg(test)]
mod tests {
    mod monitor_containing {
        use super::super::monitor_containing;
        use tauri::{PhysicalPosition, PhysicalSize};

        /// A laptop panel and a monitor to its right.
        fn two_monitors() -> [(PhysicalPosition<i32>, PhysicalSize<u32>); 2] {
            [
                (PhysicalPosition::new(0, 0), PhysicalSize::new(1920, 1080)),
                (
                    PhysicalPosition::new(1920, 0),
                    PhysicalSize::new(2560, 1440),
                ),
            ]
        }

        #[test]
        fn finds_the_second_monitor() {
            assert_eq!(monitor_containing(&two_monitors(), 2500.0, 700.0), Some(1));
        }

        #[test]
        fn a_point_on_no_monitor_has_none() {
            // A spot saved on a monitor that is now unplugged.
            assert_eq!(monitor_containing(&two_monitors()[..1], 2500.0, 1.0), None);
            assert_eq!(monitor_containing(&two_monitors(), 4480.0, 1.0), None);
            assert_eq!(monitor_containing(&two_monitors(), 100.0, 1200.0), None);
            assert_eq!(monitor_containing(&two_monitors(), -1.0, 0.0), None);
        }

        #[test]
        fn the_shared_edge_belongs_to_the_right_monitor() {
            assert_eq!(monitor_containing(&two_monitors(), 1919.5, 0.0), Some(0));
            assert_eq!(monitor_containing(&two_monitors(), 1920.0, 0.0), Some(1));
            assert_eq!(monitor_containing(&two_monitors(), 0.0, 0.0), Some(0));
            assert_eq!(monitor_containing(&two_monitors(), 0.0, 1080.0), None);
        }
    }

    mod centered {
        use super::super::centered;
        use tauri::{LogicalSize, PhysicalPosition, PhysicalSize};

        /// The launcher's logical size.
        const LAUNCHER: LogicalSize<f64> = LogicalSize::new(880.0, 560.0);

        #[test]
        fn centers_in_the_work_area() {
            let position = centered(
                PhysicalPosition::new(1920, 25),
                PhysicalSize::new(2560, 1415),
                LAUNCHER,
                1.0,
            );
            assert_eq!(position, PhysicalPosition::new(1920 + 840, 25 + 427));
        }

        #[test]
        fn counts_the_size_in_the_target_monitor_scale() {
            // Last shown on a 200% monitor, now opened on a 100% laptop.
            let on_laptop = centered(
                PhysicalPosition::new(0, 0),
                PhysicalSize::new(1920, 1080),
                LAUNCHER,
                1.0,
            );
            assert_eq!(on_laptop, PhysicalPosition::new(520, 260));
            // And the other way, onto a 200% monitor to its right.
            let on_4k = centered(
                PhysicalPosition::new(1920, 0),
                PhysicalSize::new(3840, 2160),
                LAUNCHER,
                2.0,
            );
            assert_eq!(on_4k, PhysicalPosition::new(1920 + 1040, 520));
        }

        #[test]
        fn a_window_larger_than_the_area_sits_at_its_corner() {
            let position = centered(
                PhysicalPosition::new(-800, 0),
                PhysicalSize::new(800, 600),
                LAUNCHER,
                1.0,
            );
            assert_eq!(position, PhysicalPosition::new(-800, 20));
        }
    }

    #[test]
    fn many_threads_can_read_the_clipboard_at_once() {
        // The capture thread and the widget pane read together; AppKit
        // crashes on unguarded parallel pasteboard reads.
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..20 {
                        super::clipboard_change();
                        super::read_clipboard(false, false);
                    }
                });
            }
        });
    }
}
