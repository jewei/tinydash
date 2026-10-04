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
    actions::{self, Action},
    error::{Error, Result},
    events,
    features::{
        clipboard::Content,
        emoji::EmojiIndex,
        library::{LibraryItem, LibraryKind},
    },
    refresh,
    search::{self, Category, result::SearchResult},
    settings::{self, Settings},
    shortcut,
    state::State,
    tray, watcher, window,
};

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LauncherInit {
    pub settings: Settings,
    pub platform: Platform,
    /// Startup problems to show once.
    pub warnings: Vec<String>,
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

/// Details for the preview pane, loaded when a result is selected.
#[derive(Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Preview {
    Text {
        text: String,
    },
    /// Load the pixels from `clip://localhost/<id>`.
    Image {
        id: i64,
        width: u32,
        height: u32,
    },
    Files {
        paths: Vec<String>,
    },
    File {
        path: String,
        size: u64,
        modified: Option<i64>,
        is_dir: bool,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct About {
    pub version: String,
    pub data_folder: String,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| Error::msg(error.to_string()))?
}

#[tauri::command]
pub fn launcher_init(state: tauri::State<State>) -> LauncherInit {
    LauncherInit {
        settings: Settings::clone(&state.settings.get()),
        platform: PLATFORM,
        warnings: std::mem::take(&mut *state.warnings.lock().unwrap_or_else(|e| e.into_inner())),
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
    blocking(move || {
        let state = app.state::<State>();
        let Some((prefix, key)) = id.split_once(':') else {
            return Ok(None);
        };
        Ok(match prefix {
            "clip" => {
                let Ok(id) = key.parse() else { return Ok(None) };
                state.store.clip(id)?.map(|content| match content {
                    Content::Text(text) => Preview::Text { text },
                    Content::Image { width, height, .. } => Preview::Image { id, width, height },
                    Content::Files(paths) => Preview::Files {
                        paths: paths.iter().map(|p| p.display().to_string()).collect(),
                    },
                })
            }
            "file" | "app" => std::fs::symlink_metadata(key)
                .ok()
                .map(|metadata| Preview::File {
                    path: key.into(),
                    size: metadata.len(),
                    modified: metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .and_then(|age| i64::try_from(age.as_secs()).ok()),
                    is_dir: metadata.is_dir(),
                }),
            "snippet" => key
                .parse()
                .ok()
                .and_then(|id| state.library.get().find(id).cloned())
                .filter(|item| item.kind == LibraryKind::Snippet)
                .map(|item| Preview::Text { text: item.text }),
            _ => None,
        })
    })
    .await
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<()> {
    window::dismiss(&app)
}

#[tauri::command]
pub fn get_settings(state: tauri::State<State>) -> Settings {
    Settings::clone(&state.settings.get())
}

/// Validate, apply, and save new settings. If any part cannot apply, the
/// previous settings stay in effect and the error explains why.
#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> Result<Settings> {
    blocking(move || {
        let state = app.state::<State>();
        let new = settings.normalized();
        let old = state.settings.get();
        if new.shortcut != old.shortcut
            && let Err(error) = shortcut::register(&app, &new.shortcut)
        {
            shortcut::register(&app, &old.shortcut).ok();
            return Err(error);
        }
        if new.launch_at_login != old.launch_at_login {
            let autostart = app.autolaunch();
            let toggled = if new.launch_at_login {
                autostart.enable()
            } else {
                autostart.disable()
            };
            toggled.map_err(|error| {
                Error::msg(format!("Could not change launch at login: {error}"))
            })?;
        }
        settings::save(&state.config_dir, &new)?;
        state.settings.set(new.clone());

        if new.show_tray_icon != old.show_tray_icon {
            tray::set_visible(&app, new.show_tray_icon)?;
        }
        if new.emoji_languages != old.emoji_languages {
            state.emoji.set(EmojiIndex::new(&new.emoji_languages));
        }
        if new.file_search_folders != old.file_search_folders
            || new.file_search_excluded_dirs != old.file_search_excluded_dirs
        {
            refresh::files(&app);
            watcher::watch(&app);
        }
        if new.currency_rates_enabled && !old.currency_rates_enabled {
            refresh::rates(&app, true);
        }
        events::settings_changed(&app, &new);
        events::results_stale(&app);
        Ok(new)
    })
    .await
}

#[tauri::command]
pub fn pause_shortcut(app: AppHandle, paused: bool) -> Result<()> {
    shortcut::pause(&app, paused)
}

#[tauri::command]
pub fn library_items(state: tauri::State<State>) -> Vec<LibraryItem> {
    state.library.get().items().to_vec()
}

#[tauri::command]
pub fn save_library_item(app: AppHandle, item: LibraryItem) -> Result<LibraryItem> {
    let state = app.state::<State>();
    let saved = state.store.save_library_item(&item.validated()?)?;
    state.reload_library()?;
    events::results_stale(&app);
    Ok(saved)
}

#[tauri::command]
pub fn delete_library_item(app: AppHandle, id: i64) -> Result<()> {
    let state = app.state::<State>();
    state.store.delete_library_item(id)?;
    let ids = [format!("snippet:{id}"), format!("link:{id}")];
    state
        .pins
        .update(|pins| ids.iter().for_each(|id| pins.remove(id)));
    state
        .usage
        .update(|usage| ids.iter().for_each(|id| usage.remove(id)));
    state.reload_library()?;
    events::results_stale(&app);
    Ok(())
}

#[tauri::command]
pub fn about(app: AppHandle) -> About {
    About {
        version: app.package_info().version.to_string(),
        data_folder: app.state::<State>().data_dir.display().to_string(),
    }
}
