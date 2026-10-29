use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::mpsc,
    thread::JoinHandle,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
    Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, GetDiskFreeSpaceExW},
    System::{
        DataExchange::{
            CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
            IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        },
        LibraryLoader::GetModuleHandleW,
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        Power::SetSuspendState,
        Registry::{HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW},
        Shutdown::LockWorkStation,
        Threading::GetCurrentThreadId,
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyState, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD,
            KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, ToUnicodeEx, VIRTUAL_KEY,
            VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME,
            VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT,
            VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN,
            VK_SCROLL, VK_SHIFT, VK_TAB, VK_UP,
        },
        Shell::{SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW},
        WindowsAndMessaging::{
            CallNextHookEx, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, HC_ACTION,
            HHOOK, HWND_BROADCAST, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PostMessageW,
            PostThreadMessageW, SC_MONITORPOWER, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
            SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
            WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_QUIT, WM_RBUTTONDOWN, WM_SETTINGCHANGE,
            WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_XBUTTONDOWN,
        },
    },
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, pixels_fit},
        expansion::{self, Key, Piece},
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
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(name) = app_name(path) else {
                continue;
            };
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

/// The name of a Start menu entry that launches an app, or `None` for other
/// files. Start menus also hold uninstallers; nobody launches those by name.
fn app_name(path: &Path) -> Option<String> {
    let is_app = path.extension().is_some_and(|ext| {
        ["lnk", "url", "exe", "appref-ms"]
            .iter()
            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
    });
    let name = path.file_stem()?.to_string_lossy().into_owned();
    (is_app && !name.to_lowercase().starts_with("uninstall")).then_some(name)
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
    Ok(super::Volume {
        name: drive_name(path),
        total_bytes,
        free_bytes,
    })
}

