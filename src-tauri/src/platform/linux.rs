use std::{
    ffi::{CStr, CString, OsStr, c_char, c_int, c_uint, c_ulong},
    path::{Path, PathBuf},
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use gio::prelude::*;
use x11_dl::{
    xlib::{self, Display, Xlib},
    xrecord::{self, Xf86vmode as Xrecord},
};
use xkeysym::Keysym;

use super::{SECRET_FORMATS, run};
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, MAX_TEXT_BYTES},
        expansion::{self, Key, Piece},
        system::SystemCommand,
    },
};

pub const FILE_MANAGER: &str = "Files";
/// No folder is shown as a single file here.
pub const PACKAGE_EXTENSIONS: &[&str] = &[];
/// Installing a .deb update needs root (pkexec or sudo), so TinyDash does not
/// update itself here; new versions come from Releases.
pub const SELF_UPDATE: bool = false;
/// Only text: GTK reads it in the same request chain that checks for secrets.
pub const RICH_CLIPBOARD: bool = false;
pub const NATIVE_ICONS: bool = false;
pub const PHYSICAL_PLACEMENT: bool = true;
/// inotify needs one watch per folder from a shared per-user limit, so only
/// the top folders are watched; deeper changes show up at the next rescan,
/// when the launcher opens and the index is more than 15 minutes old.
pub const RECURSIVE_WATCH: bool = false;
pub const TEMPLATE_TRAY_ICON: bool = false;

const OWN_DESKTOP_IDS: &[&str] = &[
    "TinyDash.desktop",
    "tinydash.desktop",
    "dev.tinydash.launcher.desktop",
];

/// XDG application folders, including Flatpak and Snap exports listed in
/// `XDG_DATA_DIRS`.
pub fn app_folders() -> Vec<PathBuf> {
    xdg_app_folders(
        &std::env::home_dir().unwrap_or_default(),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var("XDG_DATA_DIRS").ok(),
    )
}

/// The user's data folder comes first, as XDG ranks it above system folders.
/// The XDG spec treats an empty variable as unset, so its default applies.
fn xdg_app_folders(
    home: &Path,
    data_home: Option<std::ffi::OsString>,
    data_dirs: Option<String>,
) -> Vec<PathBuf> {
    let data_home = data_home
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let data_dirs = data_dirs
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::iter::once(data_home)
        .chain(
            data_dirs
                .split(':')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        )
        .map(|dir| dir.join("applications"))
        .collect()
}

/// GIO applies XDG precedence, localization, `NoDisplay`, `OnlyShowIn`, and `TryExec`.
pub fn discover_apps() -> Vec<App> {
    gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.should_show())
        .filter_map(|app| {
            let desktop = app.downcast::<gio::DesktopAppInfo>().ok()?;
            if desktop
                .id()
                .is_some_and(|id| OWN_DESKTOP_IDS.contains(&id.as_str()))
            {
                return None;
            }
            let mut aliases: Vec<String> =
                desktop.keywords().iter().map(ToString::to_string).collect();
            aliases.extend(executable_alias(&desktop.executable()));
            // A generic name ("Web Browser") is short; a comment says more.
            let description = desktop
                .generic_name()
                .map(|name| name.to_string())
                .or_else(|| desktop.description().map(|comment| comment.to_string()))
                .filter(|words| !words.trim().is_empty());
            Some(App {
                name: desktop.display_name().to_string(),
                path: desktop.filename()?.to_str()?.to_owned(),
                aliases,
                description,
            })
        })
        .collect()
}

/// Programs that only start another one: Flatpak exports run
/// `flatpak run ...` and many Snap files run `env VAR=... /snap/bin/x`, so
/// their name would be the same alias for every such app.
const LAUNCHER_WRAPPERS: &[&str] = &["flatpak", "snap", "env", "sh", "bash"];

