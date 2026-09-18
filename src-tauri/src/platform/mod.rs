#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{discover_apps, launch, run_system_command, system_commands};
#[cfg(target_os = "linux")]
pub use linux::{read_clipboard, watch_clipboard};
#[cfg(target_os = "macos")]
pub use macos::LauncherFocus;
#[cfg(target_os = "macos")]
pub use macos::clipboard_snapshot;
#[cfg(target_os = "macos")]
pub use macos::load_app_icons;
#[cfg(target_os = "macos")]
pub use macos::{discover_apps, launch, run_system_command, system_commands};
#[cfg(not(target_os = "macos"))]
pub fn load_app_icons(
    apps: Vec<crate::providers::apps::AppEntry>,
) -> Vec<crate::providers::apps::AppEntry> {
    apps
}
#[cfg(target_os = "windows")]
pub use windows::clipboard_snapshot;
#[cfg(target_os = "windows")]
pub use windows::{discover_apps, launch, run_system_command, system_commands};

pub fn is_wayland() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland"))
}

pub fn file_is_hidden(entry: &walkdir::DirEntry) -> std::io::Result<bool> {
    if entry.file_name().to_string_lossy().starts_with('.') {
        return Ok(true);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        // BSD UF_HIDDEN. Finder uses this flag in addition to dot names.
        Ok(entry.metadata()?.st_flags() & 0x8000 != 0)
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM.
        Ok(entry.metadata()?.file_attributes() & 0x6 != 0)
    }
    #[cfg(target_os = "linux")]
    {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "Uses installed macOS applications and WindowServer. Run with --ignored."]
    fn installed_app_results_have_native_icons() {
        use crate::launcher::{query::SearchMode, search::SearchManager};
        use crate::providers::apps::AppProvider;

        let apps =
            super::load_app_icons(super::discover_apps().expect("discover installed applications"));
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(apps));
        let results = manager
            .search("Activity Monitor", SearchMode::Apps)
            .unwrap()
            .results;
        let activity = results
            .iter()
            .find(|result| result.title == "Activity Monitor")
            .expect("Activity Monitor is installed on macOS");
        assert!(
            activity
                .icon
                .as_deref()
                .is_some_and(|icon| icon.starts_with("data:image/png;base64,")),
            "The native search response must include Activity Monitor's image"
        );
        let activity_icon = activity.icon.clone();
        let store = manager
            .search("App Store", SearchMode::Apps)
            .unwrap()
            .results;
        let store = store
            .iter()
            .find(|result| result.title == "App Store")
            .expect("App Store is installed");
        assert!(store.icon.is_some());
        assert_ne!(
            activity_icon, store.icon,
            "Installed apps must have distinct native icons"
        );
    }

    #[test]
    #[ignore = "Scans the host's installed apps. Run with --ignored --nocapture."]
    fn installed_apps_smoke() {
        let apps = super::discover_apps().expect("discover apps");
        assert!(!apps.is_empty(), "No installed applications found");
        let count = apps.len();
        let mut manager = crate::launcher::search::SearchManager::default();
        manager.replace_apps(crate::providers::apps::AppProvider::new(apps));
        let started = std::time::Instant::now();
        for _ in 0..1000 {
            manager
                .search("sa", crate::launcher::query::SearchMode::Apps)
                .expect("search");
        }
        println!(
            "Discovered {count} apps. 1,000 searches took {:?}.",
            started.elapsed()
        );
    }
}
