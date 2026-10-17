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
        Registry::{HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW},
        Shutdown::LockWorkStation,
    },
    UI::{
        Shell::{SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW},
        WindowsAndMessaging::{
            HWND_BROADCAST, PostMessageW, SC_MONITORPOWER, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
            WM_SETTINGCHANGE, WM_SYSCOMMAND,
        },
    },
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, pixels_fit},
        system::SystemCommand,
    },
};

pub const FILE_MANAGER: &str = "File Explorer";
/// No folder is shown as a single file here.
pub const PACKAGE_EXTENSIONS: &[&str] = &[];
pub const SELF_UPDATE: bool = true;
pub const RICH_CLIPBOARD: bool = true;
pub const NATIVE_ICONS: bool = false;
pub const PHYSICAL_PLACEMENT: bool = true;
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

pub fn icon_stamp(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
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

/// No system file index here: Windows Search is not used yet. Search uses TinyDash's own index.
pub fn find_files(_name: &str, _folder: &Path, _limit: usize) -> Vec<PathBuf> {
    Vec::new()
}

pub fn run_system_command(command: SystemCommand) -> Result<()> {
    let failed = |what: &str| {
        Error::msg(format!(
            "Windows could not {what}: {}",
            std::io::Error::last_os_error()
        ))
    };
    // SAFETY: These Win32 calls take no pointers except the documented null
    // window handle and root path, meaning "no owner" and "all drives", and
    // the documented HWND_BROADCAST handle, meaning "every top-level window".
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
            // Posted, not sent: sending to every window could wait on a hung one.
            // 2 means "off" for SC_MONITORPOWER.
            SystemCommand::SleepDisplays => {
                super::wait_for_key_release();
                if PostMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER as usize, 2) == 0 {
                    Err(failed("turn off the displays"))
                } else {
                    Ok(())
                }
            }
            SystemCommand::ToggleDarkMode => toggle_dark_mode(),
            // Explorer's exit code says nothing about success, so it is not awaited.
            SystemCommand::OpenTrash => std::process::Command::new("explorer.exe")
                .arg("shell:RecycleBinFolder")
                .spawn()
                .map(drop)
                .map_err(|error| Error::msg(format!("Could not open the Recycle Bin: {error}"))),
        }
    }
}

/// Switch apps and the taskbar between light and dark, as Settings >
/// Personalization > Colors does. A custom mix of the two becomes one mode,
/// the opposite of the apps' mode.
fn toggle_dark_mode() -> Result<()> {
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let failed = |status: u32| {
        Error::msg(format!(
            "Windows could not change the color mode: {}",
            std::io::Error::from_raw_os_error(status as i32)
        ))
    };
    const DWORD_BYTES: u32 = 4;
    let mut light: u32 = 1;
    let mut size = DWORD_BYTES;
    let apps = wide("AppsUseLightTheme");
    // SAFETY: The strings are NUL-terminated and outlive the call, and
    // `light` has room for the `size` bytes of the DWORD asked for.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            apps.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut light).cast(),
            &raw mut size,
        )
    };
    // No value yet means the default, light.
    let next = u32::from(status == 0 && light == 0);
    for name in ["AppsUseLightTheme", "SystemUsesLightTheme"] {
        let name = wide(name);
        // SAFETY: The strings are NUL-terminated, and `next` holds the four
        // bytes of the DWORD written.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_DWORD,
                (&raw const next).cast(),
                DWORD_BYTES,
            )
        };
        if status != 0 {
            return Err(failed(status));
        }
    }
    // Windows open now repaint in the new colors, as after Settings. Each
    // busy window may take the whole timeout, so the action does not wait.
    std::thread::spawn(move || {
        let area = wide("ImmersiveColorSet");
        // SAFETY: `area` is NUL-terminated and outlives the call; no result
        // is read.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                area.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                1000,
                std::ptr::null_mut(),
            )
        };
    });
    Ok(())
}

/// Copies that should stay out of histories are marked here, so there is
/// nothing to remember.
pub fn note_clipboard_change(_change: u64) {}

pub fn clipboard_change() -> u64 {
    // SAFETY: Takes no arguments and only reads a counter.
    u64::from(unsafe { GetClipboardSequenceNumber() })
}

/// How many times the secret check tries to open a busy clipboard.
const OPEN_ATTEMPTS: u32 = 5;

/// The ID of a registered clipboard format, or 0 when Windows has none.
fn clipboard_format(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
    unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
}