/// The file name of the first word of `Exec`, unless that is a wrapper.
fn executable_alias(executable: &Path) -> Option<String> {
    let name = executable.file_name()?.to_string_lossy();
    (!LAUNCHER_WRAPPERS.contains(&name.as_ref())).then(|| name.into_owned())
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

/// Only names that start with a dot are hidden here, and the scan skips
/// those on every OS, so it reads no metadata for this.
pub const HIDDEN: Option<fn(&std::fs::Metadata) -> bool> = None;

pub fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// Launch through GIO, which handles field codes, terminals, and D-Bus
/// activation. Desktop files are never run through a shell.
pub fn launch_app(path: &Path) -> Result<()> {
    let app = gio::DesktopAppInfo::from_filename(path)
        .ok_or_else(|| Error::msg("The application is no longer installed."))?;
    app.launch(&[], None::<&gio::AppLaunchContext>)
        .map_err(|error| Error::msg(error.to_string()))
}

pub fn app_icon(_path: &Path, _pixels: u32) -> Option<Vec<u8>> {
    None
}

/// Calls KDE's session manager directly: its command-line client is
/// `qdbus`, `qdbus6`, or `qdbus-qt6` depending on the distribution.
fn kde_log_out() -> Result<()> {
    let failed = |error: gio::glib::Error| Error::msg(format!("Could not log out: {error}"));
    gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        .map_err(failed)?
        .call_sync(
            Some("org.kde.Shutdown"),
            "/Shutdown",
            "org.kde.Shutdown",
            "logout",
            None,
            None,
            gio::DBusCallFlags::NONE,
            -1,
            None::<&gio::Cancellable>,
        )
        .map(drop)
        .map_err(failed)
}

/// The mount point that holds `path` (`/`, or `/home` on its own disk), and
/// the space an ordinary user may still fill there.
pub fn disk_space(path: &Path) -> Result<super::Volume> {
    use std::os::unix::fs::MetadataExt;
    let info = gio::File::for_path(path)
        .query_filesystem_info("filesystem::size,filesystem::free", gio::Cancellable::NONE)
        .map_err(|error| Error::msg(error.to_string()))?;
    // The mount point is the highest folder on the same device.
    let device = std::fs::metadata(path)?.dev();
    let mount = path
        .ancestors()
        .take_while(|folder| std::fs::metadata(folder).is_ok_and(|meta| meta.dev() == device))
        .last()
        .unwrap_or(path);
    Ok(super::Volume {
        name: mount.display().to_string(),
        total_bytes: info.attribute_uint64("filesystem::size"),
        free_bytes: info.attribute_uint64("filesystem::free"),
    })
}

/// No system file index here: desktops use different file indexes, none of them yet. Search uses TinyDash's own index.
pub fn find_files(_name: &str, _folder: &Path, _limit: usize) -> Vec<PathBuf> {
    Vec::new()
}

pub fn run_system_command(command: SystemCommand) -> Result<()> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let unsupported = || Err(Error::msg("This command is not supported on your desktop."));
    match command {
        SystemCommand::Lock => run("loginctl", &["lock-session"]),
        SystemCommand::Sleep => run("systemctl", &["suspend"]),
        SystemCommand::Restart => run("systemctl", &["reboot"]),
        SystemCommand::ShutDown => run("systemctl", &["poweroff"]),
        SystemCommand::EmptyTrash => run("gio", &["trash", "--empty"]),
        SystemCommand::LogOut if desktop.contains("gnome") => {
            run("gnome-session-quit", &["--logout", "--no-prompt"])
        }
        SystemCommand::LogOut if desktop.contains("kde") => kde_log_out(),
        SystemCommand::LogOut if desktop.contains("xfce") => {
            run("xfce4-session-logout", &["--logout"])
        }
        SystemCommand::OpenSystemSettings if desktop.contains("gnome") => {
            super::launch("gnome-control-center")
        }
        SystemCommand::OpenSystemSettings if desktop.contains("kde") => {
            super::launch("systemsettings")
        }
        SystemCommand::OpenSystemSettings if desktop.contains("xfce") => {
            super::launch("xfce4-settings-manager")
        }
        SystemCommand::OpenTrash => open_trash(),
        SystemCommand::SleepDisplays
            if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11") =>
        {
            super::wait_for_key_release();
            run("xset", &["dpms", "force", "off"])
        }
        SystemCommand::SleepDisplays if desktop.contains("kde") => {
            super::wait_for_key_release();
            run("kscreen-doctor", &["--dpms", "off"])
        }
        SystemCommand::ToggleDarkMode if desktop.contains("gnome") => toggle_gnome_dark_mode(),
        SystemCommand::LogOut
        | SystemCommand::OpenSystemSettings
        | SystemCommand::SleepDisplays
        | SystemCommand::ToggleDarkMode => unsupported(),
    }
}

