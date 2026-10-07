//! IPC commands: the whole API the windows can call. Wrappers live in
//! `src/lib/ipc.ts`; keep both files in step.
//!
//! Commands stay thin. Slow work runs on a blocking worker so the main
//! thread keeps the windows responsive.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use ts_rs::TS;

use crate::{
    actions,
    error::{Error, Result},
    events,
    features::{
        emoji::EmojiIndex,
        library::{LibraryItem, MAX_LIBRARY_ITEMS},
    },
    platform,
    preview::{self, Preview},
    refresh,
    search::{
        self, Category,
        id::Source,
        result::{Action, SearchResult},
    },
    settings::{self, LauncherPosition, Settings},
    shortcut,
    state::State,
    tray, updates, watcher,
    widgets::{self, Widgets},
    window,
};

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LauncherInit {
    pub settings: Settings,
    pub platform: Platform,
    /// Startup problems to show once.
    pub warnings: Vec<String>,
    /// The category of the latest show, which may predate the page.
    pub category: Option<Category>,
    /// A newer version found before the page loaded, ready to install.
    pub update: Option<String>,
}

#[derive(Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

const PLATFORM: Platform = if cfg!(target_os = "macos") {
    Platform::Macos
} else if cfg!(windows) {
    Platform::Windows
} else {
    Platform::Linux
};

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct About {
    pub version: String,
    /// Where `settings.json` is; the same as `data_folder` on macOS only.
    pub settings_folder: String,
    pub data_folder: String,
    /// Clipboard history can save images and copied files on this OS.
    pub rich_clipboard: bool,
    /// This build can update itself (not on Linux, not in local builds).
    pub self_update: bool,
    /// A newer version found before Settings opened, ready to install.
    pub pending_update: Option<String>,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| Error::msg(error.to_string()))?
}

#[tauri::command]
pub fn launcher_init(app: AppHandle, state: tauri::State<State>) -> LauncherInit {
    LauncherInit {
        settings: Settings::clone(&state.settings.get()),
        platform: PLATFORM,
        warnings: std::mem::take(&mut *state.warnings.lock().unwrap_or_else(|e| e.into_inner())),
        category: *state
            .shown_category
            .lock()
            .unwrap_or_else(|e| e.into_inner()),
        update: updates::pending_version(&app),
    }
}

#[tauri::command]
pub async fn search(
    app: AppHandle,
    query: String,
    category: Category,
) -> Result<Vec<SearchResult>> {
    blocking(move || {
        let snapshot = app.state::<State>().snapshot();
        Ok(search::search(&snapshot, &query, category))
    })
    .await
}

/// `result_id` is sent with a result's primary action so usage can be ranked.
#[tauri::command]
pub async fn run_action(app: AppHandle, action: Action, result_id: Option<String>) -> Result<()> {
    blocking(move || actions::run(&app, action, result_id.as_deref())).await
}

#[tauri::command]
pub async fn preview(app: AppHandle, id: String) -> Result<Option<Preview>> {
    blocking(move || preview::load(&app.state::<State>(), &id)).await
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<()> {
    window::dismiss(&app)
}

/// Start moving the launcher with the pointer. On the main thread, which
/// a window drag needs; it waits for nothing.
#[tauri::command]
pub fn drag_launcher(app: AppHandle) -> Result<()> {
    window::drag(&app)
}

/// Save where the launcher opens, or `None` to center it. The launcher
/// saves its spot after a drag, and Center Launcher clears it. Never call
/// it on the main thread: a settings change may hold the lock while it
/// waits for the main thread.
pub fn set_launcher_position(app: &AppHandle, position: Option<LauncherPosition>) -> Result<()> {
    let state = app.state::<State>();
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let old = state.settings.get();
    if old.launcher_position == position {
        return Ok(());
    }
    let new = Settings {
        launcher_position: position,
        ..Settings::clone(&old)
    };
    settings::save(&state.dirs.config, &new)?;
    state.settings.set(new.clone());
    events::settings_changed(app, &new);
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: tauri::State<State>) -> Settings {
    Settings::clone(&state.settings.get())
}

/// Validate, apply, and save changed settings, given as the fields that
/// changed. Either everything applies, or the previous settings stay in
/// effect and the error explains why.
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    changes: serde_json::Map<String, serde_json::Value>,
) -> Result<Settings> {
    blocking(move || {
        let state = app.state::<State>();
        let _one_change_at_a_time = state
            .settings_change
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let old = state.settings.get();
        let new = old.with_changes(changes)?.normalized();
        // Checked only when the list changes, so an entry from a hand-edited
        // file does not block other settings.
        if new.file_search_folders != old.file_search_folders
            && let Some(folder) = new.relative_folder(&state.dirs.home)
        {
            return Err(Error::msg(format!(
                "“{folder}” is not a full folder path. Enter the whole path, or start it with ~ for your home folder, such as ~/Projects."
            )));
        }
        if new.clock_cities != old.clock_cities
            && let Some(city) = new.unknown_clock_city()
        {
            return Err(Error::msg(format!(
                "TinyDash does not know “{city}”. Enter a city or a time zone, such as Tokyo or Europe/London."
            )));
        }
        let applied = apply_to_system(&app, &old, &new)
            .and_then(|()| settings::save(&state.dirs.config, &new));
        if let Err(error) = applied {
            // Undo whatever part did apply; the original error is the one to show.
            apply_to_system(&app, &new, &old).ok();
            return Err(error);
        }
        state.settings.set(new.clone());

        state.focus.apply(&old, &new);
        if new.emoji_languages != old.emoji_languages {
            state.emoji.set(EmojiIndex::new(&new.emoji_languages));
        }
        if new.file_search_folders != old.file_search_folders
            || new.file_search_excluded_dirs != old.file_search_excluded_dirs
        {
            refresh::files(&app);
            watcher::watch(&app);
        }
        // The settings are saved; a failed trim is retried by the next capture.
        if new.clipboard_history_limit < old.clipboard_history_limit {
            let trimmed = state
                .store
                .trim_clipboard(new.clipboard_history_limit)
                .and_then(|()| state.reload_clipboard());
            if let Err(error) = trimmed {
                tracing::warn!(%error, "Could not remove old clipboard entries");
            }
        }
        if new.currency_rates_enabled && !old.currency_rates_enabled {
            refresh::rates(&app, true);
        }
        if new.show_weather != old.show_weather || new.weather_city != old.weather_city {
            refresh::weather(&app, false);
        }
        events::settings_changed(&app, &new);
        events::results_stale(&app);
        Ok(new)
    })
    .await
}

