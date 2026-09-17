use std::{
    sync::{Mutex, atomic::Ordering},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::LauncherState;
use crate::{currency::Rates, ranking};

#[derive(Default)]
struct Refresh {
    running: bool,
    last_attempt: Option<Instant>,
    warning: Option<String>,
    cached_at: Option<i64>,
}

impl Refresh {
    fn begin(&mut self, cached: Option<i64>, now: i64, instant: Instant, force: bool) -> bool {
        let fresh = cached
            .is_some_and(|cached| now >= cached && now - cached < crate::currency::FRESH_SECONDS);
        let cooldown = if force {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(60 * 60)
        };
        if self.running
            || (!force && fresh)
            || self
                .last_attempt
                .is_some_and(|last| instant.saturating_duration_since(last) < cooldown)
        {
            return false;
        }
        self.running = true;
        self.last_attempt = Some(instant);
        true
    }
}

#[derive(Default)]
pub struct Currency(Mutex<Refresh>);

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyStatus {
    pub as_of: Option<String>,
    pub refreshing: bool,
    pub warning: Option<String>,
}

impl Currency {
    pub fn loaded(&self, rates: &Rates) {
        if let Ok(mut state) = self.0.lock() {
            state.cached_at = Some(rates.fetched_at);
        }
    }
    pub fn status(&self, rates: Option<&Rates>) -> CurrencyStatus {
        let state = self.0.lock().ok();
        CurrencyStatus {
            as_of: rates.map(|rates| rates.date.clone()),
            refreshing: state.as_ref().is_some_and(|state| state.running),
            warning: state.as_ref().and_then(|state| state.warning.clone()),
        }
    }

    pub fn warning(&self, warning: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.warning = warning;
        }
    }
}

#[tauri::command]
pub fn refresh_currency(app: AppHandle) {
    refresh(&app, true);
}

pub fn refresh(app: &AppHandle, force: bool) {
    let state = app.state::<LauncherState>();
    if !state.ready.load(Ordering::Acquire) {
        return;
    }
    if !state.settings.currency_rates_enabled {
        if force {
            state.currency.warning(Some(
                "Currency refresh is disabled in settings.json.".into(),
            ));
            let _ = app.emit("currency-changed", ());
        }
        return;
    }
    let cached = {
        let Ok(mut gate) = state.currency.0.lock() else {
            return;
        };
        let cached = gate.cached_at;
        if !gate.begin(cached, ranking::now(), Instant::now(), force) {
            return;
        }
        cached
    };
    let _ = app.emit("currency-changed", ());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = crate::currency::fetch().await;
        let worker = app.clone();
        let warning = match result {
            Ok(rates) => {
                let saved = tauri::async_runtime::spawn_blocking(move || {
                    let state = worker.state::<LauncherState>();
                    let accepted = state.search.lock().map_err(|error| error.to_string())?.set_rates(rates.clone());
                    if !accepted { return Err("The service returned older currency rates. The saved rates remain in use.".into()); }
                    state.currency.loaded(&rates);
                    state.storage.save_rates(&worker, &rates)
                }).await;
                match saved {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "Could not cache currency rates");
                        Some(error)
                    }
                    Err(error) => Some(format!("Could not cache currency rates: {error}")),
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Could not refresh currency rates");
                Some(if cached.is_some() {
                    "Could not refresh currency rates. Saved rates remain available offline.".into()
                } else {
                    crate::currency::Error::Unavailable.to_string()
                })
            }
        };
        if let Ok(mut state) = app.state::<LauncherState>().currency.0.lock() {
            state.running = false;
            state.warning = warning;
        }
        let _ = app.emit("currency-changed", ());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refreshes_stale_rates_once_and_throttles_failed_attempts_without_timers() {
        let mut refresh = Refresh::default();
        let start = Instant::now();
        assert!(!refresh.begin(Some(100), 101, start, false));
        assert!(refresh.begin(None, 101, start, false));
        assert!(!refresh.begin(None, 101, start + Duration::from_secs(60), true));
        refresh.running = false;
        assert!(!refresh.begin(None, 102, start + Duration::from_secs(4), true));
        assert!(!refresh.begin(None, 102, start + Duration::from_secs(60), false));
        assert!(refresh.begin(None, 102, start + Duration::from_secs(60), true));
        refresh.running = false;
        assert!(refresh.begin(
            Some(100),
            100 + crate::currency::FRESH_SECONDS,
            start + Duration::from_secs(4000),
            false
        ));
    }
}