/// Show the trash in the file manager. The output is discarded, not
/// captured: a file manager that GLib starts directly inherits it, and a
/// captured pipe would stay open until its window closes. `gio open` itself
/// exits once the launch is done.
fn open_trash() -> Result<()> {
    use std::process::Stdio;
    let status = std::process::Command::new("gio")
        .args(["open", "trash:///"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| Error::msg(format!("Could not run gio: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::msg(
            "No file manager could open the trash. Install one, such as Files.",
        ))
    }
}

/// Switch GNOME's color scheme between dark and the default, as its quick
/// settings do.
fn toggle_gnome_dark_mode() -> Result<()> {
    const SCHEMA: &str = "org.gnome.desktop.interface";
    let output = std::process::Command::new("gsettings")
        .args(["get", SCHEMA, "color-scheme"])
        .output()
        .map_err(|error| Error::msg(format!("Could not run gsettings: {error}")))?;
    if !output.status.success() {
        return Err(Error::msg(
            "This desktop has no dark mode setting that TinyDash can change.",
        ));
    }
    let dark = String::from_utf8_lossy(&output.stdout).contains("prefer-dark");
    let next = if dark { "default" } else { "prefer-dark" };
    run("gsettings", &["set", SCHEMA, "color-scheme", next])
}

/// Bumped on every owner change, before any request, so a callback can tell
/// that a newer owner replaced the one it asked.
static OWNER: AtomicU64 = AtomicU64::new(0);
/// Bumped after a checked read finishes; the monitor watches this counter.
static CHANGE: AtomicU64 = AtomicU64::new(0);
/// Text from the last checked owner, waiting for the monitor.
static CAPTURED: Mutex<Option<String>> = Mutex::new(None);

/// Copies that should stay out of histories are marked here, so there is
/// nothing to remember.
pub fn note_clipboard_change(_change: u64) {}

pub fn clipboard_change() -> u64 {
    CHANGE.load(Ordering::Acquire)
}

/// Text captured by the GTK handler for the latest change. Linux saves text
/// only: GTK reads it in the same request chain that checked for secrets.
/// The text is kept, not taken: if a newer capture lands during a read, the
/// monitor skips it and reads it again on the next tick.
pub fn read_clipboard(_images: bool, _files: bool) -> Option<Content> {
    CAPTURED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .map(Content::Text)
}

/// X11 has no change counter, so watch GTK owner changes. For each owner,
/// first check the offered formats for a password-manager marker, then read
/// the text, and drop both replies if a newer owner appeared meanwhile.
pub fn watch_clipboard(capturing: impl Fn() -> bool + 'static) {
    use gtk::prelude::*;
    let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    // gtk-rs 0.18 has no typed binding for this signal.
    clipboard.connect_local("owner-change", false, move |_| {
        let owner = OWNER.fetch_add(1, Ordering::AcqRel) + 1;
        if !capturing() {
            return None;
        }
        let current = move || OWNER.load(Ordering::Acquire) == owner;
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        let targets = gtk::gdk::Atom::intern("TARGETS");
        clipboard.request_contents(&targets, move |clipboard, selection| {
            let concealed = selection.targets().is_some_and(|targets| {
                targets
                    .iter()
                    .any(|target| SECRET_FORMATS.contains(&target.name().as_str()))
            });
            if !current() || concealed {
                return;
            }
            clipboard.request_text(move |_, text| {
                if current() {
                    let text = text.filter(|text| text.len() <= MAX_TEXT_BYTES);
                    *CAPTURED.lock().unwrap_or_else(|e| e.into_inner()) = text.map(str::to_owned);
                    CHANGE.fetch_add(1, Ordering::AcqRel);
                }
            });
        });
        None
    });
}

/// Ask GTK for the clipboard's text on the main thread, with the same
/// secret check as capture, and wait briefly for the reply. A newer owner
/// during the two requests drops both, so the text read is always the text
/// whose markers were checked.
pub fn clipboard_text(app: &tauri::AppHandle) -> Option<String> {
    let (reply, answer) = std::sync::mpsc::channel();
    let asked = app.run_on_main_thread(move || {
        let owner = OWNER.load(Ordering::Acquire);
        let current = move || OWNER.load(Ordering::Acquire) == owner;
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        let targets = gtk::gdk::Atom::intern("TARGETS");
        clipboard.request_contents(&targets, move |clipboard, selection| {
            let concealed = selection.targets().is_some_and(|targets| {
                targets
                    .iter()
                    .any(|target| SECRET_FORMATS.contains(&target.name().as_str()))
            });
            if concealed || !current() {
                reply.send(None).ok();
                return;
            }
            clipboard.request_text(move |_, text| {
                let text = text.filter(|_| current()).map(str::to_owned);
                reply.send(text).ok();
            });
        });
    });
    if let Err(error) = asked {
        tracing::debug!(%error, "Could not ask GTK for the clipboard");
        return None;
    }
    answer
        .recv_timeout(std::time::Duration::from_millis(500))
        .ok()
        .flatten()
}

/// Marks a copy so clipboard managers skip it (`x-kde-passwordManagerHint`).
pub fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    arboard::SetExtLinux::exclude_from_history(set)
}

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

