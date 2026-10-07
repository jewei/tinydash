use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use gio::prelude::*;

use super::{SECRET_FORMATS, run};
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, MAX_TEXT_BYTES},
        system::SystemCommand,
    },
};

pub const FILE_MANAGER: &str = "Files";
/// No folder is shown as a single file here.
pub const PACKAGE_EXTENSIONS: &[&str] = &[];
/// A .deb install cannot replace itself; new versions come from Releases.
pub const SELF_UPDATE: bool = false;
/// Only text: GTK reads it in the same request chain that checks for secrets.
pub const RICH_CLIPBOARD: bool = false;
pub const NATIVE_ICONS: bool = false;
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
    let home = std::env::home_dir().unwrap_or_default();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
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
            if let Some(executable) = desktop.executable().file_name() {
                aliases.push(executable.to_string_lossy().into_owned());
            }
            Some(App {
                name: desktop.display_name().to_string(),
                path: desktop.filename()?.to_str()?.to_owned(),
                aliases,
            })
        })
        .collect()
}

pub fn prepare_app(app: &mut tauri::App) {
    let _ = app;
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
        SystemCommand::LogOut | SystemCommand::OpenSystemSettings => unsupported(),
    }
}

/// Bumped on every owner change, before any request, so a callback can tell
/// that a newer owner replaced the one it asked.
static OWNER: AtomicU64 = AtomicU64::new(0);
/// Bumped after a checked read finishes; the monitor watches this counter.
static CHANGE: AtomicU64 = AtomicU64::new(0);
/// Text from the last checked owner, waiting for the monitor.
static CAPTURED: Mutex<Option<String>> = Mutex::new(None);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_can_be_read_again() {
        *CAPTURED.lock().unwrap() = Some("note".into());
        assert!(read_clipboard(false, false).is_some());
        assert!(read_clipboard(false, false).is_some());
    }
}
