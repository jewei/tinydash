//! What happens when the user runs a result's action.
//!
//! The frontend sends back an [`Action`] that a search produced. Each action
//! is checked here again before it touches the OS, so the webview can only
//! open what TinyDash itself indexed or offered.

use std::path::Path;

use tauri::{AppHandle, Manager};

use crate::{
    commands,
    error::{Error, Result},
    events,
    features::{
        clip_card,
        clipboard::Content,
        library::{self, LibraryKind, Target},
        system::SystemCommand,
    },
    hud, platform, refresh,
    search::{
        self, Context, Snapshot,
        id::Source,
        result::Action,
        usage::{self, MAX_ALIASES, MAX_HIDDEN},
    },
    state::State,
    system_clipboard, timer, window,
};

/// Pins kept in all, including pins of items that are hidden for now.
const MAX_PINS: usize = 100;

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
            | Action::CopyPlainText
            // System commands that act inside TinyDash. From the actions
            // menu they come without a result ID, so only the System
            // results count.
            | Action::ClearClipboard
            | Action::OpenSettings
            | Action::Quit
    );
    let quit = matches!(action, Action::Quit);
    match action {
        Action::Launch { path } => {
            if !state.apps.get().contains(&path) {
                return Err(Error::msg("This app is no longer installed."));
            }
            platform::launch_app(Path::new(&path))?;
            window::hide(app)?;
        }
        Action::Open { path } => {
            if !offered_file(&state, &path) {
                return Err(Error::msg("This file is not in the index."));
            }
            open_path(Path::new(&path))?;
            window::hide(app)?;
        }
        Action::Reveal { path } => {
            if !state.apps.get().contains(&path) && !offered_file(&state, &path) {
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
        Action::CopyJson { pretty } => {
            let text = platform::clipboard_text(app)
                .and_then(|text| clip_card::format_json(&text, pretty))
                .ok_or_else(|| Error::msg("The clipboard no longer holds JSON."))?;
            copy_and_close(app, || system_clipboard::write_text(&text, false))?;
        }
        Action::CopyClip { id } => {
            let content = state
                .clip(id)?
                .ok_or_else(|| Error::msg("This entry is no longer in the history."))?;
            copy_and_close(app, || system_clipboard::write(&content))?;
        }
        Action::CopyClipText { id } => {
            let text = match state.clip(id)? {
                Some(Content::Text(text)) => text,
                Some(Content::Files(paths)) => paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                Some(Content::Image { .. }) => {
                    return Err(Error::msg("An image has no text to copy."));
                }
                None => return Err(Error::msg("This entry is no longer in the history.")),
            };
            copy_and_close(app, || system_clipboard::write_text(&text, false))?;
        }
        // Reads as widgets do, so a copy marked secret stays as it is.
        Action::CopyPlainText => {
            let text = platform::clipboard_text(app).ok_or_else(|| {
                Error::msg(
                    "The clipboard holds no text, or the app that copied it marked it secret.",
                )
            })?;
            copy_and_close(app, || system_clipboard::write_text(&text, false))?;
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
        Action::CopySnippet { id, query } => {
            let library = state.library.get();
            let item = library
                .find(id)
                .filter(|item| item.kind == LibraryKind::Snippet)
                .ok_or_else(|| Error::msg("This snippet was deleted."))?;
            // The clipboard may hold a password, so text that includes it
            // is copied as secret and stays out of clipboard histories.
            let read_clipboard = std::cell::Cell::new(false);
            let text = library::render_snippet(&item.text, &query, chrono::Local::now(), || {
                read_clipboard.set(true);
                system_clipboard::read_text()
            });
            copy_and_close(app, || {
                system_clipboard::write_text(&text, read_clipboard.get())
            })?;
        }
        Action::OpenQuicklink { id, query } => {
            let library = state.library.get();
            let item = library
                .find(id)
                .filter(|item| item.kind == LibraryKind::Quicklink)
                .ok_or_else(|| Error::msg("This quicklink was deleted."))?;
            // Read the way widgets read it: a copy its source marked
            // secret, such as a password, never goes into a link.
            let clipboard = || platform::clipboard_text(app);
            match library::quicklink_target(&item.text, &query, chrono::Local::now(), clipboard)? {
                Target::Url(url) => tauri_plugin_opener::open_url(url, None::<&str>)
                    .map_err(|e| Error::msg(e.to_string()))?,
                Target::Path(path) => open_path(&path)?,
            }
            window::hide(app)?;
        }
        Action::System { command } => {
            // Hide first: Lock and Sleep must not leave the launcher on screen.
            window::hide(app)?;
            if command == SystemCommand::SleepDisplays {
                // Releasing the key that ran it would wake the displays again.
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            if let Err(error) = platform::run_system_command(command) {
                window::show_again(app)?;
                return Err(error);
            }
        }
        Action::Pin { id } => {
            let _limit = state
                .limited_change
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            // Only items that exist can be pinned, and the pin stores the
            // item's own ID, so a pin never widens what Open and Reveal accept.
            // Read readiness first: a scan that finishes after the snapshot
            // must not make its old, empty index count as ready.
            let ready = state.freshness.ready();
            let snapshot = state.snapshot();
            let id = search::resolve(&snapshot, &Context::none(), &id)
                .ok_or_else(|| Error::msg("This item no longer exists."))?
                .id;
            if let Some(oldest) = pin_to_drop(&snapshot, &id, ready)? {
                state.store.set_pinned(&oldest, false)?;
                state.pins.update(|pins| pins.remove(&oldest));
            }
            state.store.set_pinned(&id, true)?;
            state.pins.update(|pins| pins.add(&id));
        }
        Action::Hide { id } => {
            let _limit = state
                .limited_change
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !Source::parse(&id).is_some_and(|(source, _)| source.hideable()) {
                return Err(Error::msg("This result cannot be hidden."));
            }
            if state.hidden.get().len() >= MAX_HIDDEN {
                return Err(Error::msg(format!(
                    "You can hide up to {MAX_HIDDEN} results. Show some again in Settings > Search."
                )));
            }
            state
                .store
                .set_hidden(&id, true, chrono::Utc::now().timestamp())?;
            state.hidden.update(|hidden| hidden.add(&id));
            // A hidden pin would hold a place in the list no one can see.
            state.store.set_pinned(&id, false)?;
            state.pins.update(|pins| pins.remove(&id));
        }
        Action::SetAlias { id, alias } => set_alias(&state, &id, &alias)?,
        Action::Unpin { id } => {
            state.store.set_pinned(&id, false)?;
            state.pins.update(|pins| pins.remove(&id));
        }
        Action::Refresh => {
            refresh::apps(app);
            refresh::files(app);
            refresh::rates(app, true);
            refresh::weather(app, true);
        }
        Action::TurnOnCurrencyRates => commands::turn_on_currency_rates(app)?,
        Action::OpenSettings => window::open_settings(app)?,
        Action::CenterLauncher => window::center(app)?,
        Action::Focus { control } => timer::control(app, control)?,
        // Exits below, once the use is saved: exit ends the process from
        // the main thread while this worker may still be writing.
        Action::Quit => {}
    }
    if counts_as_use && let Some(id) = result_id.filter(|id| learns_from_use(id)) {
        record_use(&state, id);
    }
    if quit {
        app.exit(0);
    } else {
        events::results_stale(app);
    }
    Ok(())
}

/// Give a result an alias, or remove its alias when `alias` is empty. Only
/// a result that exists gets one, and an alias names one result.
fn set_alias(state: &State, id: &str, alias: &str) -> Result<()> {
    let _limit = state
        .limited_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !Source::parse(id).is_some_and(|(source, _)| source.aliasable()) {
        return Err(Error::msg("This result cannot have an alias."));
    }
    let alias = usage::clean_alias(alias)?;
    if alias.is_empty() {
        state.store.set_alias(id, None)?;
        state.aliases.update(|aliases| aliases.remove(id));
        return Ok(());
    }
    let snapshot = state.snapshot();
    let ctx = Context::none();
    if search::resolve(&snapshot, &ctx, id).is_none() {
        return Err(Error::msg("This item no longer exists."));
    }
    let aliases = &snapshot.aliases;
    if let Some(owner) = aliases.owner(&alias).filter(|owner| *owner != id) {
        let name = search::resolve(&snapshot, &ctx, owner).map_or(owner.to_owned(), |r| r.title);
        return Err(Error::msg(format!(
            "“{alias}” is already the alias of {name}. Choose another word."
        )));
    }
    if aliases.get(id).is_none() && aliases.len() >= MAX_ALIASES {
        return Err(Error::msg(format!(
            "You can set up to {MAX_ALIASES} aliases. Remove some in Settings > Search."
        )));
    }
    state.store.set_alias(id, Some(&alias))?;
    state.aliases.update(|aliases| aliases.set(id, &alias));
    Ok(())
}

/// An indexed file, or one a Spotlight search returned: TinyDash offered it.
fn offered_file(state: &State, path: &str) -> bool {
    state.files.get().contains(path)
        || state
            .found
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(path)
}

/// Copy, then hide and hand focus back so the user can paste right away,
/// and say that it worked.
fn copy_and_close(app: &AppHandle, copy: impl FnOnce() -> Result<()>) -> Result<()> {
    copy()?;
    window::dismiss(app)?;
    hud::show(app, "Copied");
    Ok(())
}

fn open_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::msg(format!("{} no longer exists.", path.display())));
    }
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| Error::msg(e.to_string()))
}