pub fn prepare_launcher(_window: &tauri::WebviewWindow) {}

/// The window manager returns focus when the launcher hides.
pub fn remember_frontmost_app() {}

pub fn restore_frontmost_app() {}

/// X11 does not mark the keys a program sends, so the recording also gets
/// TinyDash's own typing. Keys count again this long after the X server
/// handled the last one.
const OWN_KEYS_AFTER: Duration = Duration::from_millis(300);
/// How long the recording thread waits for keys before it checks whether
/// to stop.
const POLL_MILLISECONDS: c_int = 200;
/// libxdo's pause between keys, in microseconds, as `xdotool type` uses.
const KEY_DELAY: c_uint = 12_000;
/// UTF-16 units typed between focus checks: under half a second.
const PIECE_UNITS: usize = 32;
/// libxdo's `CURRENTWINDOW`: type into the window that has focus.
const CURRENT_WINDOW: c_ulong = 0;
/// Modifiers that make a key a shortcut: Control, Alt (Mod1), and Super
/// (Mod4). AltGr (Mod5) types characters.
const SHORTCUT_MODIFIERS: c_uint = xlib::ControlMask | xlib::Mod1Mask | xlib::Mod4Mask;

/// libxdo's context.
#[repr(C)]
struct Xdo {
    _private: [u8; 0],
}

// libxdo 3, which the .deb and .rpm packages depend on.
#[link(name = "xdo")]
unsafe extern "C" {
    fn xdo_new_with_opened_display(
        display: *mut Display,
        name: *const c_char,
        close_display_when_freed: c_int,
    ) -> *mut Xdo;
    fn xdo_free(xdo: *mut Xdo);
    fn xdo_enter_text_window(
        xdo: *const Xdo,
        window: c_ulong,
        text: *const c_char,
        delay: c_uint,
    ) -> c_int;
    fn xdo_send_keysequence_window(
        xdo: *const Xdo,
        window: c_ulong,
        keys: *const c_char,
        delay: c_uint,
    ) -> c_int;
}

/// Until when keys come from TinyDash's own typing.
static OWN_TYPING_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn own_typing_until(until: Instant) {
    *OWN_TYPING_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) = Some(until);
}

fn own_typing() -> bool {
    OWN_TYPING_UNTIL
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|until| Instant::now() < until)
}

pub fn typing_unavailable() -> Option<&'static str> {
    wayland_session(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
    )
    .then_some("Not available on Wayland.")
}

/// Wayland lets no app read the keys typed in another app.
fn wayland_session(session_type: Option<&str>, wayland_display: Option<&OsStr>) -> bool {
    session_type.is_some_and(|kind| kind.eq_ignore_ascii_case("wayland"))
        || wayland_display.is_some_and(|display| !display.is_empty())
}

/// X11 asks for no permission to read or send keys.
pub fn typing_access(_ask: bool) -> Result<()> {
    Ok(())
}

