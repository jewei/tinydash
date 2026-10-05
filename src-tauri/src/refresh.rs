//! Keeps indexes and exchange rates fresh without background polling.
//!
//! The file watcher only marks an index dirty. Work happens when the
//! launcher opens (or settings change), on a blocking worker, and the new
//! index replaces the old one in one swap.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager};

use crate::{
    events,
    features::{apps::AppIndex, currency, files, files::FileIndex},
    platform,
    search::id::Source,
    state::State,
};

/// Rescan even without watcher events after this long, in case events were lost.
const MAX_INDEX_AGE: Duration = Duration::from_secs(15 * 60);
/// Wait this long after a failed rate download before trying again.
const RATE_RETRY: Duration = Duration::from_secs(60 * 60);

#[derive(Default)]
pub struct Freshness {
    pub apps: Slot,
    pub files: Slot,
    rates_attempt: Mutex<Option<Instant>>,
}

impl Freshness {
    /// Which sources have their items, read once. A source that a scan
    /// fills looks empty until its first scan finishes.
    pub fn ready(&self) -> impl Fn(Source) -> bool + use<> {
        let (apps, files) = (self.apps.built(), self.files.built());
        move |source| match source {
            Source::App => apps,
            Source::File => files,
            Source::Clip | Source::Snippet | Source::Link | Source::Emoji | Source::System => true,
        }
    }
}

pub struct Slot {
    dirty: AtomicBool,
    busy: AtomicBool,
    finished: Mutex<Option<Instant>>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            dirty: AtomicBool::new(true),
            busy: AtomicBool::new(false),
            finished: Mutex::new(None),
        }
    }
}

impl Slot {
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Whether the index was built at least once. Before that, every item
    /// of this source looks missing.
    fn built(&self) -> bool {
        self.finished
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Dirty or old, and not already being rebuilt.
    fn needs_work(&self) -> bool {
        if self.busy.load(Ordering::SeqCst) {
            return false;
        }
        self.dirty.load(Ordering::SeqCst)
            || self
                .finished
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none_or(|at| at.elapsed() > MAX_INDEX_AGE)
    }
}

/// Called whenever the launcher opens.
pub fn on_launcher_shown(app: &AppHandle) {
    let state = app.state::<State>();
    if state.freshness.apps.needs_work() {
        apps(app);
    }
    if state.freshness.files.needs_work() {
        files(app);
    }
    rates(app, false);
}

pub fn apps(app: &AppHandle) {
    rebuild(
        app,
        |state| &state.freshness.apps,
        |state| {
            let index = AppIndex::new(platform::discover_apps());
            tracing::info!(apps = index.len(), "Indexed applications");
            state.apps.set(index);
        },
    );
}

pub fn files(app: &AppHandle) {
    rebuild(
        app,
        |state| &state.freshness.files,
        |state| {
            let settings = state.settings.get();
            let folders = settings.file_folders(&state.dirs.home);
            let index = FileIndex::scan(
                &folders,
                &settings.file_search_excluded_dirs,
                platform::PACKAGE_EXTENSIONS,
            );
            if index.truncated {
                tracing::warn!(
                    limit = files::LIMIT,
                    "The file index is full; index fewer folders"
                );
            }
            tracing::info!(files = index.len(), "Indexed files");
            state.files.set(index);
        },
    );
}

/// Run `work` on a blocking worker, one run at a time. A request that
/// arrives during a run marks the slot dirty, and the work runs again.
fn rebuild(app: &AppHandle, slot: fn(&State) -> &Slot, work: fn(&State)) {
    let state = app.state::<State>();
    // Ask first, then try to become the worker. A running worker checks
    // `dirty` after it clears `busy`, so the request is never lost.
    slot(&state).mark_dirty();
    if slot(&state).busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<State>();
        let slot = slot(&state);
        loop {
            slot.dirty.store(false, Ordering::SeqCst);
            work(&state);
            *slot.finished.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
            if slot.dirty.load(Ordering::SeqCst) {
                continue;
            }
            slot.busy.store(false, Ordering::SeqCst);
            // A request that arrived after the check above saw `busy` and only
            // marked the slot dirty; take the slot back and run it.
            if !slot.dirty.load(Ordering::SeqCst) || slot.busy.swap(true, Ordering::SeqCst) {
                break;
            }
        }
        events::results_stale(&app);
    });
}

/// Download exchange rates when they are missing or old. `force` skips the
/// retry delay, for an explicit refresh or when the user enables rates.
pub fn rates(app: &AppHandle, force: bool) {
    let state = app.state::<State>();
    if !state.settings.get().currency_rates_enabled {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    if !force
        && state
            .rates
            .get()
            .as_ref()
            .as_ref()
            .is_some_and(|r| !r.is_stale(now))
    {
        return;
    }
    {
        let mut attempt = state
            .freshness
            .rates_attempt
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !force && attempt.is_some_and(|at| at.elapsed() < RATE_RETRY) {
            return;
        }
        *attempt = Some(Instant::now());
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<State>();
        match currency::fetch(now) {
            Ok(rates) => {
                if let Err(error) = state.store.save_rates(&rates) {
                    tracing::warn!(%error, "Could not save exchange rates");
                }
                state.rates.set(Some(rates));
                events::results_stale(&app);
            }
            Err(error) => tracing::warn!(%error, "Could not refresh exchange rates"),
        }
    });
}