/// The drive part of `path`, such as `C:` or `\\server\share`, or "Disk"
/// when the path has none.
fn drive_name(path: &Path) -> String {
    match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => {
            prefix.as_os_str().to_string_lossy().into_owned()
        }
        _ => "Disk".into(),
    }
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
                if recycle_bin_emptied(result) {
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

/// Whether an `SHEmptyRecycleBinW` result means the bin is now empty. An
/// already empty bin reports E_UNEXPECTED; that is not a failure.
fn recycle_bin_emptied(result: i32) -> bool {
    result >= 0 || result == 0x8000_FFFF_u32 as i32
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
    let next = next_light_theme(status, light);
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

/// The `AppsUseLightTheme` value that switches the mode, from the status and
/// value of reading it: 1 (light) after dark, else 0 (dark). No value yet
/// means the default, light.
fn next_light_theme(read_status: u32, light: u32) -> u32 {
    u32::from(read_status == 0 && light == 0)
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

/// `ToUnicodeEx` flag: leave the keyboard state as it is, so reading a dead
/// key does not use it up before the app gets it (Windows 10 1607 and later).
const KEEP_KEYBOARD_STATE: u32 = 0x4;
/// Marks the input TinyDash sends ("TDSH").
const OWN_INPUT: usize = 0x5444_5348;
/// Keys sent in one `SendInput` call, with a pause after each call, so the
/// app in front keeps up.
const INPUT_BATCH: usize = 64;
const BATCH_GAP: Duration = Duration::from_millis(5);

pub fn typing_unavailable() -> Option<&'static str> {
    None
}

/// Windows asks for no permission to read or send keys.
pub fn typing_access(_ask: bool) -> Result<()> {
    Ok(())
}

/// Reads keys with low-level keyboard and mouse hooks, on a thread that
/// runs a message loop.
pub struct TypingWatch {
    thread_id: u32,
    thread: JoinHandle<()>,
}

impl TypingWatch {
    pub fn stop(self) {
        // SAFETY: The message carries no pointers, and the thread has a
        // message queue, as it set its hooks before it sent its ID.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if self.thread.join().is_err() {
            tracing::warn!("The key watch ended with a panic");
        }
    }
}

/// What the hooks work with, on the hook thread only.
struct Watcher {
    on_key: Box<dyn FnMut(Key)>,
    /// The window that got the latest key; a key in another starts afresh.
    foreground: HWND,
}

thread_local! {
    static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

pub fn watch_typing(on_key: impl FnMut(Key) + Send + 'static) -> Result<TypingWatch> {
    let (ready, started) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("key-watch".into())
        .spawn(move || run_hooks(Box::new(on_key), &ready))?;
    match started.recv() {
        Ok(Ok(thread_id)) => Ok(TypingWatch { thread_id, thread }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err(Error::msg("Could not start reading keys."))
        }
    }
}

fn run_hooks(on_key: Box<dyn FnMut(Key)>, ready: &mpsc::Sender<Result<u32>>) {
    WATCHER.set(Some(Watcher {
        on_key,
        foreground: std::ptr::null_mut(),
    }));
    // SAFETY: A null name asks for this program's own module.
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    let hook = |kind, procedure| {
        // SAFETY: The procedure has the hook type, and this thread removes
        // the hook before it ends.
        let hook = unsafe { SetWindowsHookExW(kind, procedure, module, 0) };
        if hook.is_null() {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(hook)
        }
    };
    let hooks = hook(WH_KEYBOARD_LL, Some(on_keyboard)).and_then(|keyboard| {
        hook(WH_MOUSE_LL, Some(on_mouse))
            .map(|mouse| [keyboard, mouse])
            .inspect_err(|_| unhook(keyboard))
    });
    match hooks {
        Ok(hooks) => {
            // SAFETY: Takes no arguments.
            if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_ok() {
                let mut message = MSG::default();
                // The hooks run while this thread waits for messages.
                // WM_QUIT ends the loop with 0, and an error with -1.
                // SAFETY: `message` is a valid MSG to fill.
                while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {}
            }
            hooks.into_iter().for_each(unhook);
        }
        Err(error) => {
            let _ = ready.send(Err(Error::msg(format!(
                "Windows did not let TinyDash read keys: {error}"
            ))));
        }
    }
    // Drops the typed text.
    WATCHER.set(None);
}

fn unhook(hook: HHOOK) {
    // SAFETY: This thread set the hook, and removes it once.
    unsafe { UnhookWindowsHookEx(hook) };
}

/// The keyboard hook. Windows waits for it before the key reaches the app,
/// so it only reads the key.
unsafe extern "system" fn on_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let message = u32::try_from(wparam).unwrap_or_default();
    if code == HC_ACTION as i32 && (message == WM_KEYDOWN || message == WM_SYSKEYDOWN) {
        // SAFETY: For HC_ACTION, `lparam` points to the key's
        // KBDLLHOOKSTRUCT, which is valid during the call.
        let key = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // Keys that a program sends, TinyDash's own included, are not typing.
        if key.flags & LLKHF_INJECTED == 0 {
            read_key(key.vkCode, key.scanCode, message == WM_SYSKEYDOWN);
        }
    }
    // SAFETY: Passes the hook's own arguments on, as every hook must.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// The mouse hook: a click may move the cursor.
unsafe extern "system" fn on_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let message = u32::try_from(wparam).unwrap_or_default();
    let click = matches!(
        message,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
    );
    if code == HC_ACTION as i32 && click {
        send(&[Key::Reset]);
    }
    // SAFETY: Passes the hook's own arguments on, as every hook must.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

fn send(keys: &[Key]) {
    WATCHER.with(|watcher| {
        if let Ok(mut watcher) = watcher.try_borrow_mut()
            && let Some(watcher) = watcher.as_mut()
        {
            for &key in keys {
                (watcher.on_key)(key);
            }
        }
    });
}

/// Whether the window in front changed since the latest key.
fn foreground_changed(foreground: HWND) -> bool {
    WATCHER.with(|watcher| {
        watcher.try_borrow_mut().is_ok_and(|mut watcher| {
            watcher.as_mut().is_some_and(|watcher| {
                std::mem::replace(&mut watcher.foreground, foreground) != foreground
            })
        })
    })
}

fn read_key(vk: u32, scan: u32, system_key: bool) {
    // A modifier alone types nothing.
    let Some(role) = VIRTUAL_KEY::try_from(vk).ok().and_then(key_role) else {
        return;
    };
    // SAFETY: Takes no arguments.
    let foreground = unsafe { GetForegroundWindow() };
    if foreground_changed(foreground) {
        send(&[Key::Reset]);
    }
    // A system key comes with Alt alone; AltGr holds Control and Alt.
    let shortcut = system_key
        || [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(is_down);
    let keys = match role {
        _ if shortcut => vec![Key::Reset],
        KeyRole::Backspace => vec![Key::Backspace],
        KeyRole::Reset => vec![Key::Reset],
        KeyRole::Text(vk) => expansion::keys_for(&translate(vk, scan, foreground)),
    };
    send(&keys);
}

/// What a key types in the keyboard layout of the window in front, with
/// Shift and Caps Lock as they are now. A dead key types nothing yet.
fn translate(vk: VIRTUAL_KEY, scan: u32, foreground: HWND) -> Vec<u16> {
    let mut state = [0u8; 256];
    if is_down(VK_SHIFT) {
        state[usize::from(VK_SHIFT)] = 0x80;
    }
    // SAFETY: Takes no pointers.
    if unsafe { GetKeyState(i32::from(VK_CAPITAL)) } & 1 != 0 {
        state[usize::from(VK_CAPITAL)] = 0x01;
    }
    // SAFETY: A null process ID pointer is allowed. A window that is gone
    // gives thread 0, which means this thread's layout.
    let layout =
        unsafe { GetKeyboardLayout(GetWindowThreadProcessId(foreground, std::ptr::null_mut())) };
    let mut units = [0u16; 8];
    // SAFETY: `state` has the 256 entries Windows reads, and `units` holds
    // the length passed.
    let count = unsafe {
        ToUnicodeEx(
            u32::from(vk),
            scan,
            state.as_ptr(),
            units.as_mut_ptr(),
            units.len() as i32,
            KEEP_KEYBOARD_STATE,
            layout,
        )
    };
    usize::try_from(count).map_or_else(
        |_| Vec::new(),
        |count| units[..count.min(units.len())].to_vec(),
    )
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    // SAFETY: Takes no pointers.
    let state = unsafe { GetAsyncKeyState(i32::from(vk)) };
    state < 0
}

/// What a key does to the typed text, before what it types is known.
#[derive(Debug, PartialEq, Eq)]
enum KeyRole {
    Backspace,
    /// Moves the cursor or ends the text, such as Return or an arrow.
    Reset,
    /// Types what the layout says, or nothing, such as a function key.
    Text(VIRTUAL_KEY),
}

/// `None` for a key that types nothing alone, such as Shift or Caps Lock.
fn key_role(vk: VIRTUAL_KEY) -> Option<KeyRole> {
    Some(match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU
        | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL => {
            return None;
        }
        VK_BACK => KeyRole::Backspace,
        VK_RETURN | VK_ESCAPE | VK_TAB | VK_LEFT | VK_UP | VK_RIGHT | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE | VK_INSERT => KeyRole::Reset,
        _ => KeyRole::Text(vk),
    })
}

/// One key to send: a virtual key, or a UTF-16 unit typed as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stroke {
    Key(VIRTUAL_KEY),
    Unit(u16),
}

/// The keys that delete `erase` characters and type `text`. A character
/// beyond U+FFFF is two units, a surrogate pair.
fn strokes(erase: usize, text: &str) -> Vec<Stroke> {
    let mut strokes = vec![Stroke::Key(VK_BACK); erase];
    for piece in expansion::pieces(text, usize::MAX) {
        match piece {
            Piece::Return => strokes.push(Stroke::Key(VK_RETURN)),
            Piece::Text(units) => strokes.extend(units.into_iter().map(Stroke::Unit)),
        }
    }
    strokes
}

/// A key press or release for `SendInput`, marked as TinyDash's own.
fn keyboard_input(stroke: Stroke, down: bool) -> KEYBDINPUT {
    let release = if down { 0 } else { KEYEVENTF_KEYUP };
    let (key, unit, unicode) = match stroke {
        Stroke::Key(key) => (key, 0, 0),
        Stroke::Unit(unit) => (0, unit, KEYEVENTF_UNICODE),
    };
    KEYBDINPUT {
        wVk: key,
        wScan: unit,
        dwFlags: unicode | release,
        time: 0,
        dwExtraInfo: OWN_INPUT,
    }
}

/// Types with `SendInput`, which puts the keys into the input stream as if
/// they were typed. Windows marks them injected, so the hook skips them.
/// Typing stops when another window comes to the front, so the rest never
/// lands there.
pub fn type_text(erase: usize, text: &str) -> Result<()> {
    let inputs: Vec<INPUT> = strokes(erase, text)
        .into_iter()
        .flat_map(|stroke| {
            [true, false].map(|down| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: keyboard_input(stroke, down),
                },
            })
        })
        .collect();
    // SAFETY: Takes no arguments.
    let foreground = unsafe { GetForegroundWindow() };
    for batch in inputs.chunks(INPUT_BATCH) {
        // SAFETY: Takes no arguments.
        if unsafe { GetForegroundWindow() } != foreground {
            return Err(Error::msg(
                "Stopped typing the snippet: another window came to the front.",
            ));
        }
        // SAFETY: The pointer and count describe `batch`, and the size is
        // INPUT's.
        let sent = unsafe {
            SendInput(
                batch.len() as u32,
                batch.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent as usize != batch.len() {
            return Err(Error::msg(format!(
                "Windows did not let TinyDash type the snippet: {}",
                std::io::Error::last_os_error()
            )));
        }
        std::thread::sleep(BATCH_GAP);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn sorts_keys_by_what_they_do_to_typed_text() {
        use super::{KeyRole, key_role};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            VK_BACK, VK_CAPITAL, VK_DELETE, VK_ESCAPE, VK_HOME, VK_LEFT, VK_LSHIFT, VK_NEXT,
            VK_NUMPAD1, VK_OEM_1, VK_RETURN, VK_RMENU, VK_SPACE, VK_TAB,
        };
        for vk in [VK_LSHIFT, VK_RMENU, VK_CAPITAL] {
            assert_eq!(key_role(vk), None, "{vk}");
        }
        assert_eq!(key_role(VK_BACK), Some(KeyRole::Backspace));
        for vk in [
            VK_RETURN, VK_ESCAPE, VK_TAB, VK_LEFT, VK_HOME, VK_NEXT, VK_DELETE,
        ] {
            assert_eq!(key_role(vk), Some(KeyRole::Reset), "{vk}");
        }
        // Letters, the ; key on US layouts, Space, and the number pad.
        for vk in [u16::from(b'S'), VK_OEM_1, VK_SPACE, VK_NUMPAD1] {
            assert_eq!(key_role(vk), Some(KeyRole::Text(vk)), "{vk}");
        }
    }

    #[test]
    fn types_backspaces_then_text_with_return_for_line_breaks() {
        use super::{Stroke, strokes};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_RETURN};
        assert_eq!(
            strokes(2, "a😀\r\nb"),
            [
                Stroke::Key(VK_BACK),
                Stroke::Key(VK_BACK),
                Stroke::Unit(u16::from(b'a')),
                Stroke::Unit(0xD83D),
                Stroke::Unit(0xDE00),
                Stroke::Key(VK_RETURN),
                Stroke::Unit(u16::from(b'b')),
            ]
        );
        assert!(strokes(0, "").is_empty());
    }

    #[test]
    fn builds_marked_key_presses_and_releases() {
        use super::{OWN_INPUT, Stroke, keyboard_input};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK,
        };
        let down = keyboard_input(Stroke::Unit(0xD83D), true);
        assert_eq!(
            (down.wVk, down.wScan, down.dwFlags, down.dwExtraInfo),
            (0, 0xD83D, KEYEVENTF_UNICODE, OWN_INPUT)
        );
        let up = keyboard_input(Stroke::Unit(0xD83D), false);
        assert_eq!(up.dwFlags, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
        let backspace = keyboard_input(Stroke::Key(VK_BACK), false);
        assert_eq!(
            (backspace.wVk, backspace.wScan, backspace.dwFlags),
            (VK_BACK, 0, KEYEVENTF_KEYUP)
        );
    }

    #[test]
    fn names_start_menu_apps_and_skips_other_files() {
        let name = |path: &str| super::app_name(Path::new(path));
        assert_eq!(name(r"C:\Start\Firefox.lnk").as_deref(), Some("Firefox"));
        assert_eq!(name(r"C:\Start\Tool.EXE").as_deref(), Some("Tool"));
        assert_eq!(name(r"C:\Start\Site.url").as_deref(), Some("Site"));
        assert_eq!(name(r"C:\Start\App.appref-ms").as_deref(), Some("App"));
        assert_eq!(name(r"C:\Start\Uninstall Firefox.lnk"), None);
        assert_eq!(name(r"C:\Start\UNINSTALL.exe"), None);
        assert_eq!(name(r"C:\Start\desktop.ini"), None);
        assert_eq!(name(r"C:\Start\Readme"), None);
    }

    #[test]
    fn names_the_drive_of_a_path() {
        assert_eq!(super::drive_name(Path::new(r"C:\Users\me")), "C:");
        assert_eq!(super::drive_name(Path::new(r"d:\")), "d:");
        assert_eq!(
            super::drive_name(Path::new(r"\\server\share\folder")),
            r"\\server\share"
        );
        assert_eq!(super::drive_name(Path::new(r"Users\me")), "Disk");
    }

    #[test]
    fn an_empty_recycle_bin_is_not_a_failure() {
        assert!(super::recycle_bin_emptied(0));
        assert!(super::recycle_bin_emptied(1));
        assert!(super::recycle_bin_emptied(0x8000_FFFF_u32 as i32));
        // E_ACCESSDENIED
        assert!(!super::recycle_bin_emptied(0x8007_0005_u32 as i32));
    }

    #[test]
    fn toggles_the_color_mode() {
        assert_eq!(super::next_light_theme(0, 1), 0);
        assert_eq!(super::next_light_theme(0, 0), 1);
        // ERROR_FILE_NOT_FOUND: no value yet means light, so dark comes next.
        assert_eq!(super::next_light_theme(2, 0), 0);
    }

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
