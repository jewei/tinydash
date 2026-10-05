use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chrono::Local;

use crate::{
    error::Result,
    features::{
        apps::AppIndex,
        clipboard::{ClipboardHistory, Content},
        currency::Rates,
        emoji::EmojiIndex,
        files::FileIndex,
        library::Library,
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

/// Folders the OS assigns to the app and the user.
pub struct Dirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub home: PathBuf,
}

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
    pub dirs: Dirs,
    /// Held while settings change or the shortcut pauses, so two changes
    /// never interleave their effects on the OS.
    pub settings_change: Mutex<()>,
    /// Problems found at startup, shown once in the launcher.
    pub warnings: Mutex<Vec<String>>,
}

impl State {
    /// Load saved data. A broken database part is reported, not fatal.
    pub fn new(settings: Settings, store: Store, dirs: Dirs, mut warnings: Vec<String>) -> Self {
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
            dirs,
            settings_change: Mutex::new(()),
            warnings: Mutex::new(warnings),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let settings = self.settings.get();
        // While history is off, saved entries stay in the database but are
        // hidden, so no result can act on an entry the user cannot see.
        let clipboard = if settings.clipboard_history_enabled {
            self.clipboard.get()
        } else {
            Arc::default()
        };
        Snapshot {
            settings,
            apps: self.apps.get(),
            files: self.files.get(),
            clipboard,
            library: self.library.get(),
            emoji: self.emoji.get(),
            usage: self.usage.get(),
            pins: self.pins.get(),
            rates: self.rates.get(),
            now: Local::now(),
        }
    }

    /// The full content of a saved clipboard entry, hidden like search
    /// results while history is off.
    pub fn clip(&self, id: i64) -> Result<Option<Content>> {
        if !self.settings.get().clipboard_history_enabled {
            return Ok(None);
        }
        self.store.clip(id)
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

    /// A state with an empty in-memory database and no folders.
    #[cfg(test)]
    pub fn for_tests(settings: Settings) -> Self {
        let dirs = Dirs {
            config: PathBuf::new(),
            data: PathBuf::new(),
            home: PathBuf::new(),
        };
        Self::new(settings, Store::in_memory(), dirs, Vec::new())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{self, Category};

    fn state_with_clip(clipboard_history_enabled: bool) -> State {
        let state = State::for_tests(Settings {
            clipboard_history_enabled,
            ..Settings::default()
        });
        state
            .store
            .save_clip(&Content::Text("secret note".into()), 1, 10)
            .unwrap();
        state.reload_clipboard().unwrap();
        state
    }

    fn titles(state: &State, query: &str, category: Category) -> Vec<String> {
        search::search(&state.snapshot(), query, category)
            .into_iter()
            .filter(|result| result.id.starts_with("clip:"))
            .map(|result| result.title)
            .collect()
    }

    #[test]
    fn hides_saved_clips_while_history_is_off() {
        let on = state_with_clip(true);
        assert_eq!(titles(&on, "", Category::Clipboard), ["secret note"]);
        assert_eq!(titles(&on, "secret", Category::All), ["secret note"]);

        let off = state_with_clip(false);
        for (query, category) in [
            ("", Category::Clipboard),
            ("secret", Category::Clipboard),
            ("secret", Category::All),
        ] {
            assert!(titles(&off, query, category).is_empty());
        }
    }

    #[test]
    fn serves_saved_content_only_while_history_is_on() {
        for enabled in [true, false] {
            let state = state_with_clip(enabled);
            let id = state.store.clipboard_history().unwrap()[0].id;
            assert_eq!(state.clip(id).unwrap().is_some(), enabled);
        }
    }
}
