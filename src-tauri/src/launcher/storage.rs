use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use super::LauncherState;
use super::query::SearchMode;
use super::search::SearchManager;
use crate::{
    db::Database,
    error::Error,
    providers::clipboard::{
        ClipboardProvider, MAX_SELECTION_ENTRIES, Observed, combine_entries, entry_id, valid_text,
    },
    ranking,
};

#[derive(Default)]
struct Session {
    database: Option<Database>,
    observed: Observation,
}

/// Seconds after a capture in which an upstream clear still deletes it. Apple
/// Passwords sets no secret marker and empties the clipboard after about 90 s.
const CLEARED_CAPTURE_SECONDS: i64 = 120;

#[derive(Default)]
struct Observation {
    text: Option<String>,
    // The entry that the monitor saved for `text`, and when.
    captured: Option<(i64, i64)>,
}

impl Observation {
    fn copied(&mut self, text: String, sensitive: bool) -> bool {
        let changed = self.changed(Some(text)).is_some();
        // Remember the OS value so the monitor also skips our password copy.
        changed && !sensitive
    }
    fn changed(&mut self, text: Option<String>) -> Option<String> {
        let text = text.filter(|text| valid_text(text));
        if self.text == text {
            return None;
        }
        self.text.clone_from(&text);
        self.captured = None;
        text
    }
    // A source that empties the clipboard considered its content sensitive.
    // Return only the entry saved for that content. Older history, copies
    // from TinyDash, and captures outside the window stay.
    fn cleared(&mut self, now: i64) -> Option<i64> {
        self.text = None;
        self.captured
            .take()
            .filter(|(_, at)| now.saturating_sub(*at) <= CLEARED_CAPTURE_SECONDS)
            .map(|(id, _)| id)
    }
}

#[derive(Default)]
pub struct Storage {
    database: OnceLock<Mutex<Session>>,
    warning: Mutex<Option<String>>,
}

impl Storage {
    // Call only on a blocking worker. Search itself never accesses SQLite or
    // waits on this mutex. OnceLock also orders any action during startup.
    fn session(&self, app: &AppHandle, search: &Mutex<SearchManager>) -> &Mutex<Session> {
        self.database.get_or_init(|| {
            let loaded = (|| -> anyhow::Result<Database> {
                let path = app.path().app_data_dir()?.join("tinydash.sqlite3");
                let settings = app.path().app_config_dir()?.join("settings.json");
                let database = Database::open_with_settings(&path, &settings)?;
                let usage = database.load_usage()?;
                let pins = database.load_pins()?;
                let settings = app.state::<LauncherState>().settings();
                // An unreadable settings file uses undecided first-use values.
                // Those fallback limits must not prune existing history.
                if settings.clipboard_history_decided {
                    database.prune_clipboard(settings.clipboard_limit())?;
                }
                let clipboard = ClipboardProvider::new(database.load_clipboard()?);
                let rates = match database.load_rates() {
                    Ok(rates) => rates,
                    Err(error) => {
                        tracing::warn!(%error, "Could not load cached currency rates");
                        app.state::<LauncherState>().currency.warning(Some("Could not load saved currency rates. Refresh rates in Calculator mode.".into()));
                        None
                    }
                };
                let mut search = search.lock().map_err(|_| Error::IndexUnavailable)?;
                search.set_usage(usage);
                search.set_pins(pins);
                search.clipboard = clipboard;
                if let Some(rates) = rates {
                    app.state::<LauncherState>().currency.loaded(&rates);
                    search.set_rates(rates);
                }
                Ok(database)
            })();
            Mutex::new(Session {
                database: match loaded {
                    Ok(database) => Some(database),
                    Err(error) => {
                        self.failed(&error);
                        let recovery = match error.downcast_ref::<crate::db::Error>() {
                            Some(crate::db::Error::Backup(_)) => Some("Could not save a recovery backup. Your database was not migrated. Check disk space and folder access, then restart TinyDash."),
                            Some(crate::db::Error::NewerSchema { .. }) => Some("Your saved data needs a compatible TinyDash version. Your database was kept. Use a version that supports its schema."),
                            _ => None,
                        };
                        if let Some(message) = recovery
                            && let Ok(mut warning) = self.warning.lock()
                        {
                            *warning = Some(message.into());
                        }
                        None
                    }
                },
                observed: Observation::default(),
            })
        })
    }

