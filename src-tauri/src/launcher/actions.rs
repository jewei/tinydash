use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{LauncherState, result::Action, window};
use crate::{error::Error, platform, providers::apps::AppEntry};

pub enum ResolvedAction {
    Launch(AppEntry),
    Reveal(PathBuf),
    Copy(String),
    Delete(i64),
}

impl ResolvedAction {
    fn usage_id(&self, id: &str) -> Option<String> {
        match self {
            Self::Launch(entry) => Some(entry.id.clone()),
            Self::Copy(_) if id.starts_with("emoji:") => Some(id.to_owned()),
            // Revealing a location is not a launch. Calculation IDs are temporary.
            _ => None,
        }
    }
}

#[tauri::command]
pub async fn execute_action(id: String, action: Action, app: AppHandle) -> Result<(), String> {
    let keep_open = action == Action::Delete;
    let worker_app = app.clone();
    let usage_id = tauri::async_runtime::spawn_blocking(move || {
        // Resolve backend-owned IDs. The webview supplies neither executable
        // paths nor clipboard content. Release the search lock before OS work.
        let action = worker_app
            .state::<LauncherState>()
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .resolve_action(&id, action)
            .map_err(|error| error.to_string())?;
        let usage_id = action.usage_id(&id);
        match action {
            ResolvedAction::Launch(entry) => {
                platform::launch(&entry).map_err(|error| error.to_string())
            }
            ResolvedAction::Reveal(path) => worker_app
                .opener()
                .reveal_item_in_dir(path)
                .map_err(|error| error.to_string()),
            ResolvedAction::Copy(value) => {
                worker_app
                    .state::<LauncherState>()
                    .storage
                    .copy(&worker_app, &id, value)
            }
            ResolvedAction::Delete(id) => worker_app
                .state::<LauncherState>()
                .storage
                .delete_clipboard(&worker_app, Some(id)),
        }?;
        Ok::<_, String>(usage_id)
    })
    .await
    .map_err(|error| error.to_string())??;
    // Hide after the OS action succeeds, before waiting for disk writes.
    let hidden = if keep_open {
        Ok(())
    } else {
        window::hide_launcher(app.clone())
    };
    if let Some(id) = usage_id {
        let worker_app = app.clone();
        if let Err(error) = tauri::async_runtime::spawn_blocking(move || {
            let state = worker_app.state::<LauncherState>();
            state.storage.record(&worker_app, &state.search, &id);
        })
        .await
        {
            tracing::warn!(%error, "Could not record successful action");
        }
        if let Err(error) = app.emit("usage-changed", ()) {
            tracing::debug!(%error, "No usage listener");
        }
    }
    hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_app_launches_and_emoji_copies_have_durable_usage_ids() {
        let app = AppEntry::new("Test".into(), "/test".into(), vec![]);
        assert_eq!(
            ResolvedAction::Launch(app.clone()).usage_id("ignored"),
            Some(app.id)
        );
        assert_eq!(
            ResolvedAction::Copy("🚀".into()).usage_id("emoji:🚀"),
            Some("emoji:🚀".into())
        );
        assert_eq!(
            ResolvedAction::Copy("3".into()).usage_id("calculation:1"),
            None
        );
        assert_eq!(
            ResolvedAction::Reveal("/test".into()).usage_id("app:/test"),
            None
        );
    }
}
