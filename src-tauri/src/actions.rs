//! What happens when the user runs a result's action.
//!
//! The frontend sends back an [`Action`] that a search produced. Each action
//! is checked here again before it touches the OS, so the webview can only
//! open what TinyDash itself indexed or offered.

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::{
    error::{Error, Result},
    events,
    features::library::{self, LibraryKind, Target},
    platform, refresh,
    search::{self, Context, id::Source, result::Action},
    state::State,
    system_clipboard, window,
};

/// Pins the user can see.
const MAX_PINS: usize = 100;
/// Pins kept in all, including pins of items that are hidden for now.
const MAX_SAVED_PINS: usize = 300;

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
            if !state.files.get().contains(&path) {
                return Err(Error::msg("This file is not in the index."));
            }
            open_path(Path::new(&path))?;
            window::hide(app)?;
        }
        Action::Reveal { path } => {
            if !state.apps.get().contains(&path) && !state.files.get().contains(&path) {
                return Err(Error::msg("This item is not in the index."));
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
                .clip(id)?
                .ok_or_else(|| Error::msg("This entry is no longer in the history."))?;
            copy_and_close(app, || system_clipboard::write(&content))?;
        }
        // Deleting reveals nothing, so it works while history is off too.
        Action::DeleteClip { id } => {
            state.store.delete_clip(id)?;
            state.pins.update(|pins| pins.remove(&Source::Clip.id(id)));
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
                .filter(|item| item.kind == LibraryKind::Snippet)
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
                .filter(|item| item.kind == LibraryKind::Quicklink)
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
            // Only items that exist can be pinned, and the pin stores the
            // item's own ID, so a pin never widens what Open and Reveal accept.
            let snapshot = state.snapshot();
            let ctx = Context::none();
            let id = search::resolve(&snapshot, &ctx, &id)
                .ok_or_else(|| Error::msg("This item no longer exists."))?
                .id;
            // Count only pins the user can see: a pin of a file outside the
            // index stays saved for when its folder returns, but cannot be
            // unpinned, so it must not use up the limit.
            let visible = snapshot
                .pins
                .ids()
                .iter()
                .filter(|pin| search::resolve(&snapshot, &ctx, pin).is_some())
                .count();
            if visible >= MAX_PINS {
                return Err(Error::msg(format!("You can pin up to {MAX_PINS} items.")));
            }
            // Hidden pins wait for their item to return, but not without end:
            // past the cap, the oldest hidden pin goes.
            if snapshot.pins.ids().len() >= MAX_SAVED_PINS
                && let Some(oldest) = snapshot
                    .pins
                    .ids()
                    .iter()
                    .find(|pin| search::resolve(&snapshot, &ctx, pin).is_none())
            {
                state.store.set_pinned(oldest, false)?;
                state.pins.update(|pins| pins.remove(oldest));
            }
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
    if counts_as_use && let Some(id) = result_id.filter(|id| learns_from_use(id)) {
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
fn learns_from_use(id: &str) -> bool {
    Source::parse(id).is_some_and(|(source, _)| source.learns_from_use())
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
        assert!(learns_from_use("app:/Applications/Safari.app"));
        assert!(learns_from_use("emoji:🚀"));
        assert!(!learns_from_use("calc:1+1"));
        assert!(!learns_from_use("clip:3"));
        assert!(!learns_from_use("password:Pin:6"));
    }
}
