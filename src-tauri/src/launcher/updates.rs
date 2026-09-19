use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

const UPDATE_ENDPOINT_ENV: &str = "TINYDASH_UPDATE_ENDPOINT";
const UPDATE_PUBLIC_KEY_ENV: &str = "TAURI_UPDATER_PUBLIC_KEY";
const UPDATE_TIMEOUT: Duration = Duration::from_secs(15);
const UPDATE_CACHE_MAX_AGE: Duration = Duration::from_secs(15 * 60);

struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub message: String,
}

struct CachedUpdate {
    update: Update,
    checked_at: Instant,
}

pub struct UpdateState {
    cached: Mutex<Option<CachedUpdate>>,
    busy: AtomicBool,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            cached: Mutex::new(None),
            busy: AtomicBool::new(false),
        }
    }
}

fn status(
    available: bool,
    version: Option<String>,
    notes: Option<String>,
    message: String,
) -> UpdateStatus {
    UpdateStatus {
        available,
        version,
        notes,
        message,
    }
}

fn config_from_values(
    endpoint: Option<&str>,
    public_key: Option<&str>,
) -> Result<(Url, String), String> {
    let endpoint = endpoint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{UPDATE_ENDPOINT_ENV} is not configured."))?;
    let endpoint = Url::parse(endpoint)
        .map_err(|_| format!("{UPDATE_ENDPOINT_ENV} must be a valid HTTPS URL."))?;
    if endpoint.scheme() != "https" {
        return Err(format!("{UPDATE_ENDPOINT_ENV} must use HTTPS."));
    }
    let public_key = public_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{UPDATE_PUBLIC_KEY_ENV} is not configured."))?
        .to_owned();
    Ok((endpoint, public_key))
}

fn compile_time_config() -> Result<(Url, String), String> {
    config_from_values(
        option_env!("TINYDASH_UPDATE_ENDPOINT"),
        option_env!("TAURI_UPDATER_PUBLIC_KEY"),
    )
}

fn linux_status() -> UpdateStatus {
    let message = "Linux updates use the manual .deb package. Download the latest package and install it after closing TinyDash.".to_owned();
    status(false, None, None, message)
}

fn update_builder<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri_plugin_updater::Updater, String> {
    let (endpoint, public_key) = compile_time_config()?;
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|error| format!("Could not configure the updater: {error}"))?
        .pubkey(public_key)
        .timeout(UPDATE_TIMEOUT)
        .build()
        .map_err(|error| format!("Could not start the updater: {error}"))
}

fn take_cached(state: &UpdateState) -> Result<Option<CachedUpdate>, String> {
    state
        .cached
        .lock()
        .map_err(|_| "Updater state is unavailable.".to_owned())
        .map(|mut cached| cached.take())
}

fn put_cached(state: &UpdateState, cached: CachedUpdate) -> Result<(), String> {
    state
        .cached
        .lock()
        .map_err(|_| "Updater state is unavailable.".to_owned())
        .map(|mut current| current.replace(cached))
        .map(|_| ())
}

fn clear_cached(state: &UpdateState) -> Result<(), String> {
    state
        .cached
        .lock()
        .map_err(|_| "Updater state is unavailable.".to_owned())
        .map(|mut cached| cached.take())
        .map(|_| ())
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> UpdateStatus {
    if cfg!(target_os = "linux") {
        return linux_status();
    }

    let state = app.state::<UpdateState>();
    if state.busy.swap(true, Ordering::AcqRel) {
        return status(
            false,
            None,
            None,
            "An update operation is in progress. Try again after it finishes.".to_owned(),
        );
    }
    let _busy = BusyGuard(&state.busy);

    let updater = match update_builder(&app) {
        Ok(updater) => updater,
        Err(error) => {
            let _ = clear_cached(&state);
            tracing::debug!(%error, "Update configuration is unavailable");
            return status(false, None, None, "Updates are not configured in this build. Use a new package from the TinyDash release page.".into());
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            let result = status(
                true,
                Some(update.version.clone()),
                update.body.clone(),
                format!("TinyDash {} is ready to install.", update.version),
            );
            let cached = CachedUpdate {
                update,
                checked_at: Instant::now(),
            };
            if let Err(error) = put_cached(&state, cached) {
                return status(false, None, None, error);
            }
            result
        }
        Ok(None) => {
            let _ = clear_cached(&state);
            status(false, None, None, "TinyDash is up to date.".to_owned())
        }
        Err(error) => {
            let _ = clear_cached(&state);
            status(false, None, None, format!("Update check failed: {error}"))
        }
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    if cfg!(target_os = "linux") {
        return Err(
            "Linux updates are manual. Close TinyDash, install the latest .deb package, and reopen it."
                .to_owned(),
        );
    }

    let state = app.state::<UpdateState>();
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("An update operation is already in progress.".to_owned());
    }
    let _busy = BusyGuard(&state.busy);
    install_cached(&state).await
}

async fn install_cached(state: &UpdateState) -> Result<(), String> {
    let cached = take_cached(state)?.ok_or_else(|| {
        "No update is ready. Run Check for updates first, then install the reported version."
            .to_owned()
    })?;
    if cached.checked_at.elapsed() > UPDATE_CACHE_MAX_AGE {
        return Err("The update check is stale. Run Check for updates again.".to_owned());
    }

    let update = cached.update.restart_after_install(false);
    let bytes = match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let message = format!("Update download failed: {error}");
            let _ = put_cached(
                state,
                CachedUpdate {
                    update,
                    checked_at: cached.checked_at,
                },
            );
            return Err(message);
        }
    };
    if let Err(error) = update.install(bytes) {
        let message = format!("Update installation failed: {error}");
        let _ = put_cached(
            state,
            CachedUpdate {
                update,
                checked_at: cached.checked_at,
            },
        );
        return Err(message);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_https_endpoint_and_public_key() {
        assert!(config_from_values(None, Some("key")).is_err());
        assert!(
            config_from_values(Some("http://updates.example/latest.json"), Some("key")).is_err()
        );
        assert!(config_from_values(Some("https://updates.example/latest.json"), None).is_err());
        assert!(
            config_from_values(Some("https://updates.example/latest.json"), Some("key")).is_ok()
        );
    }

    #[test]
    fn status_fields_are_explicit() {
        let status = status(
            true,
            Some("1.2.3".to_owned()),
            Some("Fixes startup.".to_owned()),
            "Ready.".to_owned(),
        );
        assert!(status.available);
        assert_eq!(status.version.as_deref(), Some("1.2.3"));
        assert_eq!(status.notes.as_deref(), Some("Fixes startup."));
    }

    #[test]
    fn linux_message_is_manual_package_update() {
        let message = linux_status().message;
        assert!(message.contains("manual .deb"));
        assert!(!message.contains("ready to install"));
    }
}
