use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use super::LauncherState;
use super::search::SearchManager;
use crate::{
    db::Database,
    error::Error,
    providers::clipboard::{ClipboardProvider, valid_text},
    ranking,
};

#[derive(Default)]
struct Session {
    database: Option<Database>,
    observed: Observation,
}

#[derive(Default)]
struct Observation(Option<String>);

impl Observation {
    fn changed(&mut self, text: Option<String>) -> Option<String> {
        let text = text.filter(|text| valid_text(text));
        if self.0 == text {
            return None;
        }
        self.0.clone_from(&text);
        text
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
                let database = Database::open(&path)?;
                let usage = database.load_usage()?;
                database
                    .prune_clipboard(app.state::<LauncherState>().settings.clipboard_limit())?;
                let clipboard = ClipboardProvider::new(database.load_clipboard()?);
                let mut search = search.lock().map_err(|_| Error::IndexUnavailable)?;
                search.set_usage(usage);
                search.clipboard = clipboard;
                Ok(database)
            })();
            Mutex::new(Session {
                database: match loaded {
                    Ok(database) => Some(database),
                    Err(error) => {
                        self.failed(&error);
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
        if let Some(store) = database.database.as_ref() {
            if let Err(error) = store.save_usage(id, usage) {
                self.failed(&error);
                // Continue with session-only ranking. Do not retry or flood logs.
                database.database = None;
            }
        }
    }

    pub fn capture(&self, app: &AppHandle, text: Option<String>, generation: u64) {
        let state = app.state::<LauncherState>();
        let Ok(mut session) = self.session(app, &state.search).lock() else {
            return;
        };
        // A delete, clear, or copy invalidates reads that were already in flight.
        if generation != state.clipboard.generation() {
            return;
        }
        let Some(text) = session.observed.changed(text) else {
            return;
        };
        self.save_clipboard(app, &mut session, &text);
    }

    fn save_clipboard(&self, app: &AppHandle, session: &mut Session, text: &str) {
        let state = app.state::<LauncherState>();
        if !state.settings.clipboard_history_enabled || !valid_text(text) {
            return;
        }
        let Some(database) = session.database.as_mut() else {
            return;
        };
        match database.capture_clipboard(text, ranking::now(), state.settings.clipboard_limit()) {
            Ok((entry, removed)) => {
                let indexed = entry.into();
                if let Ok(mut search) = state.search.lock() {
                    search.clipboard.update(indexed, &removed);
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
        app.clipboard()
            .write_text(text.clone())
            .map_err(|error| format!("Could not copy to the clipboard: {error}"))?;
        state.clipboard.invalidate();
        let changed = session.observed.changed(Some(text.clone())).is_some();
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

    pub fn delete_clipboard(&self, app: &AppHandle, id: Option<i64>) -> Result<(), String> {
        let state = app.state::<LauncherState>();
        let mut session = self
            .session(app, &state.search)
            .lock()
            .map_err(|_| "Clipboard storage is unavailable.")?;
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
        let mut search = state
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?;
        match id {
            Some(id) => search.clipboard.remove(id),
            None => search.clipboard = ClipboardProvider::default(),
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
