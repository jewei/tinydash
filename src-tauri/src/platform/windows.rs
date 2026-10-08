use std::path::{Path, PathBuf};

use windows_sys::Win32::{
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, GetDiskFreeSpaceExW},
    System::{
        DataExchange::{
            CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
            IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        },
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        Power::SetSuspendState,
        Shutdown::LockWorkStation,
    },
    UI::Shell::{SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW},
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{apps::App, clipboard::Content, system::SystemCommand},
};

pub const FILE_MANAGER: &str = "File Explorer";
/// No folder is shown as a single file here.
pub const PACKAGE_EXTENSIONS: &[&str] = &[];
pub const SELF_UPDATE: bool = true;
pub const RICH_CLIPBOARD: bool = true;
pub const NATIVE_ICONS: bool = false;
/// ReadDirectoryChangesW watches a whole tree with one handle.
pub const RECURSIVE_WATCH: bool = true;
pub const TEMPLATE_TRAY_ICON: bool = false;

/// Shortcuts are found up to this many folders deep.
const APP_DEPTH: usize = 6;

/// The per-user and shared Start menu program folders.
pub fn app_folders() -> Vec<PathBuf> {
    ["APPDATA", "PROGRAMDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

pub fn discover_apps() -> Vec<App> {
    let mut apps = Vec::new();
    for folder in app_folders() {
        let walker = walkdir::WalkDir::new(folder)
            .max_depth(APP_DEPTH)
            .into_iter();
        for entry in walker.flatten() {
            let path = entry.path();
            let is_app = path.extension().is_some_and(|ext| {
                ["lnk", "url", "exe", "appref-ms"]
                    .iter()
                    .any(|allowed| ext.eq_ignore_ascii_case(allowed))
            });
            let Some(name) = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
            else {
                continue;
            };
            // Start menus also hold uninstallers; nobody launches those by name.
            if !is_app
                || !entry.file_type().is_file()
                || name.to_lowercase().starts_with("uninstall")
            {
                continue;
            }
            if let Some(path) = path.to_str() {
                // Reading a shortcut's comment needs COM; the folder shows instead.
                apps.push(App {
                    name,
                    path: path.to_owned(),
                    aliases: Vec::new(),
                    description: None,
                });
            }
        }
    }
    apps
}

pub fn prepare_app(app: &mut tauri::App) {
    if let Err(error) = app.handle().plugin(tauri_plugin_notification::init()) {
        tracing::warn!(%error, "Notifications are unavailable");
    }
}

pub fn notify(app: &tauri::AppHandle, title: &str, body: &str) -> Result<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| Error::msg(error.to_string()))
}

/// Opening the app again starts a second process, which the single-instance
/// plugin hands over, so no event needs handling.
pub fn is_reopen(_: &tauri::RunEvent) -> bool {
    false
}

/// Explorer hides files and folders with the hidden attribute, such as the
/// `desktop.ini` in each user folder and `AppData`. The system attribute
/// alone does not hide an item.
pub const HIDDEN: Option<fn(&std::fs::Metadata) -> bool> = Some(has_hidden_attribute);

fn has_hidden_attribute(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

pub fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    // Files in the user's local AppData folder are private to the user by
    // default, and a roaming profile does not copy them.
    let _ = path;
    Ok(())
}

pub fn launch_app(path: &Path) -> Result<()> {
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|error| Error::msg(error.to_string()))
}

pub fn app_icon(_path: &Path, _pixels: u32) -> Option<Vec<u8>> {
    None
}

/// The drive of `path` (`C:`) and the space the user may still fill there,
/// which honors disk quotas.
pub fn disk_space(path: &Path) -> Result<super::Volume> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let (mut free_bytes, mut total_bytes) = (0u64, 0u64);
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call,
    // the two out pointers are valid, and the third may be null.
    let done = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_bytes,
            &mut total_bytes,
            std::ptr::null_mut(),
        )
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let name = match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => {
            prefix.as_os_str().to_string_lossy().into_owned()
        }
        _ => "Disk".into(),
    };
    Ok(super::Volume {
        name,
        total_bytes,
        free_bytes,
    })
}

