use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{LauncherState, result::Action, window};
use crate::{error::Error, platform};

#[tauri::command]
pub async fn execute_action(id: String, action: Action, app: AppHandle) -> Result<(), String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Resolve the ID from the current index. The webview cannot supply paths,
        // executable names, command arguments, or a command line.
        let entry = worker_app
            .state::<LauncherState>()
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .app(&id)
            .map_err(|error| error.to_string())?;
        match action {
            Action::Launch => platform::launch(&entry).map_err(|error| error.to_string()),
            Action::Reveal => worker_app
                .opener()
                .reveal_item_in_dir(&entry.path)
                .map_err(|error| error.to_string()),
        }
    })
    .await
    .map_err(|error| error.to_string())??;
    window::hide_launcher(app).map_err(|error| error.to_string())
}