    pub fn initialize(&self, app: &AppHandle, search: &Mutex<SearchManager>) {
        self.session(app, search);
    }

    pub fn set_pinned(
        &self,
        app: &AppHandle,
        id: &str,
        category: SearchMode,
        pinned: bool,
    ) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Pin storage is unavailable.")?;
        let key = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .pin_key(id, category)
            .map_err(|error| error.to_string())?;
        let database = session
            .database
            .as_ref()
            .ok_or("Could not save the pin. Local storage is unavailable.")?;
        database
            .set_pinned(&key, category, pinned)
            .map_err(|error| format!("Could not save the pin: {error}"))?;
        // Publish only after a successful write. Searches do not wait for disk.
        state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .set_pinned(&key, category, pinned);
        Ok(())
    }

    pub fn apply_clipboard_limit(&self, app: &AppHandle) {
        let state = app.state::<LauncherState>();
        let Ok(mut session) = self.session(app, &state.search).lock() else {
            return;
        };
        let outcome = (|| -> anyhow::Result<()> {
            let database = session
                .database
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Clipboard storage is unavailable"))?;
            database.prune_clipboard(state.settings().clipboard_limit())?;
            let entries = database.load_clipboard()?;
            state
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable)?
                .clipboard = ClipboardProvider::new(entries);
            Ok(())
        })();
        if let Err(error) = outcome {
            self.failed(&error);
            session.database = None;
        }
        super::clipboard::changed(app);
    }

    pub fn save_rates(
        &self,
        app: &AppHandle,
        rates: &crate::currency::Rates,
    ) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let session = self
            .session(app, &state.search)
            .lock()
            .map_err(|error| error.to_string())?;
        let database = session
            .database
            .as_ref()
            .ok_or("Currency rates are available for this session, but could not be saved.")?;
        database
            .save_rates(rates)
            .map_err(|error| error.to_string())
    }

    pub fn record(&self, app: &AppHandle, search: &Mutex<SearchManager>, id: &str) {
        let database = self.session(app, search);
        // Serialize writes so an older action cannot overwrite a newer count.
        let Ok(mut database) = database.lock() else {
            return;
        };
        let usage = match search.lock() {
            Ok(mut search) => search.record_usage(id, ranking::now()),
            Err(error) => {
                tracing::warn!(%error, "Could not update usage ranking");
                return;
            }
        };
        if let Some(store) = database.database.as_ref()
            && let Err(error) = store.save_usage(id, usage)
        {
            self.failed(&error);
            // Continue with session-only ranking. Do not retry or flood logs.
            database.database = None;
        }
    }

    pub fn capture(&self, app: &AppHandle, observed: Observed, generation: u64) {
        let state = app.state::<LauncherState>();
        let Ok(mut session) = self.session(app, &state.search).lock() else {
            return;
        };
        // A delete, clear, or copy invalidates reads that were already in flight.
        if generation != state.clipboard.generation() {
            return;
        }
        let text = match observed {
            Observed::Cleared => {
                if let Some(id) = session.observed.cleared(ranking::now()) {
                    self.forget_cleared(app, &mut session, id);
                }
                return;
            }
            Observed::Text(text) => Some(text),
            // A marked secret is never read, and it replaces the previous value.
            Observed::Secret | Observed::Other => None,
        };
        if !state.settings().clipboard_history_enabled {
            return;
        }
        let Some(text) = session.observed.changed(text) else {
            return;
        };
        if let Some(id) = self.save_clipboard(app, &mut session, &text) {
            session.observed.captured = Some((id, ranking::now()));
        }
    }

    fn forget_cleared(&self, app: &AppHandle, session: &mut Session, id: i64) {
        let state = app.state::<LauncherState>();
        let Some(database) = session.database.as_ref() else {
            return;
        };
        match database.delete_unpinned_clipboard(id) {
            Ok(false) => {}
            Ok(true) => {
                if let Ok(mut search) = state.search.lock() {
                    search.clipboard.remove(id);
                }
                super::clipboard::changed(app);
            }
            Err(error) => {
                self.failed(&error);
                session.database = None;
                super::clipboard::changed(app);
            }
        }
    }

    fn save_clipboard(&self, app: &AppHandle, session: &mut Session, text: &str) -> Option<i64> {
        let state = app.state::<LauncherState>();
        if !state.settings().clipboard_history_enabled || !valid_text(text) {
            return None;
        }
        let database = session.database.as_mut()?;
        match database.capture_clipboard(text, ranking::now(), state.settings().clipboard_limit()) {
            Ok((entry, removed)) => {
                let id = entry.id;
                let indexed = entry.into();
                if let Ok(mut search) = state.search.lock() {
                    search.clipboard.update(indexed, &removed);
                }
                super::clipboard::changed(app);
                Some(id)
            }
            Err(error) => {
                self.failed(&error);
                session.database = None;
                super::clipboard::changed(app);
                None
            }
        }
    }

    // Serialize our own writes with observations, delete, and clear. OS and disk
    // work run on a worker, with no search lock held.
    pub fn copy(&self, app: &AppHandle, id: &str, text: String) -> Result<(), String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        // Revalidate after acquiring the storage lock so a concurrent delete
        // cannot copy an entry the user has already removed.
        let clipboard_id = if id.starts_with("clipboard:") {
            Some(
                state
                    .search
                    .lock()
                    .map_err(|_| Error::IndexUnavailable.to_string())?
                    .clipboard_entry(id)
                    .map_err(|error| error.to_string())?
                    .id,
            )
        } else {
            None
        };
        let secret = id.starts_with("password:");
        if secret {
            super::clipboard::write_secret(&text).map_err(|error| error.to_string())
        } else {
            app.clipboard()
                .write_text(text.clone())
                .map_err(|error| error.to_string())
        }
        .map_err(|error| format!("Could not copy to the clipboard: {error}"))?;
        state.clipboard.invalidate();
        let changed = session.observed.copied(text.clone(), secret);
        if let Some(id) = clipboard_id {
            if let Some(database) = session.database.as_mut() {
                match database.touch_clipboard(id, ranking::now()) {
                    Ok(Some(entry)) => {
                        let indexed = entry.into();
                        if let Ok(mut search) = state.search.lock() {
                            search.clipboard.update(indexed, &[]);
                        }
                        super::clipboard::changed(app);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.failed(&error);
                        session.database = None;
                        super::clipboard::changed(app);
                    }
                }
            }
        } else if changed {
            self.save_clipboard(app, &mut session, &text);
        }
        Ok(())
    }

    pub fn edit_clipboard(&self, app: &AppHandle, id: &str, text: String) -> Result<(), String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if !valid_text(&text) {
            return Err("Clipboard text is empty or too large.".into());
        }
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .clipboard_entry(id)
            .map_err(|error| error.to_string())?;
        app.clipboard()
            .write_text(text.clone())
            .map_err(|error| format!("Could not copy to the clipboard: {error}"))?;
        state.clipboard.invalidate();
        session.observed.copied(text.clone(), false);
        // Editing a copy must not modify history or evict the original entry.
        Ok(())
    }

    pub fn copy_clipboard_selection(
        &self,
        app: &AppHandle,
        ids: &[String],
        separator: &str,
    ) -> Result<(), String> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if ids.is_empty() {
            return Err("Select at least one clipboard entry.".into());
        }
        if ids.len() > MAX_SELECTION_ENTRIES {
            return Err(format!(
                "Select no more than {MAX_SELECTION_ENTRIES} clipboard entries."
            ));
        }
        let numeric_ids = ids
            .iter()
            .map(|id| entry_id(id).ok_or("A clipboard entry is no longer available."))
            .collect::<Result<Vec<_>, _>>()?;
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        let text = {
            let search = state
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable.to_string())?;
            // Borrow history while validating the combined size. Oversized
            // selections must not allocate a copy of every selected payload.
            let entries = search
                .clipboard
                .entries_for_ids(&numeric_ids)
                .ok_or("A clipboard entry is no longer available.")?;
            combine_entries(&entries, separator)?
        };
        app.clipboard()
            .write_text(text.clone())
            .map_err(|error| format!("Could not copy to the clipboard: {error}"))?;
        state.clipboard.invalidate();
        session.observed.copied(text, false);
        Ok(())
    }

    pub fn delete_clipboard(&self, app: &AppHandle, id: Option<i64>) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        let mut search = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?;
        let database = session
            .database
            .as_ref()
            .ok_or("Clipboard storage is unavailable. Restart TinyDash to try again.")?;
        let result = match id {
            Some(id) => database.delete_clipboard(id),
            None => database.clear_clipboard(),
        };
        if let Err(error) = result {
            self.failed(&error);
            session.database = None;
            return Err(
                "Could not delete clipboard history. Restart TinyDash to try again.".into(),
            );
        }
        state.clipboard.invalidate();
        match id {
            Some(id) => search.clipboard.remove(id),
            None => search.clipboard = ClipboardProvider::default(),
        }
        search.forget_clipboard_pins(id);
        drop(search);
        super::clipboard::changed(app);
        Ok(())
    }

    pub fn clear_unpinned_clipboard(&self, app: &AppHandle) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        let mut search = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?;
        let database = session
            .database
            .as_ref()
            .ok_or("Clipboard storage is unavailable. Restart TinyDash to try again.")?;
        let removed = match database.clear_unpinned_clipboard() {
            Ok(removed) => removed,
            Err(error) => {
                self.failed(&error);
                session.database = None;
                return Err(
                    "Could not clear unpinned clipboard history. Restart TinyDash to try again."
                        .into(),
                );
            }
        };
        state.clipboard.invalidate();
        search.clipboard.remove_many(&removed);
        for id in removed {
            search.forget_clipboard_pins(Some(id));
        }
        drop(search);
        super::clipboard::changed(app);
        Ok(())
    }

    pub fn warning(&self) -> Option<String> {
        self.warning.lock().ok().and_then(|warning| warning.clone())
    }

    fn failed(&self, error: &dyn std::fmt::Display) {
        tracing::warn!(%error, "State storage is unavailable");
        if let Ok(mut warning) = self.warning.lock() {
            *warning = Some("Storage is unavailable. Clipboard capture is paused. Ranking changes will be lost when TinyDash quits.".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_copy_is_not_captured_by_the_copy_action_or_monitor() {
        let mut observed = Observation::default();
        assert!(!observed.copied("a-generated-secret".into(), true));
        assert_eq!(observed.changed(Some("a-generated-secret".into())), None);
        assert_eq!(observed.changed(Some("a-generated-secret".into())), None);
        assert_eq!(
            observed.changed(Some("ordinary text".into())),
            Some("ordinary text".into())
        );
        assert!(observed.copied("https://example.com".into(), false));
    }

    #[test]
    fn upstream_clear_deletes_only_the_recent_capture_of_the_cleared_value() {
        let mut observed = Observation::default();
        observed.changed(Some("copied from a password manager".into()));
        observed.captured = Some((7, 1_000));
        assert_eq!(observed.cleared(1_000 + CLEARED_CAPTURE_SECONDS), Some(7));
        // The clear also forgets the value, so a new copy is captured again.
        assert_eq!(observed.cleared(1_001), None);
        assert_eq!(
            observed.changed(Some("copied from a password manager".into())),
            Some("copied from a password manager".into())
        );

        observed.captured = Some((8, 1_000));
        assert_eq!(observed.cleared(1_001 + CLEARED_CAPTURE_SECONDS), None);

        // A later value replaces the capture. Clearing it keeps entry 9.
        observed.changed(Some("A".into()));
        observed.captured = Some((9, 1_000));
        observed.changed(Some("B".into()));
        assert_eq!(observed.cleared(1_001), None);

        // Copies from TinyDash are never recorded as captures.
        assert!(observed.copied("C".into(), false));
        assert_eq!(observed.cleared(1_001), None);
    }

    #[test]
    fn a_secret_forgets_the_previous_value() {
        let mut observed = Observation::default();
        assert_eq!(observed.changed(Some("A".into())), Some("A".into()));
        observed.captured = Some((1, 1_000));
        // capture() maps Observed::Secret to no text.
        assert_eq!(observed.changed(None), None);
        assert_eq!(observed.cleared(1_001), None);
        assert_eq!(observed.changed(Some("A".into())), Some("A".into()));
    }

    #[test]
    fn ignores_consecutive_values_but_accepts_returning_text() {
        let mut observed = Observation::default();
        assert_eq!(observed.changed(Some("A".into())), Some("A".into()));
        assert_eq!(observed.changed(Some("A".into())), None);
        // Deleting history does not reset the observed value.
        assert_eq!(observed.changed(Some("A".into())), None);
        assert_eq!(observed.changed(Some("B".into())), Some("B".into()));
        assert_eq!(observed.changed(Some("A".into())), Some("A".into()));
        assert_eq!(observed.changed(None), None);
        assert_eq!(observed.changed(Some("A".into())), Some("A".into()));
        assert_eq!(observed.changed(Some(" \n".into())), None);
    }
}