/// Reads keys with the X RECORD extension, on a thread of its own.
pub struct TypingWatch {
    stopped: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl TypingWatch {
    /// The thread looks at `stopped` at least every `POLL_MILLISECONDS`.
    pub fn stop(self) {
        self.stopped.store(true, Ordering::Release);
        if self.thread.join().is_err() {
            tracing::warn!("The key watch ended with a panic");
        }
    }
}

pub fn watch_typing(on_key: impl FnMut(Key) + Send + 'static) -> Result<TypingWatch> {
    if let Some(reason) = typing_unavailable() {
        return Err(Error::msg(reason));
    }
    let (ready, started) = mpsc::channel();
    let stopped = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("key-watch".into())
        .spawn({
            let stopped = Arc::clone(&stopped);
            move || record_keys(Box::new(on_key), &stopped, &ready)
        })?;
    match started.recv() {
        Ok(Ok(())) => Ok(TypingWatch { stopped, thread }),
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

/// Two connections to the X server: RECORD sends keys on `data`, and
/// `control` runs RECORD and translates keys. Both close when dropped.
struct Connections<'a> {
    xlib: &'a Xlib,
    control: *mut Display,
    data: *mut Display,
}

impl<'a> Connections<'a> {
    fn open(xlib: &'a Xlib) -> Option<Self> {
        // SAFETY: A null name opens the display that DISPLAY names.
        let (control, data) = unsafe {
            (
                (xlib.XOpenDisplay)(ptr::null()),
                (xlib.XOpenDisplay)(ptr::null()),
            )
        };
        let connections = Self {
            xlib,
            control,
            data,
        };
        (!control.is_null() && !data.is_null()).then_some(connections)
    }
}

impl Drop for Connections<'_> {
    fn drop(&mut self) {
        // `data` first: closing it may still pass keys to the callback.
        for display in [self.data, self.control] {
            if !display.is_null() {
                // SAFETY: Opened in `open`, and closed once.
                unsafe { (self.xlib.XCloseDisplay)(display) };
            }
        }
    }
}

/// What the RECORD callback works with, on the recording thread only.
struct Recording<'a> {
    xlib: &'a Xlib,
    xrecord: &'a Xrecord,
    /// The connection that translates keys and asks for the focus.
    lookup: *mut Display,
    on_key: Box<dyn FnMut(Key)>,
    /// The window that had focus at the latest key.
    focus: xlib::Window,
}

fn record_keys(
    on_key: Box<dyn FnMut(Key)>,
    stopped: &AtomicBool,
    ready: &mpsc::Sender<Result<()>>,
) {
    let fail = |message: &str| {
        let _ = ready.send(Err(Error::msg(message)));
    };
    let (Ok(xlib), Ok(xrecord)) = (Xlib::open(), Xrecord::open()) else {
        return fail("Snippet expansion needs the libX11 and libXtst libraries.");
    };
    let Some(connections) = Connections::open(&xlib) else {
        return fail("Could not connect to the X server.");
    };
    let Some(context) = record_context(&xlib, &xrecord, connections.control) else {
        return fail("The X server did not let TinyDash read keys. It needs the RECORD extension.");
    };
    let recording = Box::into_raw(Box::new(Recording {
        xlib: &xlib,
        xrecord: &xrecord,
        lookup: connections.control,
        on_key,
        focus: 0,
    }));
    // SAFETY: `recording` stays valid until the data connection closes, and
    // only this thread uses it: RECORD calls back inside the calls on the
    // data connection below.
    let enabled = unsafe {
        (xrecord.XRecordEnableContextAsync)(
            connections.data,
            context,
            Some(on_record),
            recording.cast(),
        )
    };
    if enabled == 0 {
        fail("The X server did not let TinyDash read keys.");
    } else if ready.send(Ok(())).is_ok() {
        // SAFETY: The connection is open.
        let fd = unsafe { (xlib.XConnectionNumber)(connections.data) };
        while !stopped.load(Ordering::Acquire) {
            let mut wait = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: One valid pollfd.
            unsafe { libc::poll(&mut wait, 1, POLL_MILLISECONDS) };
            // SAFETY: The connection is open; replies call `on_record` here.
            unsafe { (xrecord.XRecordProcessReplies)(connections.data) };
            drain_events(&xlib, connections.data);
            drain_events(&xlib, connections.control);
        }
    }
    // SAFETY: The context belongs to the control connection, which is open.
    // While the context is on, the server reads nothing more from the data
    // connection, and closing that connection waits for a reply. XSync sends
    // the requests and waits until the server has handled them.
    unsafe {
        if enabled != 0 {
            (xrecord.XRecordDisableContext)(connections.control, context);
        }
        (xrecord.XRecordFreeContext)(connections.control, context);
        (xlib.XSync)(connections.control, xlib::False);
    }
    drop(connections);
    // SAFETY: Made by Box::into_raw above. With the data connection closed,
    // no callback can use it any more.
    drop(unsafe { Box::from_raw(recording) });
}

