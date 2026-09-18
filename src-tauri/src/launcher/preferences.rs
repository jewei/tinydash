use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;

use super::LauncherState;
use crate::{
    platform,
    settings::{self, Settings},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInfo {
    settings: Settings,
    defaults: Settings,
    platform: &'static str,
    version: String,
    config_path: String,
    data_path: String,
    shortcuts_available: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<SettingsInfo, String> {
    Ok(SettingsInfo {
        settings: app.state::<LauncherState>().settings(),
        defaults: Settings::default(),
        platform: std::env::consts::OS,
        version: app.package_info().version.to_string(),
        config_path: app
            .path()
            .app_config_dir()
            .map_err(|error| error.to_string())?
            .join("settings.json")
            .to_string_lossy()
            .into_owned(),
        data_path: app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("tinydash.sqlite3")
            .to_string_lossy()
            .into_owned(),
        shortcuts_available: !platform::is_wayland(),
    })
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    // Window creation must not block the event loop on Windows.
    let window = match app.get_webview_window("settings") {
        Some(window) => window,
        None => WebviewWindowBuilder::new(
            &app,
            "settings",
            WebviewUrl::App("index.html?view=settings".into()),
        )
        .title("TinyDash Settings")
        .inner_size(860.0, 640.0)
        .min_inner_size(620.0, 480.0)
        .center()
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?,
    };
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    super::window::hide(&app).map_err(|error| error.to_string())?;
    Ok(())
}

trait ShortcutRegistry {
    fn contains(&self, shortcut: &str) -> bool;
    fn register(&self, shortcut: &str) -> Result<(), String>;
    fn unregister(&self, shortcut: &str) -> Result<(), String>;
}

struct NativeShortcuts<'a>(&'a AppHandle);
impl ShortcutRegistry for NativeShortcuts<'_> {
    fn contains(&self, shortcut: &str) -> bool {
        self.0.global_shortcut().is_registered(shortcut)
    }
    fn register(&self, shortcut: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .register(shortcut)
            .map_err(|error| {
                format!("Could not use this shortcut. It may be in use by another app. {error}")
            })
    }
    fn unregister(&self, shortcut: &str) -> Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
}

fn save_with_shortcut(
    registry: Option<&dyn ShortcutRegistry>,
    old: &str,
    new: &str,
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let Some(registry) = registry else {
        return persist();
    };
    // Register the new shortcut first. A conflict must leave the old one working.
    let added = !registry.contains(new);
    if added {
        registry.register(new)?;
    }
    let same = old.parse::<tauri_plugin_global_shortcut::Shortcut>().ok()
        == new.parse::<tauri_plugin_global_shortcut::Shortcut>().ok();
    let removed = !same && registry.contains(old);
    if removed && let Err(error) = registry.unregister(old) {
        if added {
            let _ = registry.unregister(new);
        }
        return Err(error);
    }
    if let Err(mut error) = persist() {
        if removed && let Err(restore) = registry.register(old) {
            error.push_str(&format!(
                " Could not restore the previous shortcut: {restore}"
            ));
        }
        if added && let Err(restore) = registry.unregister(new) {
            error.push_str(&format!(" Could not release the new shortcut: {restore}"));
        }
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        settings.validate().map_err(|error| error.to_string())?;
        let state = app.state::<LauncherState>();
        let _update = state
            .settings_update
            .lock()
            .map_err(|_| "Settings are unavailable.")?;
        if state.shortcut_recording.load(Ordering::Acquire) {
            return Err("Finish recording the shortcut before saving.".into());
        }
        let previous = state.settings();
        let directory = app
            .path()
            .app_config_dir()
            .map_err(|error| error.to_string())?;
        let registry = NativeShortcuts(&app);
        save_with_shortcut(
            (!platform::is_wayland()).then_some(&registry as &dyn ShortcutRegistry),
            &previous.shortcut,
            &settings.shortcut,
            || settings::save(&directory, &settings).map_err(|error| format!("{error:#}")),
        )?;
        state.replace_settings(settings.clone());
        if previous.clipboard_history_enabled != settings.clipboard_history_enabled {
            state.clipboard.invalidate();
        }
        if settings.clipboard_history_enabled {
            super::clipboard::start(&app);
            super::clipboard::refresh(&app);
        }
        if previous.clipboard_history_limit != settings.clipboard_history_limit {
            state.storage.apply_clipboard_limit(&app);
        }
        if !previous.same_file_settings(&settings) {
            super::files::scan_files(&app);
        }
        if previous.currency_rates_enabled != settings.currency_rates_enabled {
            state.currency.warning(None);
            if settings.currency_rates_enabled {
                super::currency::refresh(&app, false);
            }
        }
        let _ = app.emit("settings-changed", &settings);
        Ok(settings)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn restore_shortcut(app: &AppHandle) -> Result<(), String> {
    if platform::is_wayland() {
        return Ok(());
    }
    let state = app.state::<LauncherState>();
    let _update = state
        .settings_update
        .lock()
        .map_err(|_| "Settings are unavailable.")?;
    if state.shortcut_recording.load(Ordering::Acquire) {
        let registry = NativeShortcuts(app);
        let shortcut = state.settings().shortcut;
        state.shortcut_recording.store(false, Ordering::Release);
        if !registry.contains(&shortcut) {
            registry.register(&shortcut)?;
        }
    }
    Ok(())
}

pub fn restore_shortcut_in_background(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = restore_shortcut(&app) {
            tracing::warn!(%error, "Could not restore launch shortcut");
            let _ = app.emit("shortcut-error", error);
        }
    });
}

