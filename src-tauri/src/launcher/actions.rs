use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{LauncherState, result::Action, window};
use crate::{
    error::Error,
    platform,
    providers::{apps::AppEntry, files::FileEntry, system::SystemCommand},
};

pub enum ResolvedAction {
    Launch(AppEntry),
    File(FileEntry, Action),
    Reveal(PathBuf),
    Copy(String),
    Delete(i64),
    System(SystemCommand),
    OpenUrl(String),
    RegeneratePassword(String),
}

impl ResolvedAction {
    fn check_confirmation(&self, confirmed: bool) -> crate::error::Result<()> {
        if let Self::System(command) = self {
            command.check_confirmation(confirmed)?;
        }
        Ok(())
    }

    fn usage_id(&self, id: &str) -> Option<String> {
        match self {
            Self::Launch(entry) => Some(entry.id.clone()),
            Self::File(entry, Action::Open) => Some(entry.id.clone()),
            Self::Copy(_) if id.starts_with("emoji:") => Some(id.to_owned()),
            Self::System(command) => Some(command.id().into()),
            // Revealing a location is not a launch. Calculation IDs are temporary.
            _ => None,
        }
    }
}

#[tauri::command]
pub async fn execute_action(
    id: String,
    action: Action,
    confirmed: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    let keep_open = matches!(action, Action::Delete | Action::Regenerate);
    let worker_app = app.clone();
    let (usage_id, restore_focus) = tauri::async_runtime::spawn_blocking(move || {
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
        let restore_focus = matches!(action, ResolvedAction::Copy(_));
        // Enforce this in Rust as well as in the dialog. Missing IPC fields
        // never count as consent, including on older frontend builds.
        action
            .check_confirmation(confirmed.unwrap_or(false))
            .map_err(|error| error.to_string())?;
        match action {
            ResolvedAction::System(command) => {
                platform::run_system_command(command).map_err(|error| error.to_string())
            }
            ResolvedAction::File(entry, action) => {
                entry.validate().map_err(|error| error.to_string())?;
                match action {
                    Action::Open => worker_app.opener().open_path(entry.path, None::<&str>),
                    Action::Reveal => worker_app.opener().reveal_item_in_dir(entry.path),
                    _ => return Err(Error::InvalidAction.to_string()),
                }
                .map_err(|error| error.to_string())
            }
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
            ResolvedAction::OpenUrl(url) => worker_app
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|error| error.to_string()),
            ResolvedAction::RegeneratePassword(id) => worker_app
                .state::<LauncherState>()
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable.to_string())?
                .tools
                .regenerate(&id),
            ResolvedAction::Delete(id) => worker_app
                .state::<LauncherState>()
                .storage
                .delete_clipboard(&worker_app, Some(id)),
        }?;
        Ok::<_, String>((usage_id, restore_focus))
    })
    .await
    .map_err(|error| error.to_string())??;
    // Hide after the OS action succeeds, before waiting for disk writes.
    let hidden = if keep_open {
        Ok(())
    } else if restore_focus {
        window::dismiss(&app).map_err(|error| error.to_string())
    } else {
        window::hide(&app).map_err(|error| error.to_string())
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
    fn resolved_disruptive_actions_cannot_skip_confirmation() {
        for command in [
            SystemCommand::Sleep,
            SystemCommand::Restart,
            SystemCommand::Shutdown,
            SystemCommand::EmptyTrash,
            SystemCommand::Logout,
        ] {
            let action = ResolvedAction::System(command);
            assert!(matches!(
                action.check_confirmation(false),
                Err(Error::ConfirmationRequired)
            ));
            assert!(action.check_confirmation(true).is_ok());
            assert_eq!(action.usage_id("ignored"), Some(command.id().into()));
        }
        assert!(
            ResolvedAction::System(SystemCommand::Settings)
                .check_confirmation(false)
                .is_ok()
        );
    }

    #[test]
    fn only_successful_primary_actions_have_durable_usage_ids() {
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
        let file = FileEntry {
            id: "file:/test.txt".into(),
            name: "test.txt".into(),
            path: "/test.txt".into(),
        };
        assert_eq!(
            ResolvedAction::File(file.clone(), Action::Open).usage_id("ignored"),
            Some(file.id.clone())
        );
        assert_eq!(
            ResolvedAction::File(file, Action::Reveal).usage_id("file:/test.txt"),
            None
        );
    }
}