/// A RECORD context for key and mouse button presses from every client.
fn record_context(
    xlib: &Xlib,
    xrecord: &Xrecord,
    control: *mut Display,
) -> Option<xrecord::XRecordContext> {
    let (mut major, mut minor) = (0, 0);
    // SAFETY: The connection is open, and the call writes only the numbers.
    if unsafe { (xrecord.XRecordQueryVersion)(control, &mut major, &mut minor) } == 0 {
        return None;
    }
    // SAFETY: Returns a zeroed range, or null.
    let mut range = unsafe { (xrecord.XRecordAllocRange)() };
    if range.is_null() {
        return None;
    }
    let mut clients = xrecord::XRecordAllClients;
    // SAFETY: `range` is valid and writable. The server copies it, and it is
    // freed once. XSync makes the context known before the data connection
    // uses it.
    unsafe {
        (*range).device_events.first = xlib::KeyPress as u8;
        (*range).device_events.last = xlib::ButtonPress as u8;
        let context = (xrecord.XRecordCreateContext)(control, 0, &mut clients, 1, &mut range, 1);
        (xlib.XFree)(range.cast());
        (xlib.XSync)(control, xlib::False);
        (context != 0).then_some(context)
    }
}

/// RECORD's callback, with each recorded event in the X wire format.
unsafe extern "C" fn on_record(recording: *mut c_char, data: *mut xrecord::XRecordInterceptData) {
    // SAFETY: `recording` is the Recording that `record_keys` passed, used
    // on this thread only, and `data` stays valid until it is freed here.
    unsafe {
        let recording = &mut *recording.cast::<Recording>();
        let intercepted = &*data;
        // A device event is 32 bytes; `data_len` counts 4-byte units.
        if intercepted.category == xrecord::XRecordFromServer && intercepted.data_len >= 8 {
            let event = std::slice::from_raw_parts(intercepted.data, 32);
            // The type, the key code or button, and the modifiers before it.
            recording.event(
                event[0] & 0x7f,
                event[1],
                u16::from_ne_bytes([event[28], event[29]]),
            );
        }
        (recording.xrecord.XRecordFreeData)(data);
    }
}

impl Recording<'_> {
    fn event(&mut self, kind: u8, detail: u8, state: u16) {
        let keys = match c_int::from(kind) {
            xlib::KeyPress => self.key(detail, state),
            // Buttons 4 to 7 scroll, which moves no cursor.
            xlib::ButtonPress if !(4..=7).contains(&detail) => vec![Key::Reset],
            _ => return,
        };
        for key in keys {
            (self.on_key)(key);
        }
    }

    fn key(&mut self, keycode: u8, state: u16) -> Vec<Key> {
        // The user's keys among TinyDash's own are lost too.
        if own_typing() {
            return vec![Key::Reset];
        }
        let mut keys = Vec::new();
        let focus = self.focus();
        if std::mem::replace(&mut self.focus, focus) != focus {
            keys.push(Key::Reset);
        }
        let state = c_uint::from(state);
        keys.extend(keys_for(self.keysym(keycode, state), state));
        keys
    }

    fn focus(&self) -> xlib::Window {
        let (mut focus, mut revert) = (0, 0);
        // SAFETY: The connection is open and used by this thread only.
        unsafe { (self.xlib.XGetInputFocus)(self.lookup, &mut focus, &mut revert) };
        focus
    }

    /// What a key types with these modifiers, with Shift, Caps Lock, AltGr,
    /// and the layout group applied.
    fn keysym(&self, keycode: u8, state: c_uint) -> u32 {
        let mut keysym: xlib::KeySym = 0;
        let mut text: [c_char; 8] = [0; 8];
        // SAFETY: An all-zero XKeyEvent is valid. XLookupString reads it and
        // writes the keysym and at most `text.len()` bytes.
        unsafe {
            let mut event: xlib::XKeyEvent = std::mem::zeroed();
            event.type_ = xlib::KeyPress;
            event.display = self.lookup;
            event.keycode = c_uint::from(keycode);
            event.state = state;
            (self.xlib.XLookupString)(
                &mut event,
                text.as_mut_ptr(),
                text.len() as c_int,
                &mut keysym,
                ptr::null_mut(),
            );
        }
        u32::try_from(keysym).unwrap_or_default()
    }
}

/// Read the events that X sends every client, such as a new key map, so
/// they do not pile up. A new map also updates the one that reads keys.
fn drain_events(xlib: &Xlib, display: *mut Display) {
    // SAFETY: The connection is open and used by this thread only, and each
    // event is read into a valid XEvent.
    unsafe {
        while (xlib.XPending)(display) > 0 {
            let mut event: xlib::XEvent = std::mem::zeroed();
            (xlib.XNextEvent)(display, &mut event);
            if event.get_type() == xlib::MappingNotify {
                (xlib.XRefreshKeyboardMapping)(&mut event.mapping);
            }
        }
    }
}

