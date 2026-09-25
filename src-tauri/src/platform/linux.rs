use gio::prelude::*;
use gio_unix::DesktopAppInfo;

use crate::{
    error::{Error, Result},
    providers::apps::AppEntry,
};

pub fn discover_apps() -> Result<Vec<AppEntry>> {
    // GIO handles XDG precedence, localization, Hidden/NoDisplay, OnlyShowIn,
    // TryExec, and desktop entries exported by Flatpak and Snap.
    Ok(gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.should_show())
        .filter_map(|app| {
            let desktop = app.downcast::<DesktopAppInfo>().ok()?;
            let path = desktop.filename()?;
            if desktop.id().is_some_and(|id| {
                // Tauri's Debian bundler names the entry after productName.
                matches!(
                    id.as_str(),
                    "TinyDash.desktop" | "dev.tinydash.launcher.desktop"
                )
            }) {
                return None;
            }
            let mut aliases = desktop
                .keywords()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            aliases.push(desktop.executable().to_string_lossy().into_owned());
            if let Some(generic) = desktop.generic_name() {
                aliases.push(generic.to_string());
            }
            let mut entry = AppEntry::new(desktop.display_name().to_string(), path, aliases);
            if let Some(description) = desktop
                .generic_name()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| desktop.description())
                .filter(|value| !value.trim().is_empty())
            {
                entry.description = description.trim().to_owned();
            }
            Some(entry)
        })
        .collect())
}

pub fn launch(entry: &AppEntry) -> Result<()> {
    let app = DesktopAppInfo::from_filename(&entry.path).ok_or(Error::AppNotFound)?;
    // Native desktop activation respects field codes, terminal apps, and D-Bus.
    // Never interpret desktop-file contents through a shell.
    app.launch(&[], None::<&gio::AppLaunchContext>)
        .map_err(|error| Error::Launch(error.to_string()))
}

pub fn system_commands() -> Vec<crate::providers::system::SystemCommand> {
    crate::providers::system::SystemCommand::ALL.to_vec()
}