pub fn run_system_command(command: SystemCommand) -> Result<()> {
    let failed = |what: &str| {
        Error::msg(format!(
            "Windows could not {what}: {}",
            std::io::Error::last_os_error()
        ))
    };
    // SAFETY: These Win32 calls take no pointers except the documented null
    // window handle and root path, meaning "no owner" and "all drives".
    unsafe {
        match command {
            SystemCommand::Lock if LockWorkStation() == 0 => Err(failed("lock the screen")),
            SystemCommand::Sleep if !SetSuspendState(false, false, false) => Err(failed("sleep")),
            SystemCommand::Lock | SystemCommand::Sleep => Ok(()),
            SystemCommand::EmptyTrash => {
                let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
                let result = SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), flags);
                // An already empty bin reports E_UNEXPECTED; that is not a failure.
                if result >= 0 || result == 0x8000_FFFF_u32 as i32 {
                    Ok(())
                } else {
                    let error = std::io::Error::from_raw_os_error(result);
                    Err(Error::msg(format!(
                        "Windows could not empty the Recycle Bin: {error}"
                    )))
                }
            }
            SystemCommand::Restart => super::run("shutdown", &["/r", "/t", "0"]),
            SystemCommand::ShutDown => super::run("shutdown", &["/s", "/t", "0"]),
            SystemCommand::LogOut => super::run("shutdown", &["/l"]),
            SystemCommand::OpenSystemSettings => {
                tauri_plugin_opener::open_url("ms-settings:", None::<&str>)
                    .map_err(|error| Error::msg(error.to_string()))
            }
        }
    }
}

pub fn clipboard_change() -> u64 {
    // SAFETY: Takes no arguments and only reads a counter.
    u64::from(unsafe { GetClipboardSequenceNumber() })
}

/// The source asked history tools to skip this copy, either with a marker
/// format or with `CanIncludeInClipboardHistory` set to 0.
fn clipboard_is_concealed() -> bool {
    let format = |name: &str| {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
        unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
    };
    // SAFETY: Format IDs come from RegisterClipboardFormatW; 0 is never available.
    let available = |id: u32| id != 0 && unsafe { IsClipboardFormatAvailable(id) } != 0;
    if SECRET_FORMATS.iter().any(|name| available(format(name))) {
        return true;
    }
    let history = format("CanIncludeInClipboardHistory");
    if !available(history) {
        return false;
    }
    // SAFETY: The clipboard stays open until CloseClipboard, and the global
    // memory stays locked while the DWORD is read.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            // Busy: treat as secret rather than risk saving a password.
            return true;
        }
        let handle = GetClipboardData(history);
        let mut excluded = false;
        if !handle.is_null() && GlobalSize(handle) >= 4 {
            let pointer = GlobalLock(handle);
            if !pointer.is_null() {
                excluded = pointer.cast::<u32>().read_unaligned() == 0;
                GlobalUnlock(handle);
            }
        }
        CloseClipboard();
        excluded
    }
}

/// The clipboard content for the latest change, or `None` when its source
/// marked it secret.
pub fn read_clipboard(images: bool, files: bool) -> Option<Content> {
    let _one_reader = super::one_clipboard_reader();
    if clipboard_is_concealed() {
        return None;
    }
    super::read_with_arboard(images, files)
}

pub fn clipboard_text(_app: &tauri::AppHandle) -> Option<String> {
    match read_clipboard(false, false)? {
        Content::Text(text) => Some(text),
        Content::Image { .. } | Content::Files(_) => None,
    }
}

/// Marks a copy so clipboard managers skip it
/// (`ExcludeClipboardContentFromMonitorProcessing`), and so Windows keeps it
/// out of its own history and cloud clipboard.
pub fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    use arboard::SetExtWindows;
    set.exclude_from_monitoring()
        .exclude_from_history()
        .exclude_from_cloud()
}

/// Windows needs no change notifications: `clipboard_change` is a cheap counter.
pub fn watch_clipboard(_capturing: impl Fn() -> bool + 'static) {}

pub fn place_launcher(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    saved: Option<crate::settings::LauncherPosition>,
) -> tauri::Result<()> {
    super::place_in_physical_pixels(app, window, saved)
}

pub fn launcher_position(
    window: &tauri::WebviewWindow,
) -> tauri::Result<crate::settings::LauncherPosition> {
    super::physical_position(window)
}

/// Ask Windows 11 for rounded corners on the borderless launcher.
pub fn prepare_launcher(window: &tauri::WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let preference = DWMWCP_ROUND;
    // SAFETY: The handle belongs to a live window, and the attribute value is a
    // DWM_WINDOW_CORNER_PREFERENCE that outlives the call. Older Windows
    // versions reject the attribute, which is harmless.
    unsafe {
        DwmSetWindowAttribute(
            hwnd.0,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            std::ptr::from_ref(&preference).cast(),
            size_of_val(&preference) as u32,
        );
    }
}

/// Windows activates the previous window when the launcher hides.
pub fn remember_frontmost_app() {}

pub fn restore_frontmost_app() {}