/// The settings that change the OS: shortcut, login item, and tray icon.
fn apply_to_system(app: &AppHandle, from: &Settings, to: &Settings) -> Result<()> {
    if to.shortcut != from.shortcut {
        shortcut::register(app, &to.shortcut)?;
    }
    if to.launch_at_login != from.launch_at_login {
        let autostart = app.autolaunch();
        let toggled = if to.launch_at_login {
            autostart.enable()
        } else {
            autostart.disable()
        };
        toggled.map_err(|error| Error::msg(format!("Could not change open at login: {error}")))?;
    }
    if to.show_tray_icon != from.show_tray_icon {
        tray::set_visible(app, to.show_tray_icon)?;
    }
    Ok(())
}

/// Runs on a worker: pausing waits for any settings change in progress,
/// and that change may itself be waiting for the main thread.
#[tauri::command]
pub async fn pause_shortcut(app: AppHandle, paused: bool) -> Result<()> {
    blocking(move || shortcut::pause(&app, paused)).await
}

#[tauri::command]
pub fn library_items(state: tauri::State<State>) -> Vec<LibraryItem> {
    state.library.get().items().to_vec()
}

#[tauri::command]
pub async fn save_library_item(app: AppHandle, item: LibraryItem) -> Result<LibraryItem> {
    blocking(move || {
        let state = app.state::<State>();
        let _limit = state
            .limited_change
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if item.id.is_none() && state.library.get().items().len() >= MAX_LIBRARY_ITEMS {
            return Err(Error::msg(format!(
                "The library holds up to {MAX_LIBRARY_ITEMS} snippets and quicklinks."
            )));
        }
        let saved = state.store.save_library_item(&item.validated()?)?;
        state.reload_library()?;
        events::results_stale(&app);
        Ok(saved)
    })
    .await
}

#[tauri::command]
pub async fn delete_library_item(app: AppHandle, id: i64) -> Result<()> {
    blocking(move || {
        let state = app.state::<State>();
        state.store.delete_library_item(id)?;
        let ids = [Source::Snippet.id(id), Source::Link.id(id)];
        state
            .pins
            .update(|pins| ids.iter().for_each(|id| pins.remove(id)));
        state
            .usage
            .update(|usage| ids.iter().for_each(|id| usage.remove(id)));
        state.reload_library()?;
        events::results_stale(&app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn about(app: AppHandle) -> About {
    let dirs = &app.state::<State>().dirs;
    About {
        version: app.package_info().version.to_string(),
        settings_folder: dirs.config.display().to_string(),
        data_folder: dirs.data.display().to_string(),
        rich_clipboard: platform::RICH_CLIPBOARD,
        self_update: updates::supported(),
        pending_update: updates::pending_version(&app),
    }
}

/// Check for a newer version now: its version, or `None` when this is the latest.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<String>> {
    updates::check(&app).await
}

/// Install the version that the latest check found, and restart into it.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<()> {
    updates::install(&app).await
}

/// What the widget pane shows now.
#[tauri::command]
pub async fn widgets(app: AppHandle) -> Result<Widgets> {
    blocking(move || widgets::load(&app.state::<State>())).await
}

/// Save the scratch note of the widget pane.
#[tauri::command]
pub async fn save_note(app: AppHandle, text: String) -> Result<()> {
    blocking(move || widgets::save_note(&app.state::<State>(), &text)).await
}
