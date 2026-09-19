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
pub use macos::application_icon;
#[cfg(target_os = "macos")]
pub use macos::clipboard_snapshot;
#[cfg(target_os = "macos")]
pub use macos::{discover_apps, launch, run_system_command, system_commands};
#[cfg(not(target_os = "macos"))]
pub fn application_icon(_path: &std::path::Path, _pixels: u16) -> Option<Vec<u8>> {
    None
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
        let apps = super::discover_apps().expect("discover installed applications");
        let activity = apps
            .iter()
            .find(|app| app.name == "Activity Monitor")
            .expect("Activity Monitor");
        let store = apps
            .iter()
            .find(|app| app.name == "App Store")
            .expect("App Store");
        let activity_icon =
            super::application_icon(&activity.path, 72).expect("Activity Monitor image");
        let store_icon = super::application_icon(&store.path, 72).expect("App Store image");
        for image in [&activity_icon, &store_icon] {
            assert_eq!(&image[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(u32::from_be_bytes(image[16..20].try_into().unwrap()), 72);
            assert_eq!(u32::from_be_bytes(image[20..24].try_into().unwrap()), 72);
        }
        assert_ne!(activity_icon, store_icon);
        let preview = super::application_icon(&activity.path, 128).expect("preview image");
        assert_eq!(u32::from_be_bytes(preview[16..20].try_into().unwrap()), 128);
        assert!(super::application_icon(&activity.path, 257).is_none());

        // Exercise Finder custom icons on a temporary directory, without
        // changing an installed application or relying on its cached image.
        use objc2_app_kit::{NSWorkspace, NSWorkspaceIconCreationOptions};
        use objc2_foundation::NSString;
        let fixture = tempfile::tempdir().unwrap();
        let path = NSString::from_str(fixture.path().to_str().unwrap());
        let workspace = NSWorkspace::sharedWorkspace();
        let original = super::application_icon(fixture.path(), 72).unwrap();
        let custom = workspace.iconForFile(&NSString::from_str(activity.path.to_str().unwrap()));
        assert!(workspace.setIcon_forFile_options(
            Some(&custom),
            &path,
            NSWorkspaceIconCreationOptions::empty()
        ));
        let first = super::application_icon(fixture.path(), 72).unwrap();
        assert_ne!(original, first);
        let replacement = workspace.iconForFile(&NSString::from_str(store.path.to_str().unwrap()));
        assert!(workspace.setIcon_forFile_options(
            Some(&replacement),
            &path,
            NSWorkspaceIconCreationOptions::empty()
        ));
        assert_ne!(first, super::application_icon(fixture.path(), 72).unwrap());
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
