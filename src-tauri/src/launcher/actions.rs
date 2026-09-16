use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use super::{LauncherState, result::Action, window};
use crate::{error::Error, platform, providers::apps::AppEntry};

pub enum ResolvedAction {
    Launch(AppEntry),
    Reveal(PathBuf),
    Copy(String),
}

#[tauri::command]
pub async fn execute_action(id: String, action: Action, app: AppHandle) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Resolve backend-owned IDs. The webview supplies neither executable
        // paths nor clipboard content. Release the search lock before OS work.
        let action = worker_app
            .state::<LauncherState>()
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .resolve_action(&id, action)
            .map_err(|error| error.to_string())?;
        match action {
            ResolvedAction::Launch(entry) => {
                platform::launch(&entry).map_err(|error| error.to_string())
            }
            ResolvedAction::Reveal(path) => worker_app
                .opener()
                .reveal_item_in_dir(path)
                .map_err(|error| error.to_string()),
            ResolvedAction::Copy(value) => worker_app
                .clipboard()
                .write_text(value)
                .map_err(|error| format!("Could not copy to the clipboard: {error}")),
        }
    })
    .await
    .map_err(|error| error.to_string())??;
    window::hide_launcher(app).map_err(|error| error.to_string())
}