/// The keys for a key press: none for a modifier alone, a reset for a
/// shortcut, Backspace, or what the key typed. A key that types no
/// character, such as an arrow or a dead key, resets.
fn keys_for(keysym: u32, state: c_uint) -> Vec<Key> {
    let keysym = Keysym::new(keysym);
    if keysym.is_modifier_key() {
        Vec::new()
    } else if state & SHORTCUT_MODIFIERS != 0 {
        vec![Key::Reset]
    } else if keysym == Keysym::BackSpace {
        vec![Key::Backspace]
    } else {
        keysym.key_char().map_or_else(
            || vec![Key::Reset],
            |c| expansion::keys_for(c.encode_utf16(&mut [0; 2])),
        )
    }
}

/// Types with libxdo, which sends keys with the XTEST extension and maps a
/// character that no key types to a spare key for a moment. Typing stops
/// when another window gets the focus, so the rest never lands there.
pub fn type_text(erase: usize, text: &str) -> Result<()> {
    let xlib =
        Xlib::open().map_err(|_| Error::msg("Snippet expansion needs the libX11 library."))?;
    // SAFETY: A null name opens the display that DISPLAY names.
    let display = unsafe { (xlib.XOpenDisplay)(ptr::null()) };
    if display.is_null() {
        return Err(Error::msg("Could not connect to the X server."));
    }
    own_typing_until(Instant::now() + Duration::from_secs(60 * 60));
    let focus = || {
        let (mut focus, mut revert) = (0, 0);
        // SAFETY: The connection is open, and only this thread uses it.
        unsafe { (xlib.XGetInputFocus)(display, &mut focus, &mut revert) };
        focus
    };
    let first = focus();
    let typed = type_with_xdo(display, erase, text, || focus() == first);
    // SAFETY: The connection is open. XSync returns once the server handled
    // every key, so the recording has them before the quiet time ends.
    unsafe { (xlib.XSync)(display, xlib::False) };
    own_typing_until(Instant::now() + OWN_KEYS_AFTER);
    // SAFETY: Opened above and closed once; libxdo leaves it open.
    unsafe { (xlib.XCloseDisplay)(display) };
    typed
}

fn type_with_xdo(
    display: *mut Display,
    erase: usize,
    text: &str,
    still_in_front: impl Fn() -> bool,
) -> Result<()> {
    // SAFETY: The connection is open; 0 tells libxdo to leave it open.
    let xdo = unsafe { xdo_new_with_opened_display(display, ptr::null(), 0) };
    if xdo.is_null() {
        return Err(Error::msg(
            "Could not type the snippet: libxdo did not start.",
        ));
    }
    let typed = type_pieces(xdo, erase, text, still_in_front);
    // SAFETY: Made above and freed once.
    unsafe { xdo_free(xdo) };
    typed
}