pub fn run_system_command(command: crate::providers::system::SystemCommand) -> Result<()> {
    use crate::providers::system::SystemCommand;
    use gio::glib::variant::ToVariant;
    match command {
        SystemCommand::Settings => {
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
            let app = settings_desktop_ids(&desktop)
                .iter()
                .find_map(|id| DesktopAppInfo::new(id))
                .ok_or_else(|| {
                    Error::SystemCommand(
                        "No supported settings application is installed for this desktop.".into(),
                    )
                })?;
            app.launch(&[], None::<&gio::AppLaunchContext>)
                .map_err(|error| Error::SystemCommand(error.to_string()))
        }
        SystemCommand::Lock => lock_session(),
        SystemCommand::ToggleAppearance => toggle_appearance(),
        SystemCommand::EmptyTrash => empty_trash(),
        SystemCommand::Logout => logout(),
        SystemCommand::ShowDesktop => show_desktop(),
        SystemCommand::ToggleMute => toggle_mute(),
        _ => {
            let method = power_method(command)?;
            let bus = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>)
                .map_err(|error| Error::SystemCommand(error.to_string()))?;
            // false means no interactive polkit prompt. Respect the current
            // session's policy and inhibitors; never request elevated access.
            bus.call_sync(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                method,
                Some(&(false,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                5000,
                None::<&gio::Cancellable>,
            )
            .map_err(|error| {
                Error::SystemCommand(format!("The session could not {method}: {error}"))
            })?;
            Ok(())
        }
    }
}

fn session_call(
    service: &str,
    path: &str,
    interface: &str,
    method: &str,
    parameters: Option<&gio::glib::Variant>,
) -> Result<gio::glib::Variant> {
    let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        .map_err(|error| Error::SystemCommand(error.to_string()))?;
    bus.call_sync(
        Some(service),
        path,
        interface,
        method,
        parameters,
        None,
        gio::DBusCallFlags::NONE,
        5000,
        None::<&gio::Cancellable>,
    )
    .map_err(|error| Error::SystemCommand(format!("The desktop could not {method}: {error}")))
}

#[derive(Debug, PartialEq, Eq)]
enum Desktop {
    Gnome,
    Kde,
    Xfce,
    Other,
}

fn desktop_kind(value: &str) -> Desktop {
    for name in value.split(':') {
        match name.to_ascii_lowercase().as_str() {
            "gnome" | "ubuntu" | "unity" => return Desktop::Gnome,
            "kde" | "plasma" => return Desktop::Kde,
            "xfce" => return Desktop::Xfce,
            _ => {}
        }
    }
    Desktop::Other
}

fn current_desktop() -> Desktop {
    desktop_kind(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

fn toggle_appearance() -> Result<()> {
    use gio::glib::variant::ToVariant;
    match current_desktop() {
        Desktop::Gnome => {
            // Check the schema before constructing Settings: GSettings aborts
            // the process when a schema or key does not exist.
            let schema = gio::SettingsSchemaSource::default()
                .and_then(|source| source.lookup("org.gnome.desktop.interface", true))
                .filter(|schema| schema.has_key("color-scheme"))
                .ok_or_else(|| Error::SystemCommand("This GNOME version does not provide a light/dark appearance setting.".into()))?;
            let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
            if !settings.is_writable("color-scheme") {
                return Err(Error::SystemCommand("The system appearance is locked by desktop policy.".into()));
            }
            let next = if settings.string("color-scheme") == "prefer-dark" { "prefer-light" } else { "prefer-dark" };
            settings.set_string("color-scheme", next)
                .map_err(|error| Error::SystemCommand(error.to_string()))?;
            gio::Settings::sync();
            Ok(())
        }
        Desktop::Kde => {
            // Use the desktop portal's actual appearance, not a guess based on
            // a custom theme name. Plasma's utility applies the matching Breeze palette.
            let program = program("plasma-apply-colorscheme")?;
            let response = session_call("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings", "Read",
                Some(&("org.freedesktop.appearance", "color-scheme").to_variant()))?;
            let mut value = response.child_value(0);
            while let Some(inner) = value.as_variant() { value = inner; }
            let scheme = value.get::<u32>().ok_or_else(|| Error::SystemCommand("The desktop portal did not return a color scheme.".into()))?;
            let next = if scheme == 1 { "BreezeLight" } else { "BreezeDark" };
            super::system_process::output(program, &[next]).map(|_| ())
        }
        _ => Err(Error::SystemCommand("Appearance switching supports GNOME and KDE Plasma. Use this desktop's appearance settings.".into())),
    }
}

fn empty_trash() -> Result<()> {
    // GIO's trash backend owns the paths, metadata, and mounted-volume rules.
    // Deleting a top-level trash URI removes that item recursively without
    // following symlinks. Never recurse through arbitrary filesystem paths.
    let trash = gio::File::for_uri("trash:///");
    let entries = trash
        .enumerate_children(
            "standard::name",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            None::<&gio::Cancellable>,
        )
        .map_err(|error| {
            Error::SystemCommand(format!("The desktop trash service is unavailable: {error}"))
        })?;
    loop {
        let info = entries
            .next_file(None::<&gio::Cancellable>)
            .map_err(|error| {
                Error::SystemCommand(format!("Some trash items could not be read: {error}"))
            })?;
        let Some(info) = info else {
            break;
        };
        trash
            .child(info.name())
            .delete(None::<&gio::Cancellable>)
            .map_err(|error| {
                Error::SystemCommand(format!("Some trash items could not be deleted: {error}"))
            })?;
    }
    let (closed, error) = entries.close(None::<&gio::Cancellable>);
    if let Some(error) = error {
        return Err(Error::SystemCommand(error.to_string()));
    }
    if !closed {
        return Err(Error::SystemCommand(
            "The desktop could not finish reading the Trash.".into(),
        ));
    }
    Ok(())
}

fn logout() -> Result<()> {
    use gio::glib::variant::ToVariant;
    match current_desktop() {
        // Mode 1 skips a second prompt, but preserves inhibitors. Mode 2 would force logout.
        Desktop::Gnome => session_call("org.gnome.SessionManager", "/org/gnome/SessionManager",
            "org.gnome.SessionManager", "Logout", Some(&(1u32,).to_variant())),
        Desktop::Kde => session_call("org.kde.Shutdown", "/Shutdown", "org.kde.Shutdown", "logout", None),
        Desktop::Xfce => session_call("org.xfce.SessionManager", "/org/xfce/SessionManager",
            "org.xfce.Session.Manager", "Logout", Some(&(false, true).to_variant())),
        _ => return Err(Error::SystemCommand("Log out supports GNOME, KDE Plasma, and Xfce sessions. Use this desktop's log out command.".into())),
    }.map(|_| ())
}

fn show_desktop() -> Result<()> {
    use gio::glib::variant::ToVariant;
    if current_desktop() == Desktop::Kde {
        return session_call(
            "org.kde.KWin",
            "/KWin",
            "org.kde.KWin",
            "showDesktop",
            Some(&(true,).to_variant()),
        )
        .map(|_| ());
    }
    if super::is_wayland() {
        return Err(Error::SystemCommand("Show desktop is supported on KDE Plasma or an X11 desktop with wmctrl. Use your desktop shortcut in this Wayland session.".into()));
    }
    super::system_process::output(program("wmctrl")?, &["-k", "on"]).map(|_| ())
}

fn program(name: &str) -> Result<std::path::PathBuf> {
    gio::glib::find_program_in_path(name).ok_or_else(|| {
        Error::SystemCommand(format!(
            "This action needs {name}, which is not installed or is not in PATH."
        ))
    })
}

fn toggle_mute() -> Result<()> {
    if let Some(wpctl) = gio::glib::find_program_in_path("wpctl")
        && super::system_process::output(&wpctl, &["get-volume", "@DEFAULT_AUDIO_SINK@"]).is_ok()
    {
        return super::system_process::output(
            wpctl,
            &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"],
        )
        .map(|_| ());
    }
    // Select a working audio service before toggling. Never retry a failed
    // mutation through another service, since that could toggle it twice.
    let pactl = gio::glib::find_program_in_path("pactl")
        .ok_or_else(|| Error::SystemCommand("Toggle mute needs PipeWire with wpctl or PulseAudio with pactl and an available sound output.".into()))?;
    super::system_process::output(pactl, &["set-sink-mute", "@DEFAULT_SINK@", "toggle"]).map(|_| ())
}

fn power_method(command: crate::providers::system::SystemCommand) -> Result<&'static str> {
    use crate::providers::system::SystemCommand;
    match command {
        SystemCommand::Sleep => Ok("Suspend"),
        SystemCommand::Restart => Ok("Reboot"),
        SystemCommand::Shutdown => Ok("PowerOff"),
        _ => Err(Error::InvalidAction),
    }
}

fn lock_session() -> Result<()> {
    use gio::glib::variant::ToVariant;
    let bus = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)
        .map_err(|error| Error::SystemCommand(error.to_string()))?;
    // Desktop lock services work across X11 and Wayland when provided. Do not
    // report success for a logind Lock signal with no screen locker listening.
    for (service, path) in [
        ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
        (
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
        ),
        ("org.cinnamon.ScreenSaver", "/org/cinnamon/ScreenSaver"),
        ("org.mate.ScreenSaver", "/org/mate/ScreenSaver"),
    ] {
        // Cinnamon's Lock method takes an away message; the other interfaces
        // have no parameters. Keep these native signatures at this boundary.
        let parameters = (service == "org.cinnamon.ScreenSaver").then(|| ("",).to_variant());
        match bus.call_sync(
            Some(service),
            path,
            service,
            "Lock",
            parameters.as_ref(),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            2000,
            None::<&gio::Cancellable>,
        ) {
            Ok(_) => return Ok(()),
            Err(error) => tracing::debug!(service, %error, "Desktop lock service unavailable"),
        }
    }
    Err(Error::SystemCommand(
        "No desktop screen-lock service accepted the request. Use your desktop's lock command."
            .into(),
    ))
}

fn settings_desktop_ids(desktop: &str) -> &'static [&'static str] {
    for name in desktop.split(':') {
        let ids: &[&str] = match name.to_ascii_lowercase().as_str() {
            "gnome" | "unity" | "ubuntu" => {
                &["org.gnome.Settings.desktop", "gnome-control-center.desktop"]
            }
            "kde" | "plasma" => &[
                "systemsettings.desktop",
                "systemsettings5.desktop",
                "org.kde.systemsettings.desktop",
            ],
            "xfce" => &["xfce4-settings-manager.desktop"],
            "x-cinnamon" | "cinnamon" => &["cinnamon-settings.desktop"],
            "mate" => &["matecc.desktop"],
            "lxqt" => &["lxqt-config.desktop"],
            "cosmic" => &["com.system76.CosmicSettings.desktop"],
            _ => continue,
        };
        return ids;
    }
    &[]
}

