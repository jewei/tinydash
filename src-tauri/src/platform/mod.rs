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
pub use macos::clipboard_snapshot;
#[cfg(target_os = "macos")]
pub use macos::{discover_apps, launch, run_system_command, system_commands};
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
