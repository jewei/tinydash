#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{discover_apps, launch};
#[cfg(target_os = "macos")]
pub use macos::{discover_apps, launch};
#[cfg(target_os = "windows")]
pub use windows::{discover_apps, launch};

pub fn is_wayland() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland"))
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
            manager.search("sa").expect("search");
        }
        println!(
            "Discovered {count} apps. 1,000 searches took {:?}.",
            started.elapsed()
        );
    }
}
