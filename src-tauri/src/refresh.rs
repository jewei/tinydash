//! Keeps indexes and exchange rates fresh without background polling.
//!
//! The file watcher only marks an index dirty. Work happens at startup,
//! when the launcher opens, when settings change, and on Refresh, on a
//! blocking worker, and the new index replaces the old one in one swap.

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
    /// Something changed since the last run began; the next launcher open
    /// rebuilds.
    dirty: AtomicBool,
    /// A rebuild was asked for while one ran; the worker runs once more.
    requested: AtomicBool,
    busy: AtomicBool,
    finished: Mutex<Option<Instant>>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            dirty: AtomicBool::new(true),
            requested: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            finished: Mutex::new(None),
        }
    }
}

impl Slot {
    /// The watcher saw a change. It only waits for the next launcher open,
    /// so a folder that changes all the time cannot keep a worker scanning.
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Ask for a rebuild: true when the caller must start the worker. A
    /// worker that is running already runs once more.
    fn request(&self) -> bool {
        self.requested.store(true, Ordering::SeqCst);
        !self.busy.swap(true, Ordering::SeqCst)
    }

    /// A run starts and covers every change and request made so far.
    fn begin(&self) {
        self.requested.store(false, Ordering::SeqCst);
        self.dirty.store(false, Ordering::SeqCst);
    }

    /// A run ended: true when a request arrived meanwhile and the worker
    /// must run again.
    fn end(&self) -> bool {
        *self.finished.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        if self.requested.load(Ordering::SeqCst) {
            return true;
        }
        self.busy.store(false, Ordering::SeqCst);
        // A request that came after the check above saw `busy` and did not
        // start a worker; take the slot back and run it.
        self.requested.load(Ordering::SeqCst) && !self.busy.swap(true, Ordering::SeqCst)
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
                platform::is_hidden,
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
/// arrives during a run makes the work run again; a watcher change
/// (`mark_dirty`) waits for the next launcher open.
fn rebuild(app: &AppHandle, slot: fn(&State) -> &Slot, work: fn(&State)) {
    if !slot(&app.state::<State>()).request() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<State>();
        let slot = slot(&state);
        loop {
            slot.begin();
            work(&state);
            if !slot.end() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_during_a_run_wait_for_the_next_open_but_requests_run_again() {
        let slot = Slot::default();
        assert!(slot.request());
        slot.begin();
        slot.mark_dirty();
        assert!(!slot.end());
        assert!(slot.needs_work());

        assert!(slot.request());
        slot.begin();
        assert!(!slot.request());
        assert!(slot.end());
        slot.begin();
        assert!(!slot.end());
        assert!(!slot.needs_work());
    }

    #[test]
    fn scanned_sources_are_ready_after_their_first_scan() {
        let freshness = Freshness::default();
        let ready = freshness.ready();
        assert!(!ready(Source::App) && !ready(Source::File) && ready(Source::Emoji));

        *freshness.apps.finished.lock().unwrap() = Some(Instant::now());
        let ready = freshness.ready();
        assert!(ready(Source::App) && !ready(Source::File));
    }
}
