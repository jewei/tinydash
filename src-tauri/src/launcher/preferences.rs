use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
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
        shortcuts_available: !platform::is_wayland()
            && app
                .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
                .is_some(),
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
        self.0
            .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
            .is_some_and(|registry| registry.is_registered(shortcut))
    }
    fn register(&self, shortcut: &str) -> Result<(), String> {
        self.0
            .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
            .ok_or("Global shortcuts are unavailable. Restart TinyDash and try again.")?
            .register(shortcut)
            .map_err(|error| {
                format!("Could not use this shortcut. It may be in use by another app. {error}")
            })
    }
    fn unregister(&self, shortcut: &str) -> Result<(), String> {
        self.0
            .try_state::<tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>>()
            .ok_or("Global shortcuts are unavailable. Restart TinyDash and try again.")?
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
}

fn save_with_shortcuts(
    registry: Option<&dyn ShortcutRegistry>,
    old: &[&str],
    new: &[&str],
    persist: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let Some(registry) = registry else {
        return persist();
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let result = (|| {
        for &shortcut in new {
            if !registry.contains(shortcut) {
                registry.register(shortcut)?;
                added.push(shortcut);
            }
        }
        for &shortcut in old {
            let key = shortcut
                .parse::<tauri_plugin_global_shortcut::Shortcut>()
                .ok();
            if !new
                .iter()
                .any(|value| value.parse::<tauri_plugin_global_shortcut::Shortcut>().ok() == key)
                && registry.contains(shortcut)
            {
                registry.unregister(shortcut)?;
                removed.push(shortcut);
            }
        }
        persist()
    })();
    if let Err(mut error) = result {
        for shortcut in added.into_iter().rev() {
            if let Err(restore) = registry.unregister(shortcut) {
                error.push_str(&format!(" Could not release a new shortcut: {restore}"));
            }
        }
        for shortcut in removed {
            if let Err(restore) = registry.register(shortcut) {
                error.push_str(&format!(
                    " Could not restore a previous shortcut: {restore}"
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

fn restore_shortcuts(registry: &dyn ShortcutRegistry, shortcuts: &[&str]) -> Result<(), String> {
    let mut errors = Vec::new();
    for &shortcut in shortcuts {
        if !registry.contains(shortcut)
            && let Err(error) = registry.register(shortcut)
        {
            errors.push(format!("{shortcut}: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" "))
    }
}

pub fn edit_settings(
    app: &AppHandle,
    edit: impl FnOnce(Settings) -> Result<Settings, String>,
) -> Result<Settings, String> {
    use tauri_plugin_autostart::ManagerExt;
    let state = app.state::<LauncherState>();
    let _update = state
        .settings_update
        .lock()
        .map_err(|_| "Settings are unavailable.")?;
    if state.shortcut_recording.load(Ordering::Acquire) {
        return Err("Finish recording the shortcut before saving.".into());
    }
    let previous = state.settings();
    let settings = edit(previous.clone())?;
    settings.validate().map_err(|error| error.to_string())?;
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    let registry = NativeShortcuts(app);
    save_with_shortcuts(
        (!platform::is_wayland() && previous.shortcuts() != settings.shortcuts())
            .then_some(&registry as &dyn ShortcutRegistry),
        &previous.shortcuts(),
        &settings.shortcuts(),
        || {
            let autostart = app.autolaunch();
            let was_enabled = if previous.start_at_login != settings.start_at_login {
                autostart
                    .is_enabled()
                    .map_err(|error| format!("Could not read start at login: {error}"))?
            } else {
                settings.start_at_login
            };
            let changed = was_enabled != settings.start_at_login;
            if changed {
                if settings.start_at_login {
                    autostart.enable()
                } else {
                    autostart.disable()
                }
                .map_err(|error| format!("Could not change start at login: {error}"))?;
            }
            if let Err(error) = settings::save(&directory, &settings) {
                let mut message = format!("{error:#}");
                if changed {
                    let rollback = if was_enabled {
                        autostart.enable()
                    } else {
                        autostart.disable()
                    };
                    if let Err(error) = rollback {
                        message.push_str(&format!(" Could not restore start at login: {error}"));
                    }
                }
                return Err(message);
            }
            Ok(())
        },
    )?;
    state.replace_settings(settings.clone());
    if previous.clipboard_history_enabled != settings.clipboard_history_enabled {
        state.clipboard.invalidate();
    }
    if settings.clipboard_history_enabled {
        super::clipboard::start(app);
        super::clipboard::refresh(app);
    }
    if previous.clipboard_history_limit != settings.clipboard_history_limit {
        state.storage.apply_clipboard_limit(app);
    }
    if !previous.same_file_settings(&settings) {
        super::files::scan_files(app);
    }
    if previous.currency_rates_enabled != settings.currency_rates_enabled {
        state.currency.warning(None);
        if settings.currency_rates_enabled {
            super::currency::refresh(app, false);
        }
    }
    let _ = app.emit("settings-changed", &settings);
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || edit_settings(&app, |_| Ok(settings)))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn choose_clipboard_history(app: AppHandle, enabled: bool) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        edit_settings(&app, |mut settings| {
            settings.clipboard_history_enabled = enabled;
            settings.clipboard_history_decided = true;
            Ok(settings)
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn app_catalog(app: AppHandle) -> Result<Vec<super::result::SearchResult>, String> {
    app.state::<LauncherState>()
        .search
        .lock()
        .map(|search| search.app_catalog())
        .map_err(|_| "Application list is unavailable.".into())
}

#[tauri::command]
pub async fn set_app_preference(
    app: AppHandle,
    id: String,
    aliases: Vec<String>,
    hidden: bool,
) -> Result<Settings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        edit_settings(&app, |mut settings| {
            if aliases.is_empty() && !hidden {
                settings.app_preferences.remove(&id);
            } else {
                settings
                    .app_preferences
                    .insert(id, settings::AppPreference { aliases, hidden });
            }
            Ok(settings)
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn preview_web_search(search: settings::WebSearch, query: String) -> Result<String, String> {
    if query.len() > 1024 {
        return Err("Use a shorter preview query.".into());
    }
    let settings = Settings {
        web_searches: vec![search.clone()],
        ..Settings::default()
    };
    settings.validate().map_err(|error| error.to_string())?;
    search.url(&query).map_err(|error| error.to_string())
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
        let settings = state.settings();
        let restored = restore_shortcuts(&registry, &settings.shortcuts());
        state.shortcut_recording.store(false, Ordering::Release);
        // Recording has ended even if another app took an old shortcut. Keep
        // Settings usable so the user can save a replacement binding.
        restored?;
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
        let settings = state.settings();
        save_with_shortcuts(Some(&registry), &settings.shortcuts(), &[], || Ok(()))?;
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
            save_with_shortcuts(
                Some(&registry),
                &["Control+Space"],
                &["Alt+Space"],
                || panic!("must not save")
            )
            .is_err()
        );
        assert!(registry.contains("Control+Space"));
        assert!(!registry.contains("Alt+Space"));
    }

    #[test]
    fn recording_recovery_keeps_working_shortcuts_and_tries_keys_after_a_conflict() {
        let registry = Registry::new(true);
        registry.active.borrow_mut().clear();
        let error = restore_shortcuts(&registry, &["Control+Space", "Alt+Space", "Control+KeyC"])
            .unwrap_err();
        assert!(error.contains("Alt+Space: Shortcut conflict"));
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Control+Space".into(), "Control+KeyC".into()])
        );

        // A later recording attempt can also restore a partially registered set.
        restore_shortcuts(&registry, &["Control+Space", "Control+KeyC"]).unwrap();
        assert!(registry.contains("Control+Space"));
        assert!(registry.contains("Control+KeyC"));
    }

    #[test]
    fn failed_save_restores_old_shortcut_and_releases_new_one() {
        let registry = Registry::new(false);
        assert!(
            save_with_shortcuts(Some(&registry), &["Control+Space"], &["Alt+Space"], || Err(
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
    fn category_conflict_releases_only_new_bindings_and_preserves_all_previous_ones() {
        let registry = Registry::new(true);
        registry.register("Control+KeyC").unwrap();
        assert!(
            save_with_shortcuts(
                Some(&registry),
                &["Control+Space", "Control+KeyC"],
                &["Control+Space", "Control+KeyJ", "Alt+Space"],
                || panic!("must not save")
            )
            .is_err()
        );
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Control+Space".into(), "Control+KeyC".into()])
        );
        assert!(
            save_with_shortcuts(
                Some(&registry),
                &["Control+Space", "Control+KeyC"],
                &["Control+KeyJ"],
                || Err("Disk full".into())
            )
            .is_err()
        );
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Control+Space".into(), "Control+KeyC".into()])
        );
    }

    #[test]
    fn successful_save_replaces_the_shortcut_and_repairs_missing_registration() {
        let registry = Registry::new(false);
        save_with_shortcuts(Some(&registry), &["Control+Space"], &["Alt+Space"], || {
            Ok(())
        })
        .unwrap();
        assert_eq!(
            *registry.active.borrow(),
            HashSet::from(["Alt+Space".into()])
        );
        registry.active.borrow_mut().clear();
        save_with_shortcuts(Some(&registry), &["Alt+Space"], &["Alt+Space"], || Ok(())).unwrap();
        assert!(registry.contains("Alt+Space"));
    }
}
