use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Health {
    #[default]
    Uninitialized,
    Healthy,
    Busy,
    RecoveryRequired,
    Incompatible,
}

#[derive(Default)]
struct Session {
    database: Option<Database>,
    health: Health,
    warning: Arc<Mutex<Option<String>>>,
    // Before loading these are session-only increments; afterwards they are
    // absolute counts. A successful initialization merges them exactly once.
    pending_usage: HashMap<String, ranking::Usage>,
    observed: Observation,
}

impl Session {
    fn failed(&mut self, error: &anyhow::Error) {
        let database_error = error.downcast_ref::<crate::db::Error>();
        let health = match database_error {
            Some(error) if error.is_transient() => Health::Busy,
            Some(crate::db::Error::NewerSchema { .. }) => Health::Incompatible,
            _ => Health::RecoveryRequired,
        };
        if self.health != health {
            tracing::warn!(%error, ?health, "State storage is unavailable");
        }
        self.health = health;
        if health != Health::Busy {
            self.database = None;
        }
        let message = match (health, database_error) {
            (Health::Busy, _) => {
                "Local storage is busy. TinyDash will retry on the next storage operation. Failed clipboard deletes remain visible; try them again."
            }
            (Health::Incompatible, _) => {
                "Your saved data needs a compatible TinyDash version. Your database was kept. Use a version that supports its schema."
            }
            (_, Some(crate::db::Error::Backup(_))) => {
                "Could not save a recovery backup. Your database was not migrated. Check disk space and folder access, then restart TinyDash."
            }
            _ => {
                "Storage needs recovery. Clipboard capture is paused. Ranking changes are session-only. Your database was kept; check disk space and folder access or follow data recovery, then restart TinyDash."
            }
        };
        if let Ok(mut warning) = self.warning.lock() {
            *warning = Some(message.into());
        }
    }

    fn healthy(&mut self) {
        self.health = Health::Healthy;
        if let Ok(mut warning) = self.warning.lock() {
            *warning = None;
        }
    }

    // Called under the storage mutex, never the search mutex. Busy initialization
    // is retried by the next storage operation; permanent failures are not reopened.
    fn initialize(
        &mut self,
        path: &Path,
        settings: &Path,
        search: &Mutex<SearchManager>,
        limit: Option<usize>,
    ) -> anyhow::Result<Result<Option<crate::currency::Rates>, crate::db::Error>> {
        if self.database.is_some() || !matches!(self.health, Health::Uninitialized | Health::Busy) {
            return Ok(Ok(None));
        }
        let loaded = (|| -> anyhow::Result<_> {
            let mut database = Database::open_with_settings(path, settings)?;
            let mut usage = database.load_usage()?;
            let mut pending = self.pending_usage.clone();
            for (id, increment) in &mut pending {
                let saved = usage.entry(id.clone()).or_default();
                saved.count = saved.count.saturating_add(increment.count);
                saved.last_used_at = saved.last_used_at.max(increment.last_used_at);
                *increment = *saved;
            }
            let pins = database.load_pins()?;
            if let Some(limit) = limit {
                database.prune_clipboard(limit)?;
            }
            let clipboard = ClipboardProvider::new(database.load_clipboard()?);
            let rates = database.load_rates();
            database.save_usage_batch(&pending)?;
            {
                let mut search = search.lock().map_err(|_| Error::IndexUnavailable)?;
                search.set_usage(usage);
                search.set_pins(pins);
                search.clipboard = clipboard;
            }
            self.pending_usage.clear();
            Ok((database, rates))
        })();
        match loaded {
            Ok((database, rates)) => {
                self.database = Some(database);
                self.healthy();
                Ok(rates)
            }
            Err(error) => {
                self.failed(&error);
                Err(error)
            }
        }
    }