#[tauri::command]
pub async fn set_shortcut_recording(app: AppHandle, recording: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !recording {
            return restore_shortcut(&app);
        }
        if platform::is_wayland() {
            return Err("Set the shortcut in your desktop settings on Wayland.".into());
        }
        let state = app.state::<LauncherState>();
        let _update = state
            .settings_update
            .lock()
            .map_err(|_| "Settings are unavailable.")?;
        if !app
            .get_webview_window("settings")
            .is_some_and(|window| window.is_focused().unwrap_or(false))
        {
            return Err("Keep the settings window in focus to record a shortcut.".into());
        }
        let registry = NativeShortcuts(&app);
        let shortcut = state.settings().shortcut;
        if registry.contains(&shortcut) {
            registry.unregister(&shortcut)?;
        }
        state.shortcut_recording.store(true, Ordering::Release);
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn reveal_settings_path(app: AppHandle, data: bool) -> Result<(), String> {
    let path = if data {
        app.path()
            .app_data_dir()
            .map(|path| path.join("tinydash.sqlite3"))
    } else {
        app.path()
            .app_config_dir()
            .map(|path| path.join("settings.json"))
    }
    .map_err(|error| error.to_string())?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashSet};

    struct Registry {
        active: RefCell<HashSet<String>>,
        conflict: bool,
    }
    impl Registry {
        fn new(conflict: bool) -> Self {
            Self {
                active: RefCell::new(HashSet::from(["Control+Space".into()])),
                conflict,
            }
        }
    }
    impl ShortcutRegistry for Registry {
        fn contains(&self, shortcut: &str) -> bool {
            self.active.borrow().contains(shortcut)
        }
        fn register(&self, shortcut: &str) -> Result<(), String> {
            if self.conflict && shortcut == "Alt+Space" {
                return Err("Shortcut conflict".into());
            }
            self.active.borrow_mut().insert(shortcut.into());
            Ok(())
        }
        fn unregister(&self, shortcut: &str) -> Result<(), String> {
            self.active.borrow_mut().remove(shortcut);
            Ok(())
        }
    }

    #[test]
    fn shortcut_conflict_keeps_old_registration_and_does_not_save() {
        let registry = Registry::new(true);
        assert!(
            save_with_shortcut(Some(&registry), "Control+Space", "Alt+Space", || panic!(
                "must not save"
            ))
            .is_err()
        );
        assert!(registry.contains("Control+Space"));
        assert!(!registry.contains("Alt+Space"));
    }

    #[test]
    fn failed_save_restores_old_shortcut_and_releases_new_one() {
        let registry = Registry::new(false);
        assert!(
            save_with_shortcut(Some(&registry), "Control+Space", "Alt+Space", || Err(
                "Disk full".into()
            ))
            .is_err()
        );
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Control+Space".into()])
        );
    }

    #[test]
    fn successful_save_replaces_the_shortcut_and_repairs_missing_registration() {
        let registry = Registry::new(false);
        save_with_shortcut(Some(&registry), "Control+Space", "Alt+Space", || Ok(())).unwrap();
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Alt+Space".into()])
        );
        registry.active.borrow_mut().clear();
        save_with_shortcut(Some(&registry), "Alt+Space", "Alt+Space", || Ok(())).unwrap();
        assert!(registry.contains("Alt+Space"));
    }
}