/// The source asked history tools to skip this copy, either with a marker
/// format or with `CanIncludeInClipboardHistory` set to 0.
fn clipboard_is_concealed() -> bool {
    // SAFETY: Format IDs come from RegisterClipboardFormatW; 0 is never available.
    let available = |id: u32| id != 0 && unsafe { IsClipboardFormatAvailable(id) } != 0;
    if SECRET_FORMATS
        .iter()
        .any(|name| available(clipboard_format(name)))
    {
        return true;
    }
    let history = clipboard_format("CanIncludeInClipboardHistory");
    if !available(history) {
        return false;
    }
    // Another reader, such as Windows clipboard history, often holds the
    // clipboard for a moment after a copy; arboard waits for it the same way.
    let opened = (0..OPEN_ATTEMPTS).any(|attempt| {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // SAFETY: A null owner window is allowed; no pointer must stay valid.
        (unsafe { OpenClipboard(std::ptr::null_mut()) }) != 0
    });
    if !opened {
        // Still busy: treat as secret rather than risk saving a password.
        return true;
    }
    // SAFETY: The clipboard stays open until CloseClipboard, and the global
    // memory stays locked while the DWORD is read.
    unsafe {
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
    let before = clipboard_change();
    if clipboard_is_concealed() {
        return None;
    }
    // A copy that replaced the checked one during the read was not checked.
    super::read_with_arboard(images, files).filter(|_| clipboard_change() == before)
}

/// The standard format for a bitmap with a `BITMAPV5HEADER`.
const CF_DIBV5: u32 = 17;

/// arboard decodes the whole image, so a huge one is skipped, from its
/// header, before that. An image whose header cannot be read is skipped too:
/// arboard reads only PNG and `CF_DIBV5`, and Windows makes `CF_DIBV5` from
/// any bitmap, so it would find no image either.
pub fn clipboard_image() -> Option<image::RgbaImage> {
    let (width, height) = clipboard_image_size()?;
    if !pixels_fit(width, height) {
        return None;
    }
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    let (width, height) = (
        u32::try_from(image.width).ok()?,
        u32::try_from(image.height).ok()?,
    );
    if !pixels_fit(width, height) {
        return None;
    }
    image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
}

/// Reads the size from the header of the format arboard decodes: PNG when
/// the source offers it, else `CF_DIBV5`.
fn clipboard_image_size() -> Option<(u32, u32)> {
    let png = clipboard_format("PNG");
    // SAFETY: The clipboard stays open until CloseClipboard. Format IDs come
    // from RegisterClipboardFormatW or are standard; 0 is never available.
    unsafe {
        if !open_clipboard() {
            return None;
        }
        let size = if png != 0 && IsClipboardFormatAvailable(png) != 0 {
            read_clipboard_bytes(png, png_size)
        } else {
            read_clipboard_bytes(CF_DIBV5, dib_size)
        };
        CloseClipboard();
        size
    }
}

/// Opens the clipboard, and tries again for a short time while another app
/// holds it, as arboard does. The caller must call `CloseClipboard` after a
/// success.
fn open_clipboard() -> bool {
    const ATTEMPTS: usize = 6;
    for attempt in 1..=ATTEMPTS {
        // SAFETY: A null owner opens the clipboard for this thread.
        if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
            return true;
        }
        if attempt < ATTEMPTS {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    false
}

/// Pass the bytes of one clipboard format to `read` in place, without a copy.
///
/// # Safety
///
/// The caller must have the clipboard open.
unsafe fn read_clipboard_bytes(
    format: u32,
    read: impl FnOnce(&[u8]) -> Option<(u32, u32)>,
) -> Option<(u32, u32)> {
    // SAFETY: The clipboard is open, so the handle stays valid, and the
    // memory stays locked, at its full GlobalSize, while `read` runs.
    unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return None;
        }
        let pointer = GlobalLock(handle);
        if pointer.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), GlobalSize(handle));
        let size = read(bytes);
        GlobalUnlock(handle);
        size
    }
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png)
        .into_dimensions()
        .ok()
}

/// The size in a `BITMAPINFOHEADER` or a later version. The height is
/// negative when the rows run from the top down.
fn dib_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let field = |at: usize| Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    // The older BITMAPCOREHEADER is 12 bytes, with 16-bit sizes.
    if field(0)? < 40 {
        return None;
    }
    Some((field(4)?.unsigned_abs(), field(8)?.unsigned_abs()))
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

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_png_size_from_its_header() {
        let mut png = Vec::new();
        image::RgbaImage::new(3, 2)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(super::png_size(&png), Some((3, 2)));
        assert_eq!(super::png_size(&png[..8]), None);
    }

    #[test]
    fn reads_the_bitmap_size_from_its_header() {
        let mut header = [0u8; 124];
        header[0..4].copy_from_slice(&124i32.to_le_bytes());
        header[4..8].copy_from_slice(&12_000i32.to_le_bytes());
        header[8..12].copy_from_slice(&(-9_000i32).to_le_bytes());
        assert_eq!(super::dib_size(&header), Some((12_000, 9_000)));
        assert_eq!(super::dib_size(&header[..8]), None);
        header[0..4].copy_from_slice(&12i32.to_le_bytes());
        assert_eq!(super::dib_size(&header), None);
    }
}