    // SQLite waits at most 250 ms per attempt. No immediate retry loop: the next
    // storage operation retries on this connection, without reloading search.
    fn with_database<T>(
        &mut self,
        operation: impl FnOnce(&mut Database) -> Result<T, crate::db::Error>,
    ) -> anyhow::Result<T> {
        let database = self.database.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Local storage is unavailable. Check the storage warning for recovery instructions."
            )
        })?;
        let result = database
            .save_usage_batch(&self.pending_usage)
            .and_then(|()| {
                self.pending_usage.clear();
                operation(database)
            });
        match result {
            Ok(value) => {
                self.healthy();
                Ok(value)
            }
            Err(error) => {
                let error = anyhow::Error::from(error);
                self.failed(&error);
                Err(error)
            }
        }
    }

    fn record(&mut self, search: &Mutex<SearchManager>, id: &str, now: i64) {
        let usage = match search.lock() {
            Ok(mut search) => search.record_usage(id, now),
            Err(error) => {
                tracing::warn!(%error, "Could not update usage ranking");
                return;
            }
        };
        self.pending_usage.insert(id.to_owned(), usage);
        let _ = self.with_database(|_| Ok(()));
    }

    fn delete(&mut self, search: &Mutex<SearchManager>, deletion: Deletion) -> anyhow::Result<()> {
        let removed = self.with_database(|database| match deletion {
            Deletion::Entry(id) => database.delete_clipboard(id).map(|()| vec![id]),
            Deletion::All => database.clear_clipboard().map(|()| Vec::new()),
            Deletion::Unpinned => database.clear_unpinned_clipboard(),
        })?;
        publish_deletion(search, deletion, removed).map_err(Into::into)
    }

    fn capture(
        &mut self,
        search: &Mutex<SearchManager>,
        text: &str,
        now: i64,
        limit: usize,
    ) -> anyhow::Result<i64> {
        let (entry, removed) =
            self.with_database(|database| database.capture_clipboard(text, now, limit))?;
        let id = entry.id;
        search
            .lock()
            .map_err(|_| Error::IndexUnavailable)?
            .clipboard
            .update(entry.into(), &removed);
        Ok(id)
    }
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
    warning: Arc<Mutex<Option<String>>>,
}

impl Storage {
    // Call only on a blocking worker. Search itself never accesses SQLite or
    // waits on this mutex. OnceLock also orders any action during startup.
    fn session(&self, app: &AppHandle, search: &Mutex<SearchManager>) -> &Mutex<Session> {
        let mutex = self.database.get_or_init(|| {
            Mutex::new(Session {
                warning: Arc::clone(&self.warning),
                ..Session::default()
            })
        });
        if let Ok(mut session) = mutex.lock()
            && session.database.is_none()
            && matches!(session.health, Health::Uninitialized | Health::Busy)
        {
            let loaded = (|| -> anyhow::Result<_> {
                let path = app.path().app_data_dir()?.join("tinydash.sqlite3");
                let settings_path = app.path().app_config_dir()?.join("settings.json");
                let settings = app.state::<LauncherState>().settings();
                // Fallback first-use settings must not prune saved history.
                session.initialize(
                    &path,
                    &settings_path,
                    search,
                    settings
                        .clipboard_history_decided
                        .then(|| settings.clipboard_limit()),
                )
            })();
            match loaded {
                Ok(Ok(Some(rates))) => {
                    app.state::<LauncherState>().currency.loaded(&rates);
                    if let Ok(mut search) = search.lock() {
                        search.set_rates(rates);
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    tracing::warn!(%error, "Could not load cached currency rates");
                    app.state::<LauncherState>().currency.warning(Some(
                        "Could not load saved currency rates. Refresh rates in Calculator mode."
                            .into(),
                    ));
                }
                Err(error) => session.failed(&error),
            }
        }
        mutex
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
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Pin storage is unavailable.")?;
        let key = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .pin_key(id, category)
            .map_err(|error| error.to_string())?;
        session
            .with_database(|database| database.set_pinned(&key, category, pinned))
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
        let entries = session.with_database(|database| {
            database.prune_clipboard(state.settings().clipboard_limit())?;
            database.load_clipboard()
        });
        if let Ok(entries) = entries
            && let Ok(mut search) = state.search.lock()
        {
            search.clipboard = ClipboardProvider::new(entries);
        }
        super::clipboard::changed(app);
    }

    pub fn save_rates(
        &self,
        app: &AppHandle,
        rates: &crate::currency::Rates,
    ) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|error| error.to_string())?;
        session
            .with_database(|database| database.save_rates(rates))
            .map_err(|error| error.to_string())
    }

    pub fn record(&self, app: &AppHandle, search: &Mutex<SearchManager>, id: &str) {
        // Serialize writes so an older action cannot overwrite a newer count.
        if let Ok(mut session) = self.session(app, search).lock() {
            session.record(search, id, ranking::now());
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
        match session.with_database(|database| database.delete_unpinned_clipboard(id)) {
            Ok(false) => {}
            Ok(true) => {
                if let Ok(mut search) = state.search.lock() {
                    search.clipboard.remove(id);
                }
                super::clipboard::changed(app);
            }
            Err(_) => super::clipboard::changed(app),
        }
    }

    fn save_clipboard(&self, app: &AppHandle, session: &mut Session, text: &str) -> Option<i64> {
        let state = app.state::<LauncherState>();
        if !state.settings().clipboard_history_enabled || !valid_text(text) {
            return None;
        }
        let result = session.capture(
            &state.search,
            text,
            ranking::now(),
            state.settings().clipboard_limit(),
        );
        super::clipboard::changed(app);
        result.ok()
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
            match session.with_database(|database| database.touch_clipboard(id, ranking::now())) {
                Ok(Some(entry)) => {
                    let indexed = entry.into();
                    if let Ok(mut search) = state.search.lock() {
                        search.clipboard.update(indexed, &[]);
                    }
                    super::clipboard::changed(app);
                }
                Ok(None) => {}
                Err(_) => super::clipboard::changed(app),
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
        self.delete_entries(
            app,
            id.map_or(Deletion::All, Deletion::Entry),
            "Could not delete clipboard history. If storage is busy, try again.",
        )
    }

    pub fn clear_unpinned_clipboard(&self, app: &AppHandle) -> Result<(), String> {
        self.delete_entries(
            app,
            Deletion::Unpinned,
            "Could not clear unpinned clipboard history. If storage is busy, try again.",
        )
    }

    fn delete_entries(
        &self,
        app: &AppHandle,
        deletion: Deletion,
        failure: &str,
    ) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        // The storage lock orders this with copies, captures, and pins.
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
        session
            .delete(&state.search, deletion)
            .map_err(|error| format!("{failure} {error}"))?;
        state.clipboard.invalidate();
        drop(session);
        super::clipboard::changed(app);
        Ok(())
    }

    pub fn warning(&self) -> Option<String> {
        self.warning.lock().ok().and_then(|warning| warning.clone())
    }
}