// GTK owns the selection protocol on its main thread. Requesting text is
// asynchronous, including when another application supplies the selection.
pub fn watch_clipboard(changed: impl Fn() + 'static) {
    use gtk::prelude::*;
    let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    // gtk-rs 0.18 does not generate the typed owner-change signal binding.
    clipboard.connect_local("owner-change", false, move |_| {
        changed();
        None
    });
}

pub fn read_clipboard(received: impl FnOnce(crate::providers::clipboard::Observed) + 'static) {
    use crate::providers::clipboard::{Observed, is_secret_format};
    let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    // Check the offered targets first. A marked secret is never requested.
    // An empty target list is not a clear: X11 selections also disappear
    // when the copying application exits.
    // gtk-rs 0.18 does not bind gtk_clipboard_request_targets.
    let targets = gtk::gdk::Atom::intern("TARGETS");
    clipboard.request_contents(&targets, move |clipboard, selection| {
        if selection.targets().is_some_and(|targets| {
            targets
                .iter()
                .any(|target| is_secret_format(target.name().as_str()))
        }) {
            received(Observed::Secret);
            return;
        }
        clipboard.request_text(move |_, text| {
            received(Observed::from_text(text.map(str::to_owned)));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_actions_use_the_current_session_and_reject_unknown_desktops() {
        assert_eq!(desktop_kind("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(desktop_kind("KDE"), Desktop::Kde);
        assert_eq!(desktop_kind("Plasma"), Desktop::Kde);
        assert_eq!(desktop_kind("XFCE"), Desktop::Xfce);
        assert_eq!(desktop_kind("sway"), Desktop::Other);
        assert_eq!(desktop_kind(""), Desktop::Other);
    }

    #[test]
    fn settings_follow_the_current_desktop_instead_of_another_installed_one() {
        assert_eq!(
            settings_desktop_ids("ubuntu:GNOME")[0],
            "org.gnome.Settings.desktop"
        );
        assert_eq!(settings_desktop_ids("KDE")[0], "systemsettings.desktop");
        assert_eq!(
            settings_desktop_ids("X-Cinnamon")[0],
            "cinnamon-settings.desktop"
        );
        assert_eq!(
            settings_desktop_ids("XFCE")[0],
            "xfce4-settings-manager.desktop"
        );
        assert!(settings_desktop_ids("sway").is_empty());
        assert!(settings_desktop_ids("").is_empty());
    }

    #[test]
    fn only_power_commands_map_to_login1_methods() {
        use crate::providers::system::SystemCommand;
        assert_eq!(
            power_method(SystemCommand::Sleep).expect("sleep"),
            "Suspend"
        );
        assert_eq!(
            power_method(SystemCommand::Restart).expect("restart"),
            "Reboot"
        );
        assert_eq!(
            power_method(SystemCommand::Shutdown).expect("shutdown"),
            "PowerOff"
        );
        assert!(power_method(SystemCommand::Lock).is_err());
        assert!(power_method(SystemCommand::Settings).is_err());
    }
}
