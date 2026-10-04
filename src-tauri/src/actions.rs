//! What happens when the user runs a result's action.
//!
//! The frontend sends back an [`Action`] that a search produced. Each action
//! is checked here again before it touches the OS, so the webview can only
//! open what TinyDash itself indexed or offered.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    events,
    features::{
        library::{self, Target},
        system::SystemCommand,
    },
    platform, refresh,
    state::State,
    system_clipboard, window,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Action {
    /// Start an indexed application.
    Launch {
        path: String,
    },
    /// Open an indexed or pinned file with its default app.
    Open {
        path: String,
    },
    /// Show a file or app in the file manager.
    Reveal {
        path: String,
    },
    /// Open an http(s) URL in the browser.
    OpenUrl {
        url: String,
    },
    Copy {
        text: String,
    },
    /// Copy text marked secret, so clipboard managers skip it.
    CopySecret {
        text: String,
    },
    /// Copy a clipboard history entry in its original format.
    CopyClip {
        id: i64,
    },
    DeleteClip {
        id: i64,
    },
    /// Delete every unpinned clipboard history entry.
    ClearClipboard,
    /// Copy a snippet with its placeholders filled.
    CopySnippet {
        id: i64,
    },
    OpenQuicklink {
        id: i64,
        query: String,
    },
    System {
        command: SystemCommand,
    },
    Pin {
        id: String,
    },
    Unpin {
        id: String,
    },
    /// Rescan apps and files and download exchange rates now.
    Refresh,
    OpenSettings,
    Quit,
}

/// Run an action. `result_id` identifies the result it came from, for usage ranking.
pub fn run(app: &AppHandle, action: Action, result_id: Option<&str>) -> Result<()> {
    let state = app.state::<State>();
    let counts_as_use = matches!(
        action,
        Action::Launch { .. }
            | Action::Open { .. }
            | Action::OpenUrl { .. }
            | Action::Copy { .. }
            | Action::CopySnippet { .. }
            | Action::OpenQuicklink { .. }
            | Action::System { .. }
    );
    match action {
        Action::Launch { path } => {
            if !state.apps.get().contains(&path) {
                return Err(Error::msg("This app is no longer installed."));
            }
            platform::launch_app(Path::new(&path))?;
            window::hide(app)?;
        }
        Action::Open { path } => {
            let id = format!("file:{path}");
            if !state.files.get().contains(&path) && !state.pins.get().contains(&id) {
                return Err(Error::msg("This file is not in the index."));
            }
            open_path(Path::new(&path))?;
            window::hide(app)?;
        }
        Action::Reveal { path } => {
            if !Path::new(&path).exists() {
                return Err(Error::msg(format!("{path} no longer exists.")));
            }
            tauri_plugin_opener::reveal_item_in_dir(&path)
                .map_err(|e| Error::msg(e.to_string()))?;
            window::hide(app)?;
        }
        Action::OpenUrl { url } => {
            let scheme = url
                .split_once(':')
                .map(|(scheme, _)| scheme.to_ascii_lowercase());
            if !matches!(scheme.as_deref(), Some("http" | "https")) {
                return Err(Error::msg("Only web addresses can be opened."));
            }
            tauri_plugin_opener::open_url(&url, None::<&str>)
                .map_err(|e| Error::msg(e.to_string()))?;
            window::hide(app)?;
        }
        Action::Copy { text } => {
            copy_and_close(app, || system_clipboard::write_text(&text, false))?
        }
        Action::CopySecret { text } => {
            copy_and_close(app, || system_clipboard::write_text(&text, true))?
        }
        Action::CopyClip { id } => {
            let content = state
                .store
                .clip(id)?
                .ok_or_else(|| Error::msg("This entry is no longer in the history."))?;
            copy_and_close(app, || system_clipboard::write(&content))?;
        }
        Action::DeleteClip { id } => {
            state.store.delete_clip(id)?;
            state.pins.update(|pins| pins.remove(&format!("clip:{id}")));
            state.reload_clipboard()?;
        }
        Action::ClearClipboard => {
            state.store.clear_clipboard()?;
            state.reload_clipboard()?;
        }
        Action::CopySnippet { id } => {
            let library = state.library.get();
            let item = library
                .find(id)
                .ok_or_else(|| Error::msg("This snippet was deleted."))?;
            let text = library::render_snippet(
                &item.text,
                chrono::Local::now(),
                system_clipboard::read_text,
            );
            copy_and_close(app, || system_clipboard::write_text(&text, false))?;
        }
        Action::OpenQuicklink { id, query } => {
            let library = state.library.get();
            let item = library
                .find(id)
                .ok_or_else(|| Error::msg("This quicklink was deleted."))?;
            match library::quicklink_target(&item.text, &query)? {
                Target::Url(url) => tauri_plugin_opener::open_url(url, None::<&str>)
                    .map_err(|e| Error::msg(e.to_string()))?,
                Target::Path(path) => open_path(&path)?,
            }
            window::hide(app)?;
        }
        Action::System { command } => {
            window::hide(app)?;
            platform::run_system_command(command)?;
        }
        Action::Pin { id } => {
            state.store.set_pinned(&id, true)?;
            state.pins.update(|pins| pins.add(&id));
        }
        Action::Unpin { id } => {
            state.store.set_pinned(&id, false)?;
            state.pins.update(|pins| pins.remove(&id));
        }
        Action::Refresh => {
            refresh::apps(app);
            refresh::files(app);
            refresh::rates(app, true);
        }
        Action::OpenSettings => window::open_settings(app)?,
        Action::Quit => app.exit(0),
    }
    if counts_as_use && let Some(id) = result_id.filter(|id| tracks_usage(id)) {
        record_use(&state, id);
    }
    events::results_stale(app);
    Ok(())
}

/// Copy, then hide and hand focus back so the user can paste right away.
fn copy_and_close(app: &AppHandle, copy: impl FnOnce() -> Result<()>) -> Result<()> {
    copy()?;
    window::dismiss(app)
}

fn open_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::msg(format!("{} no longer exists.", path.display())));
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

/// Results with stable IDs learn from use; computed answers do not.
fn tracks_usage(id: &str) -> bool {
    ["app:", "file:", "emoji:", "snippet:", "link:", "system:"]
        .iter()
        .any(|prefix| id.starts_with(prefix))
}

fn record_use(state: &State, id: &str) {
    let now = chrono::Utc::now().timestamp();
    let used = state.usage.update(|usage| usage.record(id, now));
    if let Err(error) = state.store.record_use(id, used) {
        tracing::warn!(%error, "Could not save usage");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stable_results_learn_from_use() {
        assert!(tracks_usage("app:/Applications/Safari.app"));
        assert!(tracks_usage("emoji:🚀"));
        assert!(!tracks_usage("calc:1+1"));
        assert!(!tracks_usage("clip:3"));
        assert!(!tracks_usage("password:Pin:6"));
    }
}