#[derive(Clone, Copy)]
enum Deletion {
    Entry(i64),
    All,
    Unpinned,
}

// Called only after durable deletion. SQLite never waits under the search lock.
fn publish_deletion(
    search: &Mutex<SearchManager>,
    deletion: Deletion,
    removed: Vec<i64>,
) -> Result<(), Error> {
    let mut search = search.lock().map_err(|_| Error::IndexUnavailable)?;
    if let Deletion::All = deletion {
        search.clipboard = ClipboardProvider::default();
        search.forget_clipboard_pins(None);
    } else {
        search.clipboard.remove_many(&removed);
        for id in removed {
            search.forget_clipboard_pins(Some(id));
        }
    }
    Ok(())
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
    fn search_stays_available_while_a_delete_waits_for_sqlite() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("state.sqlite3");
        let mut database = Database::open(&path).expect("open");
        let id = database
            .capture_clipboard("kept", 1, 100)
            .expect("capture")
            .0
            .id;
        let key = format!("clipboard:{id}");
        let search = Mutex::new(SearchManager::default());
        search.lock().expect("search").clipboard =
            ClipboardProvider::new(database.load_clipboard().expect("load"));
        let other = rusqlite::Connection::open(&path).expect("other connection");
        other.execute_batch("BEGIN IMMEDIATE").expect("lock");

        let search = &search;
        let mut session = Session {
            database: Some(database),
            health: Health::Healthy,
            ..Session::default()
        };
        let mut session = std::thread::scope(|scope| {
            let deleting = scope.spawn(move || {
                let result = session.delete(search, Deletion::Entry(id));
                (result, session)
            });
            // SQLite waits for its busy timeout. Search must not wait too.
            while !deleting.is_finished() {
                drop(search.try_lock().expect("search is free during the write"));
                std::thread::yield_now();
            }
            let (result, session) = deleting.join().expect("delete");
            assert!(
                result
                    .unwrap_err()
                    .downcast_ref::<crate::db::Error>()
                    .unwrap()
                    .is_transient()
            );
            assert_eq!(session.health, Health::Busy);
            assert!(session.database.is_some());
            assert!(
                session
                    .warning
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .contains("busy")
            );
            session
        });
        // A failed write keeps the entry visible.
        assert!(search.lock().expect("search").clipboard_entry(&key).is_ok());

        other.execute_batch("ROLLBACK").expect("unlock");
        assert!(session.delete(search, Deletion::Entry(id)).is_ok());
        assert_eq!(session.health, Health::Healthy);
        assert!(session.warning.lock().unwrap().is_none());
        assert!(
            search
                .lock()
                .expect("search")
                .clipboard_entry(&key)
                .is_err()
        );
        assert!(
            session
                .database
                .as_ref()
                .unwrap()
                .load_clipboard()
                .expect("load")
                .is_empty()
        );
    }

    #[test]
    fn session_recovers_capture_and_usage_after_external_contention() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let settings = directory.path().join("settings.json");
        let search = Mutex::new(SearchManager::default());
        let mut session = Session::default();
        session
            .initialize(&path, &settings, &search, Some(100))
            .unwrap()
            .unwrap();
        session.record(&search, "app:one", 100);
        let kept = session.capture(&search, "kept", 100, 100).unwrap();
        let other = rusqlite::Connection::open(&path).unwrap();
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        // Exercise capture's own write failure, before a pending usage flush can fail.
        let search_ref = &search;
        session = std::thread::scope(|scope| {
            let capturing = scope.spawn(move || {
                assert!(
                    session
                        .capture(search_ref, "missed capture", 101, 100)
                        .is_err()
                );
                session
            });
            while !capturing.is_finished() {
                drop(
                    search
                        .try_lock()
                        .expect("search is free while capture waits"),
                );
                std::thread::yield_now();
            }
            capturing.join().unwrap()
        });
        assert_eq!(session.health, Health::Busy);
        session.record(&search, "app:one", 101);
        session.record(&search, "app:one", 102);
        session.record(&search, "app:two", 103);
        assert!(session.capture(&search, "missed", 104, 100).is_err());
        assert_eq!(session.health, Health::Busy);
        assert_eq!(session.pending_usage["app:one"].count, 3);
        assert_eq!(
            session.database.as_ref().unwrap().load_usage().unwrap()["app:one"].count,
            1
        );
        assert!(
            search
                .lock()
                .unwrap()
                .clipboard_entry(&format!("clipboard:{kept}"))
                .is_ok()
        );
        other.execute_batch("ROLLBACK").unwrap();

        // An unrelated capture retries storage and persists every pending count.
        let captured = session.capture(&search, "recovered", 105, 100).unwrap();
        assert_eq!(session.health, Health::Healthy);
        assert!(session.pending_usage.is_empty());
        assert!(session.warning.lock().unwrap().is_none());
        assert!(
            search
                .lock()
                .unwrap()
                .clipboard_entry(&format!("clipboard:{captured}"))
                .is_ok()
        );
        session.record(&search, "app:one", 106);
        drop(session);
        let database = Database::open(&path).unwrap();
        let usage = database.load_usage().unwrap();
        assert_eq!(usage["app:one"].count, 4);
        assert_eq!(usage["app:two"].count, 1);
        let entries = database.load_clipboard().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "recovered");
    }

    #[test]
    fn busy_initialization_merges_session_usage_once_after_unlock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let settings = directory.path().join("settings.json");
        Database::open(&path)
            .unwrap()
            .save_usage(
                "app:kept",
                ranking::Usage {
                    count: 5,
                    last_used_at: 50,
                },
            )
            .unwrap();
        let other = rusqlite::Connection::open(&path).unwrap();
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        let search = Mutex::new(SearchManager::default());
        let mut session = Session::default();
        assert!(
            session
                .initialize(&path, &settings, &search, Some(100))
                .is_err()
        );
        assert_eq!(session.health, Health::Busy);
        assert!(session.database.is_none());
        session.record(&search, "app:kept", 100);
        session.record(&search, "app:kept", 101);
        session.record(&search, "app:new", 102);
        // A second failed initialization must not lose or multiply increments.
        assert!(
            session
                .initialize(&path, &settings, &search, Some(100))
                .is_err()
        );
        other.execute_batch("ROLLBACK").unwrap();
        session
            .initialize(&path, &settings, &search, Some(100))
            .unwrap()
            .unwrap();
        session
            .initialize(&path, &settings, &search, Some(100))
            .unwrap()
            .unwrap();
        session.record(&search, "app:kept", 103);
        let usage = session.database.as_ref().unwrap().load_usage().unwrap();
        assert_eq!(
            usage["app:kept"],
            ranking::Usage {
                count: 8,
                last_used_at: 103
            }
        );
        assert_eq!(usage["app:new"].count, 1);
        assert_eq!(session.health, Health::Healthy);
    }

    #[test]
    fn failed_clear_keeps_entries_and_pins_until_retry_commits() {
        for deletion in [Deletion::All, Deletion::Unpinned] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("state.sqlite3");
            let mut database = Database::open(&path).unwrap();
            let pinned = database.capture_clipboard("pinned", 1, 100).unwrap().0.id;
            let unpinned = database.capture_clipboard("unpinned", 2, 100).unwrap().0.id;
            let key = format!("clipboard:{pinned}");
            database
                .set_pinned(&key, SearchMode::Clipboard, true)
                .unwrap();
            drop(database);
            let search = Mutex::new(SearchManager::default());
            let mut session = Session::default();
            session
                .initialize(
                    &path,
                    &path.with_file_name("settings.json"),
                    &search,
                    Some(100),
                )
                .unwrap()
                .unwrap();
            let other = rusqlite::Connection::open(&path).unwrap();
            other.execute_batch("BEGIN IMMEDIATE").unwrap();
            assert!(session.delete(&search, deletion).is_err());
            assert!(search.lock().unwrap().clipboard_entry(&key).is_ok());
            assert!(
                search
                    .lock()
                    .unwrap()
                    .clipboard_entry(&format!("clipboard:{unpinned}"))
                    .is_ok()
            );
            assert!(
                !session
                    .database
                    .as_ref()
                    .unwrap()
                    .load_pins()
                    .unwrap()
                    .is_empty()
            );
            other.execute_batch("ROLLBACK").unwrap();
            session.delete(&search, deletion).unwrap();
            assert!(
                search
                    .lock()
                    .unwrap()
                    .clipboard_entry(&format!("clipboard:{unpinned}"))
                    .is_err()
            );
            let keeps_pin = matches!(deletion, Deletion::Unpinned);
            assert_eq!(
                search.lock().unwrap().clipboard_entry(&key).is_ok(),
                keeps_pin
            );
            let database = session.database.as_ref().unwrap();
            assert_eq!(
                database.load_clipboard().unwrap().len(),
                usize::from(keeps_pin)
            );
            assert_eq!(!database.load_pins().unwrap().is_empty(), keeps_pin);
        }
    }

    #[test]
    fn permanent_failures_preserve_files_and_do_not_retry_in_session() {
        for newer in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("state.sqlite3");
            if newer {
                let connection = rusqlite::Connection::open(&path).unwrap();
                connection.pragma_update(None, "user_version", 999).unwrap();
            } else {
                std::fs::write(&path, "not a database").unwrap();
            }
            let original = std::fs::read(&path).unwrap();
            let search = Mutex::new(SearchManager::default());
            let mut session = Session::default();
            assert!(
                session
                    .initialize(&path, &path.with_file_name("settings.json"), &search, None)
                    .is_err()
            );
            assert_eq!(
                session.health,
                if newer {
                    Health::Incompatible
                } else {
                    Health::RecoveryRequired
                }
            );
            session.record(&search, "app:session-only", 100);
            assert!(session.capture(&search, "not saved", 101, 100).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            // Even replacing the file must not silently reopen a permanent failure.
            std::fs::remove_file(&path).unwrap();
            Database::open(&path).unwrap();
            session
                .initialize(&path, &path.with_file_name("settings.json"), &search, None)
                .unwrap()
                .unwrap();
            assert!(session.database.is_none());
            assert!(session.warning.lock().unwrap().is_some());
        }
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