/// The pin that must go before `id` fits, or `None` while there is room or
/// `id` is pinned already.
/// A pin of a hidden item (a file outside the index, or a clipboard entry
/// while history is off) waits for its item to return, but it cannot be
/// unpinned, so it gives up its place when the list is full. That way no
/// more than `MAX_PINS` pins ever show. Until a source is `ready`, its pins
/// only look hidden, so they stay.
fn pin_to_drop(
    snapshot: &Snapshot,
    id: &str,
    ready: impl Fn(Source) -> bool,
) -> Result<Option<String>> {
    let pins = snapshot.pins.ids();
    if pins.len() < MAX_PINS || snapshot.pins.contains(id) {
        return Ok(None);
    }
    let ctx = Context::none();
    let known = |pin: &str| Source::parse(pin).is_none_or(|(source, _)| ready(source));
    pins.iter()
        .find(|pin| known(pin) && search::resolve(snapshot, &ctx, pin).is_none())
        .map(|pin| Some(pin.clone()))
        .ok_or_else(|| Error::msg(format!("You can pin up to {MAX_PINS} items.")))
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
    use crate::{
        search::{Category, usage::Pins},
        settings::Settings,
    };

    #[test]
    fn a_full_pin_list_gives_up_its_oldest_hidden_pin() {
        let state = State::for_tests(Settings::default());
        let emoji: Vec<String> = search::search(&state.snapshot(), "", Category::Emoji)
            .into_iter()
            .map(|result| result.id)
            .collect();
        assert_eq!(emoji.len(), MAX_PINS);
        let drop_with = |pins: Vec<String>, scanned: bool| {
            state.pins.set(Pins::new(pins));
            let ready = |source| scanned || !matches!(source, Source::App | Source::File);
            pin_to_drop(&state.snapshot(), "app:/New.app", ready)
        };

        assert_eq!(drop_with(emoji[1..].to_vec(), true).unwrap(), None);
        // History is off, so the clipboard pin is hidden too, but newer.
        let mut pins = vec!["file:/gone".to_owned(), "clip:7".to_owned()];
        pins.extend_from_slice(&emoji[2..]);
        assert_eq!(
            drop_with(pins.clone(), true).unwrap().as_deref(),
            Some("file:/gone")
        );
        // Before the first scan, the file pin may only look hidden.
        assert_eq!(drop_with(pins, false).unwrap().as_deref(), Some("clip:7"));
        assert!(drop_with(emoji.clone(), true).is_err());
        // A second Pin of a pinned item changes nothing.
        state.pins.set(Pins::new(emoji.clone()));
        assert_eq!(
            pin_to_drop(&state.snapshot(), &emoji[5], |_| true).unwrap(),
            None
        );
    }

    #[test]
    fn an_alias_names_one_existing_result() {
        let state = State::for_tests(Settings::default());
        let lock = "system:lock";
        set_alias(&state, lock, " Lk ").unwrap();
        assert_eq!(state.aliases.get().get(lock), Some("lk"));
        let taken = set_alias(&state, "system:sleep", "lk").unwrap_err();
        assert!(taken.to_string().contains("Lock Screen"), "{taken}");
        assert!(set_alias(&state, "system:gone", "x").is_err());
        assert!(set_alias(&state, "clip:1", "x").is_err());
        assert_eq!(
            search::search(&state.snapshot(), "lk", search::Category::All)[0].id,
            lock
        );
        set_alias(&state, lock, "").unwrap();
        assert_eq!(state.store.aliases().unwrap().len(), 0);
        assert_eq!(state.aliases.get().len(), 0);
    }

    #[test]
    fn opens_only_indexed_files_or_ones_spotlight_returned() {
        let state = State::for_tests(Settings::default());
        assert!(!offered_file(&state, "/Users/me/notes.txt"));
        state.found.lock().unwrap().remember("/Users/me/notes.txt");
        assert!(offered_file(&state, "/Users/me/notes.txt"));
        assert!(!offered_file(&state, "/Users/me/other.txt"));
    }

    #[test]
    fn only_stable_results_learn_from_use() {
        assert!(learns_from_use("app:/Applications/Safari.app"));
        assert!(learns_from_use("emoji:🚀"));
        assert!(!learns_from_use("calc:1+1"));
        assert!(!learns_from_use("clip:3"));
        assert!(!learns_from_use("password:Pin:6"));
    }
}
