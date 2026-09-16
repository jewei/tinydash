use gio::prelude::*;

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
            let desktop = app.downcast::<gio::DesktopAppInfo>().ok()?;
            let path = desktop.filename()?;
            if desktop
                .id()
                .is_some_and(|id| id == "dev.tinydash.launcher.desktop")
            {
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
    let app = gio::DesktopAppInfo::from_filename(&entry.path).ok_or(Error::AppNotFound)?;
    // Native desktop activation respects field codes, terminal apps, and D-Bus.
    // Never interpret desktop-file contents through a shell.
    app.launch(&[], None::<&gio::AppLaunchContext>)
        .map_err(|error| Error::Launch(error.to_string()))
}
