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
            Some(AppEntry::new(
                desktop.display_name().to_string(),
                path,
                aliases,
            ))
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

pub fn read_clipboard(received: impl FnOnce(Option<String>) + 'static) {
    let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    clipboard.request_text(move |_, text| {
        received(
            text.filter(|text| crate::providers::clipboard::valid_text(text))
                .map(str::to_owned),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
