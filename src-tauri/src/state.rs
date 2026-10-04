use std::{path::PathBuf, sync::Mutex};

use chrono::Local;

use crate::{
    error::Result,
    features::{
        apps::AppIndex, clipboard::ClipboardHistory, currency::Rates, emoji::EmojiIndex,
        files::FileIndex, library::Library,
    },
    refresh::Freshness,
    search::{
        Snapshot,
        usage::{Pins, Usage},
    },
    settings::Settings,
    shared::Shared,
    store::Store,
};

/// Everything the app keeps in memory, managed by Tauri. Indexes are
/// [`Shared`] snapshots: background work builds a new value and swaps it in.
pub struct State {
    pub settings: Shared<Settings>,
    pub store: Store,
    pub apps: Shared<AppIndex>,
    pub files: Shared<FileIndex>,
    pub clipboard: Shared<ClipboardHistory>,
    pub library: Shared<Library>,
    pub emoji: Shared<EmojiIndex>,
    pub usage: Shared<Usage>,
    pub pins: Shared<Pins>,
    pub rates: Shared<Option<Rates>>,
    pub freshness: Freshness,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub home_dir: PathBuf,
    /// Problems found at startup, shown once in the launcher.
    pub warnings: Mutex<Vec<String>>,
}

impl State {
    /// Load saved data. A broken database part is reported, not fatal.
    pub fn new(
        settings: Settings,
        store: Store,
        dirs: [PathBuf; 3],
        mut warnings: Vec<String>,
    ) -> Self {
        let mut load = |what: &str, error: crate::error::Error| {
            warnings.push(format!("Could not load {what}: {error}"));
        };
        let usage = store.usage().unwrap_or_else(|e| {
            load("usage", e);
            Usage::default()
        });
        let pins = store.pins().unwrap_or_else(|e| {
            load("pins", e);
            Pins::default()
        });
        let clipboard = store.clipboard_history().unwrap_or_else(|e| {
            load("clipboard history", e);
            Vec::new()
        });
        let library = store.library().unwrap_or_else(|e| {
            load("snippets", e);
            Vec::new()
        });
        let rates = store.rates().unwrap_or_else(|e| {
            load("exchange rates", e);
            None
        });
        let [config_dir, data_dir, home_dir] = dirs;
        Self {
            emoji: Shared::new(EmojiIndex::new(&settings.emoji_languages)),
            settings: Shared::new(settings),
            store,
            apps: Shared::default(),
            files: Shared::default(),
            clipboard: Shared::new(ClipboardHistory::new(clipboard)),
            library: Shared::new(Library::new(library)),
            usage: Shared::new(usage),
            pins: Shared::new(pins),
            rates: Shared::new(rates),
            freshness: Freshness::default(),
            config_dir,
            data_dir,
            home_dir,
            warnings: Mutex::new(warnings),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            settings: self.settings.get(),
            apps: self.apps.get(),
            files: self.files.get(),
            clipboard: self.clipboard.get(),
            library: self.library.get(),
            emoji: self.emoji.get(),
            usage: self.usage.get(),
            pins: self.pins.get(),
            rates: self.rates.get(),
            now: Local::now(),
        }
    }

    pub fn reload_clipboard(&self) -> Result<()> {
        self.clipboard
            .set(ClipboardHistory::new(self.store.clipboard_history()?));
        Ok(())
    }

    pub fn reload_library(&self) -> Result<()> {
        self.library.set(Library::new(self.store.library()?));
        Ok(())
    }

    pub fn warn(&self, message: impl Into<String>) {
        let message = message.into();
        tracing::warn!("{message}");
        self.warnings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(message);
    }
}