fn type_pieces(
    xdo: *mut Xdo,
    erase: usize,
    text: &str,
    still_in_front: impl Fn() -> bool,
) -> Result<()> {
    let failed = || Error::msg("Could not type the snippet.");
    let press = |keys: &CStr| {
        // SAFETY: `xdo` is valid until its caller frees it, and `keys` is a
        // C string.
        let status =
            unsafe { xdo_send_keysequence_window(xdo, CURRENT_WINDOW, keys.as_ptr(), KEY_DELAY) };
        if status == 0 { Ok(()) } else { Err(failed()) }
    };
    // `None` is a Backspace. Short pieces let typing stop soon.
    let steps = std::iter::repeat_with(|| None)
        .take(erase)
        .chain(expansion::pieces(text, PIECE_UNITS).into_iter().map(Some));
    for step in steps {
        if !still_in_front() {
            return Err(Error::msg(
                "Stopped typing the snippet: another window came to the front.",
            ));
        }
        match step {
            None => press(c"BackSpace")?,
            Some(Piece::Return) => press(c"Return")?,
            Some(Piece::Text(units)) => {
                // `pieces` leaves NUL out, so this does not fail.
                let text = CString::new(String::from_utf16_lossy(&units)).map_err(|_| failed())?;
                // SAFETY: As above; `text` is a C string.
                let status =
                    unsafe { xdo_enter_text_window(xdo, CURRENT_WINDOW, text.as_ptr(), KEY_DELAY) };
                if status != 0 {
                    return Err(failed());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_a_wayland_session() {
        assert!(wayland_session(Some("wayland"), None));
        assert!(wayland_session(Some("x11"), Some(OsStr::new("wayland-0"))));
        assert!(!wayland_session(Some("x11"), None));
        assert!(!wayland_session(None, Some(OsStr::new(""))));
        assert!(!wayland_session(Some("tty"), None));
    }

    #[test]
    fn reads_key_presses() {
        use x11_dl::xlib::{ControlMask, Mod1Mask, Mod2Mask, Mod4Mask, Mod5Mask, ShiftMask};
        use xkeysym::key;
        assert_eq!(keys_for(key::s, 0), [Key::Char('s')]);
        assert_eq!(keys_for(key::S, ShiftMask), [Key::Char('S')]);
        assert_eq!(keys_for(key::semicolon, 0), [Key::Char(';')]);
        assert_eq!(keys_for(key::space, 0), [Key::Char(' ')]);
        assert_eq!(keys_for(key::Cyrillic_a, 0), [Key::Char('а')]);
        // Number pad digits with Num Lock (Mod2), and AltGr (Mod5) signs.
        assert_eq!(keys_for(key::KP_1, Mod2Mask), [Key::Char('1')]);
        assert_eq!(keys_for(key::at, Mod5Mask), [Key::Char('@')]);
        assert_eq!(keys_for(key::BackSpace, 0), [Key::Backspace]);
        for modifier in [
            key::Shift_L,
            key::Control_R,
            key::Caps_Lock,
            key::ISO_Level3_Shift,
        ] {
            assert!(keys_for(modifier, 0).is_empty(), "{modifier:#x}");
        }
        for shortcut in [ControlMask, Mod1Mask, Mod4Mask] {
            assert_eq!(keys_for(key::s, shortcut), [Key::Reset]);
            assert_eq!(keys_for(key::BackSpace, shortcut), [Key::Reset]);
        }
        for keysym in [
            key::Return,
            key::KP_Enter,
            key::Tab,
            key::Escape,
            key::Left,
            key::Home,
            key::Next,
            key::Delete,
            key::F5,
            key::dead_acute,
            0,
        ] {
            assert_eq!(keys_for(keysym, 0), [Key::Reset], "{keysym:#x}");
        }
    }

    #[test]
    fn a_capture_can_be_read_again() {
        *CAPTURED.lock().unwrap() = Some("note".into());
        assert!(read_clipboard(false, false).is_some());
        assert!(read_clipboard(false, false).is_some());
    }

    #[test]
    fn empty_xdg_variables_use_the_defaults() {
        let home = Path::new("/home/me");
        let defaults = vec![
            PathBuf::from("/home/me/.local/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            PathBuf::from("/usr/share/applications"),
        ];
        assert_eq!(xdg_app_folders(home, None, None), defaults);
        assert_eq!(
            xdg_app_folders(home, Some("".into()), Some(String::new())),
            defaults
        );
        assert_eq!(
            xdg_app_folders(home, Some("/data".into()), Some("/opt/share:".into())),
            vec![
                PathBuf::from("/data/applications"),
                PathBuf::from("/opt/share/applications"),
            ]
        );
    }

    #[test]
    fn launcher_wrappers_are_not_aliases() {
        assert_eq!(executable_alias(Path::new("/usr/bin/flatpak")), None);
        assert_eq!(executable_alias(Path::new("env")), None);
        assert_eq!(executable_alias(Path::new("/usr/bin/snap")), None);
        assert_eq!(
            executable_alias(Path::new("/usr/bin/firefox")).as_deref(),
            Some("firefox")
        );
        assert_eq!(executable_alias(Path::new("")), None);
    }

    #[test]
    fn app_folders_include_flatpak_and_snap_exports() {
        assert_eq!(
            xdg_app_folders(
                Path::new("/home/ann"),
                Some("/data".into()),
                Some("/var/lib/flatpak/exports/share::/var/lib/snapd/desktop:".into()),
            ),
            [
                PathBuf::from("/data/applications"),
                PathBuf::from("/var/lib/flatpak/exports/share/applications"),
                PathBuf::from("/var/lib/snapd/desktop/applications"),
            ]
        );
    }
}
