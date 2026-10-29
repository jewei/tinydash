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
        calculator, clip_card, keywords,
        library::{self, LibraryKind, Target},
    },
    hud, platform, refresh,
    search::{
        self, Context, Snapshot,
        id::Source,
        result::Action,
        usage::{self, Hotkeys, MAX_ALIASES, MAX_HIDDEN, MAX_HOTKEYS},
    },
    shortcut,
    state::State,
    system_clipboard, timer, widgets, window,
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
            | Action::CopyCalculation { .. }
            | Action::CopySavedCalculation { .. }
            | Action::CopySnippet { .. }
            | Action::OpenQuicklink { .. }
            | Action::System { .. }
            | Action::CopyPlainText
            // System commands that act inside TinyDash. From the actions
            // menu they come without a result ID, so only the System
            // results count.
            | Action::ClearClipboard
            | Action::ClearCalculations
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
            if !web_address(&url) {
                return Err(Error::msg("Only web addresses can be opened."));
            }
            tauri_plugin_opener::open_url(&url, None::<&str>)
                .map_err(|e| Error::msg(e.to_string()))?;
            window::hide(app)?;
        }
        Action::Copy { text } => {
            copy_and_close(app, || system_clipboard::write_text(&text, false))?
        }
        // Computed again, so the history keeps only what the calculator
        // itself answers.
        Action::CopyCalculation { query } => {
            let value = calculator::result(&query, Option::as_ref(&state.rates.get()))
                .ok_or_else(|| Error::msg("This calculation no longer has an answer."))?;
            copy_and_close(app, || system_clipboard::write_text(&value, false))?;
            remember_calculation(&state, &query, &value);
        }
        Action::CopySavedCalculation { id } => {
            let entry = state
                .calculations
                .get()
                .get(id)
                .cloned()
                .ok_or_else(|| Error::msg("This calculation is no longer in the history."))?;
            copy_and_close(app, || system_clipboard::write_text(&entry.result, false))?;
            remember_calculation(&state, &entry.expression, &entry.result);
        }
        Action::DeleteCalculation { id } => {
            state.store.delete_calculation(id)?;
            state.reload_calculations()?;
        }
        Action::ClearCalculations => {
            state.store.clear_calculations()?;
            state.reload_calculations()?;
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
            let content = state
                .clip(id)?
                .ok_or_else(|| Error::msg("This entry is no longer in the history."))?;
            let text = content
                .plain_text()
                .ok_or_else(|| Error::msg("An image has no text to copy."))?;
            copy_and_close(app, || system_clipboard::write_text(&text, false))?;
        }
        // Reads as widgets do, so a secret copy stays as it is; any copy it
        // can read is one that history and links may use too.
        Action::CopyPlainText => {
            let text = platform::clipboard_text(app).ok_or_else(|| {
                Error::msg(
                    "The clipboard holds no text, or its text is secret, such as a copied password.",
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
            // Read the way widgets read it: a secret copy, marked by its
            // source or made in a password app, never goes into a link.
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
            if let Err(error) = platform::run_system_command(command) {
                window::show_again(app)?;
                return Err(error);
            }
        }
        Action::Pin { id } => pin(&state, &id)?,
        Action::Hide { id } => hide(&state, &id)?,
        Action::SetAlias { id, alias } => set_alias(&state, &id, &alias)?,
        Action::SetHotkey { id, shortcut } => set_hotkey(app, &id, &shortcut)?,
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
        Action::TypeQuery { text } => {
            if text.trim().is_empty() || text.chars().count() > keywords::MAX_TYPED {
                return Err(Error::msg(
                    "This keyword cannot be typed. Type it yourself.",
                ));
            }
            events::launcher_query(app, &text);
        }
        Action::Focus { control } => timer::control(app, control)?,
        Action::OpenUpNext { kind } => {
            platform::open_calendar_app(kind)?;
            window::hide(app)?;
        }
        Action::AskCalendarAccess => {
            if !state.settings.get().show_up_next {
                return Err(Error::msg("Turn on Up Next in Settings > Widgets first."));
            }
            widgets::ask_calendar_access(app);
        }
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

/// Pin an item that exists and is not hidden. The pin stores the item's
/// own ID, so a pin never widens what Open and Reveal accept.
fn pin(state: &State, id: &str) -> Result<()> {
    let _limit = state
        .limited_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // Read readiness first: a scan that finishes after the snapshot
    // must not make its old, empty index count as ready.
    let ready = state.freshness.ready();
    let snapshot = state.snapshot();
    let id = search::resolve(&snapshot, &Context::none(), id)
        .ok_or_else(|| Error::msg("This item no longer exists."))?
        .id;
    // A hidden pin would hold a place in the list no one can see.
    if snapshot.hidden.contains(&id) {
        return Err(Error::msg(
            "This item is hidden. Show it again in Settings > Search, then pin it.",
        ));
    }
    if let Some(oldest) = pin_to_drop(&snapshot, &id, ready)? {
        state.store.set_pinned(&oldest, false)?;
        state.pins.update(|pins| pins.remove(&oldest));
    }
    state.store.set_pinned(&id, true)?;
    state.pins.update(|pins| pins.add(&id));
    Ok(())
}

/// Hide an item that exists, under its own ID, as a pin does. A row for a
/// deleted item would stay in Settings and count toward the limit.
fn hide(state: &State, id: &str) -> Result<()> {
    let _limit = state
        .limited_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !Source::parse(id).is_some_and(|(source, _)| source.hideable()) {
        return Err(Error::msg("This result cannot be hidden."));
    }
    let snapshot = state.snapshot();
    let id = search::resolve(&snapshot, &Context::none(), id)
        .ok_or_else(|| Error::msg("This item no longer exists."))?
        .id;
    if snapshot.hidden.len() >= MAX_HIDDEN {
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
    Ok(())
}

/// Give a result an alias, or remove its alias when `alias` is empty. Only
/// a result that exists gets one, under its own ID, as a pin does, and an
/// alias names one result.
pub fn set_alias(state: &State, id: &str, alias: &str) -> Result<()> {
    let _limit = state
        .limited_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if !Source::parse(id).is_some_and(|(source, _)| source.aliasable()) {
        return Err(Error::msg("This result cannot have an alias."));
    }
    let alias = usage::clean_alias(alias)?;
    let snapshot = state.snapshot();
    let ctx = Context::none();
    let found = search::resolve(&snapshot, &ctx, id);
    if alias.is_empty() {
        // The item may be gone; its alias can still be removed.
        let id = found.map_or_else(|| id.to_owned(), |found| found.id);
        state.store.set_alias(&id, None)?;
        state.aliases.update(|aliases| aliases.remove(&id));
        return Ok(());
    }
    let id = found
        .ok_or_else(|| Error::msg("This item no longer exists."))?
        .id;
    let aliases = &snapshot.aliases;
    if let Some(owner) = aliases.owner(&alias).filter(|owner| *owner != id) {
        let name = search::resolve(&snapshot, &ctx, owner).map_or(owner.to_owned(), |r| r.title);
        return Err(Error::msg(format!(
            "“{alias}” is already the alias of {name}. Choose another word."
        )));
    }
    if aliases.get(&id).is_none() && aliases.len() >= MAX_ALIASES {
        return Err(Error::msg(format!(
            "You can set up to {MAX_ALIASES} aliases. Remove some in Settings > Search."
        )));
    }
    state.store.set_alias(&id, Some(&alias))?;
    state.aliases.update(|aliases| aliases.set(&id, &alias));
    Ok(())
}

/// Give a result a hotkey, or remove it with an empty `shortcut`. The keys
/// must register, and neither the launcher nor another result may have them.
/// Either everything changes, or nothing does and the error says why.
pub fn set_hotkey(app: &AppHandle, id: &str, shortcut: &str) -> Result<()> {
    let state = app.state::<State>();
    // A hotkey changes the OS, as the launcher shortcut does, so the two
    // change one at a time, and never while a recorder paused them.
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let snapshot = state.snapshot();
    let (id, shortcut) = checked_hotkey(&snapshot, id, shortcut)?;
    let current = &snapshot.hotkeys;
    let mut next = Hotkeys::clone(current);
    match &shortcut {
        Some(shortcut) => next.set(&id, shortcut),
        None => next.remove(&id),
    }
    // Register first: keys another app holds must not be saved.
    if let Some(failure) = shortcut::sync(app, &next)
        .into_iter()
        .find(|failure| failure.id == id)
    {
        shortcut::sync(app, current);
        return Err(failure.error);
    }
    if let Err(error) = state.store.set_hotkey(&id, shortcut.as_deref()) {
        shortcut::sync(app, current);
        return Err(error);
    }
    state.hotkeys.set(next);
    Ok(())
}

/// The result ID that `set_hotkey` changes, and its keys, or `None` to
/// remove them. Refuses keys that cannot be a hotkey for it.
fn checked_hotkey(
    snapshot: &Snapshot,
    id: &str,
    shortcut: &str,
) -> Result<(String, Option<String>)> {
    if !Source::parse(id).is_some_and(|(source, _)| source.aliasable()) {
        return Err(Error::msg("This result cannot have a hotkey."));
    }
    let found = search::resolve(snapshot, &Context::none(), id);
    let shortcut = shortcut.trim();
    if shortcut.is_empty() {
        // The item may be gone; its hotkey can still be removed.
        return Ok((found.map_or_else(|| id.to_owned(), |found| found.id), None));
    }
    let found = found.ok_or_else(|| Error::msg("This item no longer exists."))?;
    if found.actions.first().is_none_or(|a| a.confirm.is_some()) {
        return Err(Error::msg(format!(
            "{} asks before it runs, so it cannot have a hotkey.",
            found.title
        )));
    }
    let keys = shortcut::parse(shortcut)?;
    if !shortcut::can_be_hotkey(&keys) {
        return Err(Error::msg(
            "A hotkey needs a modifier such as Control or Alt, or a function key, so it never blocks typing.",
        ));
    }
    if shortcut::parse(&snapshot.settings.shortcut).is_ok_and(|launcher| launcher == keys) {
        return Err(Error::msg("These keys show TinyDash. Choose other keys."));
    }
    if let Some(name) = hotkey_owner(snapshot, &keys, Some(&found.id)) {
        return Err(Error::msg(format!(
            "These keys already run {name}. Choose other keys."
        )));
    }
    let hotkeys = &snapshot.hotkeys;
    if hotkeys.get(&found.id).is_none() && hotkeys.len() >= MAX_HOTKEYS {
        return Err(Error::msg(format!(
            "You can set up to {MAX_HOTKEYS} hotkeys. Remove some in Settings > Search."
        )));
    }
    Ok((found.id, Some(shortcut.to_owned())))
}

/// The name of the result whose hotkey is `keys`, other than `except`.
pub fn hotkey_owner(
    snapshot: &Snapshot,
    keys: &tauri_plugin_global_shortcut::Shortcut,
    except: Option<&str>,
) -> Option<String> {
    let (owner, _) = snapshot.hotkeys.iter().find(|(owner, accelerator)| {
        Some(*owner) != except && shortcut::parse(accelerator).is_ok_and(|other| other == *keys)
    })?;
    Some(
        search::resolve(snapshot, &Context::none(), owner)
            .map_or_else(|| owner.to_owned(), |found| found.title),
    )
}

/// Run the main action of the result that a hotkey belongs to, as Enter
/// would in the launcher.
pub fn run_hotkey(app: &AppHandle, id: &str) -> Result<()> {
    let snapshot = app.state::<State>().snapshot();
    let ctx = Context {
        usage: &snapshot.usage,
        pins: &snapshot.pins,
        now: snapshot.now.timestamp(),
        skin_tone: snapshot.settings.emoji_skin_tone,
    };
    let found = search::resolve(&snapshot, &ctx, id).ok_or_else(|| {
        Error::msg("This hotkey's item no longer exists. Remove the hotkey in Settings > Search.")
    })?;
    let main = found
        .actions
        .into_iter()
        .next()
        .filter(|action| action.confirm.is_none())
        .ok_or_else(|| Error::msg("This item asks before it runs. Run it from TinyDash."))?;
    run(app, main.action, Some(id))
}

/// An http or https address, the only kind the OS opener may get: another
/// scheme can start a program.
fn web_address(url: &str) -> bool {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    matches!(scheme.as_deref(), Some("http" | "https"))
}

/// Whether the launcher offered this file: it is indexed, or Spotlight
/// returned it. Opening and previewing both check this.
pub(crate) fn offered_file(state: &State, path: &str) -> bool {
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

/// Keep a copied calculation in the history. The copy already worked, so a
/// failed save only costs the history entry.
fn remember_calculation(state: &State, expression: &str, value: &str) {
    if expression.chars().count() > calculator::MAX_EXPRESSION {
        return;
    }
    let saved = state
        .store
        .save_calculation(expression, value, chrono::Utc::now().timestamp())
        .and_then(|()| state.reload_calculations());
    if let Err(error) = saved {
        tracing::warn!(%error, "Could not save the calculation");
    }
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
/// only look hidden, so they stay. A pin of an item the user hid shows in
/// no list either; one can remain from before Pin refused hidden items.
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
        .find(|pin| {
            snapshot.hidden.contains(pin)
                || (known(pin) && search::resolve(snapshot, &ctx, pin).is_none())
        })
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
        search::{
            Category,
            usage::{Aliases, Hidden, Pins},
        },
        settings::Settings,
    };

    #[test]
    fn a_hotkey_needs_free_keys_and_an_item_that_never_asks() {
        let state = State::for_tests(Settings::default());
        let check = |id: &str, keys: &str| {
            checked_hotkey(&state.snapshot(), id, keys).map_err(|error| error.to_string())
        };
        assert_eq!(
            check("system:lock", " Alt+L "),
            Ok(("system:lock".into(), Some("Alt+L".into())))
        );
        assert_eq!(
            check("system:lock", "Control+Shift+Space").unwrap_err(),
            "These keys show TinyDash. Choose other keys."
        );
        assert!(check("system:lock", "Alt+Nothing").is_err());
        // Plain keys would stop that key from typing in every app.
        for keys in ["KeyA", "Shift+KeyA", "Shift+Space"] {
            assert!(check("system:lock", keys).is_err(), "{keys}");
        }
        assert!(check("system:lock", "F5").is_ok());
        assert!(check("system:lock", "Super+KeyL").is_ok());
        assert_eq!(
            check("system:sleep", "Alt+S").unwrap_err(),
            "Sleep asks before it runs, so it cannot have a hotkey."
        );
        assert_eq!(
            check("clip:1", "Alt+C").unwrap_err(),
            "This result cannot have a hotkey."
        );
        assert_eq!(
            check("app:/Gone.app", "Alt+G").unwrap_err(),
            "This item no longer exists."
        );
        // A gone item's hotkey can still be removed.
        assert_eq!(
            check("app:/Gone.app", ""),
            Ok(("app:/Gone.app".into(), None))
        );

        // The same keys, written another way, belong to Lock Screen.
        state
            .hotkeys
            .set(Hotkeys::new([("system:lock".into(), "Alt+KeyL".into())]));
        assert_eq!(
            check("system:open-trash", "alt+l").unwrap_err(),
            "These keys already run Lock Screen. Choose other keys."
        );
        assert!(check("system:lock", "Alt+L").is_ok());

        state.hotkeys.set(Hotkeys::new(
            (0..MAX_HOTKEYS).map(|n| (format!("app:/{n}.app"), format!("Alt+F{}", n % 24 + 1))),
        ));
        assert!(
            check("system:lock", "Alt+Shift+L")
                .unwrap_err()
                .starts_with("You can set up to 50 hotkeys.")
        );
    }

    #[test]
    fn copying_a_history_entry_moves_it_to_the_top() {
        let state = State::for_tests(Settings::default());
        remember_calculation(&state, "1+1", "2");
        remember_calculation(&state, "2*3", "6");
        let older = state.calculations.get().answer("= 1+1").unwrap()[0]
            .id
            .clone();
        let id: i64 = older.trim_start_matches("calc-history:").parse().unwrap();
        let entry = state.calculations.get().get(id).cloned().unwrap();
        remember_calculation(&state, &entry.expression, &entry.result);
        let titles: Vec<String> = state
            .calculations
            .get()
            .answer("=")
            .unwrap()
            .into_iter()
            .map(|result| result.title)
            .collect();
        assert_eq!(titles, ["2", "6"]);
    }

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
        // A pin made before its item was hidden shows in no list.
        state.hidden.set(Hidden::new([emoji[3].clone()]));
        assert_eq!(
            drop_with(emoji.clone(), true).unwrap().as_deref(),
            Some(emoji[3].as_str())
        );
        state.hidden.set(Hidden::default());
        // A second Pin of a pinned item changes nothing.
        state.pins.set(Pins::new(emoji.clone()));
        assert_eq!(
            pin_to_drop(&state.snapshot(), &emoji[5], |_| true).unwrap(),
            None
        );
    }

    #[test]
    fn only_existing_items_are_hidden_and_hidden_ones_are_not_pinned() {
        let state = State::for_tests(Settings::default());
        let lock = "system:lock";
        pin(&state, lock).unwrap();
        hide(&state, lock).unwrap();
        assert!(state.hidden.get().contains(lock));
        assert!(!state.pins.get().contains(lock));
        let hidden = pin(&state, lock).unwrap_err();
        assert!(hidden.to_string().contains("hidden"), "{hidden}");
        assert!(!state.pins.get().contains(lock));
        assert!(state.store.pins().unwrap().ids().is_empty());

        assert!(hide(&state, "system:gone").is_err());
        assert!(hide(&state, "snippet:99").is_err());
        assert_eq!(state.store.hidden().unwrap(), [lock]);
        assert_eq!(state.hidden.get().len(), 1);
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
        // Another spelling of an ID resolves to the item's own.
        let item = crate::features::library::LibraryItem {
            id: None,
            kind: LibraryKind::Snippet,
            name: "Sig".into(),
            keyword: String::new(),
            text: "Regards".into(),
        };
        let saved = state.store.save_library_item(&item).unwrap().id.unwrap();
        state.reload_library().unwrap();
        set_alias(&state, &format!("snippet:0{saved}"), "sg").unwrap();
        let own = Source::Snippet.id(saved);
        assert_eq!(state.aliases.get().owner("sg"), Some(own.as_str()));
        set_alias(&state, &format!("snippet:+{saved}"), "").unwrap();
        assert_eq!(state.aliases.get().owner("sg"), None);
        assert_eq!(
            search::search(&state.snapshot(), "lk", search::Category::All)[0].id,
            lock
        );
        set_alias(&state, lock, "").unwrap();
        assert_eq!(state.store.aliases().unwrap().len(), 0);
        assert_eq!(state.aliases.get().len(), 0);
    }

    #[test]
    fn a_full_alias_list_refuses_new_aliases_but_allows_renames() {
        let state = State::for_tests(Settings::default());
        let lock = "system:lock";
        let mut pairs = vec![(lock.to_owned(), "lk".to_owned())];
        pairs.extend((1..MAX_ALIASES).map(|n| (format!("app:/{n}.app"), format!("a{n}"))));
        state.aliases.set(Aliases::new(pairs));
        assert_eq!(state.aliases.get().len(), MAX_ALIASES);

        let full = set_alias(&state, "system:sleep", "sl").unwrap_err();
        assert!(full.to_string().contains("Remove some"), "{full}");
        set_alias(&state, lock, "lock").unwrap();
        assert_eq!(state.aliases.get().get(lock), Some("lock"));
        assert_eq!(state.aliases.get().len(), MAX_ALIASES);
    }

    #[test]
    fn an_alias_of_a_gone_item_can_still_be_removed() {
        let state = State::for_tests(Settings::default());
        let gone = "app:/Gone.app";
        state.store.set_alias(gone, Some("gn")).unwrap();
        state.aliases.set(state.store.aliases().unwrap());
        assert_eq!(state.aliases.get().owner("gn"), Some(gone));
        set_alias(&state, gone, "").unwrap();
        assert_eq!(state.aliases.get().owner("gn"), None);
        assert_eq!(state.store.aliases().unwrap().len(), 0);
    }

    #[test]
    fn opens_only_web_addresses() {
        for url in ["https://x.test", "HTTP://x.test"] {
            assert!(web_address(url), "{url}");
        }
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "httpx://x",
            " https://x",
            "ms-settings:",
            "no-scheme",
        ] {
            assert!(!web_address(url), "{url}");
        }
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
