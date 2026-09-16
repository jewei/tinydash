use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use super::search::SearchManager;
use crate::{db::Database, error::Error, ranking};

#[derive(Default)]
pub struct History {
    database: OnceLock<Mutex<Option<Database>>>,
    warning: Mutex<Option<String>>,
}

impl History {
    // Call only on a blocking worker. Search itself never accesses SQLite or
    // waits on this mutex. OnceLock also orders any action during startup.
    pub fn initialize(
        &self,
        app: &AppHandle,
        search: &Mutex<SearchManager>,
    ) -> &Mutex<Option<Database>> {
        self.database.get_or_init(|| {
            let loaded = (|| -> anyhow::Result<Database> {
                let path = app.path().app_data_dir()?.join("tinydash.sqlite3");
                let database = Database::open(&path)?;
                let usage = database.load_usage()?;
                search
                    .lock()
                    .map_err(|_| Error::IndexUnavailable)?
                    .set_usage(usage);
                Ok(database)
            })();
            Mutex::new(match loaded {
                Ok(database) => Some(database),
                Err(error) => {
                    self.failed(&error);
                    None
                }
            })
        })
    }

    pub fn record(&self, app: &AppHandle, search: &Mutex<SearchManager>, id: &str) {
        let database = self.initialize(app, search);
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
        if let Some(store) = database.as_ref() {
            if let Err(error) = store.save_usage(id, usage) {
                self.failed(&error);
                // Continue with session-only ranking. Do not retry or flood logs.
                *database = None;
            }
        }
    }

    pub fn warning(&self) -> Option<String> {
        self.warning.lock().ok().and_then(|warning| warning.clone())
    }

    fn failed(&self, error: &dyn std::fmt::Display) {
        tracing::warn!(%error, "Usage persistence is unavailable; keeping session ranking");
        if let Ok(mut warning) = self.warning.lock() {
            *warning = Some("Usage history could not be saved. Ranking changes will be lost when TinyDash quits.".into());
        }
    }
}
