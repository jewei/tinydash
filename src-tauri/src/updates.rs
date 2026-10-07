//! Updates on macOS and Windows. When the launcher opens and the last check
//! is old, TinyDash reads a signed feed in the background; a newer version
//! is announced to the windows and installed only when the user asks.
//!
//! The feed address and the public key come from the release build's
//! environment (`TINYDASH_UPDATE_ENDPOINT`, `TAURI_UPDATER_PUBLIC_KEY`), so a
//! local build never checks. Linux installs from a .deb, which the updater
//! cannot replace, so it never checks either.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    error::{Error, Result},
    events, platform,
    state::State,
};

const FEED: Option<&str> = option_env!("TINYDASH_UPDATE_ENDPOINT");
const PUBLIC_KEY: Option<&str> = option_env!("TAURI_UPDATER_PUBLIC_KEY");
/// Check again, when the launcher opens, after this long.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Updates {
    /// The newer version found by the latest check, ready to install.
    pending: Mutex<Option<Update>>,
    /// When a background check last started; a failed one waits too.
    last_check: Mutex<Option<Instant>>,
    /// A check or install is running, so another one waits its turn.
    busy: AtomicBool,
}

/// Whether this build can update itself.
pub fn supported() -> bool {
    let set = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());
    platform::SELF_UPDATE && set(FEED) && set(PUBLIC_KEY)
}

/// The newer version that the latest check found, if any.
pub fn pending_version(app: &AppHandle) -> Option<String> {
    let updates = app.state::<Updates>();
    let pending = updates.pending.lock().unwrap_or_else(|e| e.into_inner());
    pending.as_ref().map(|update| update.version.clone())
}

/// When the launcher opens: check in the background if the setting is on
/// and the last check is old. Failures are only logged.
pub fn on_launcher_shown(app: &AppHandle) {
    if !supported() || !app.state::<State>().settings.get().check_for_updates {
        return;
    }
    {
        let updates = app.state::<Updates>();
        let mut last = updates.last_check.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|at| at.elapsed() < CHECK_EVERY) {
            return;
        }
        *last = Some(Instant::now());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = check(&app).await {
            tracing::warn!(%error, "Could not check for updates");
        }
    });
}

/// Check the feed now. Gives the newer version, or `None` when this one is
/// the latest, and tells the windows about a newer one.
pub async fn check(app: &AppHandle) -> Result<Option<String>> {
    let _busy = Busy::start(app)?;
    let found = updater(app)?
        .check()
        .await
        .map_err(|error| Error::msg(format!("Could not check for updates: {error}")))?;
    let version = found.as_ref().map(|update| update.version.clone());
    *app.state::<Updates>()
        .pending
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = found;
    if let Some(version) = &version {
        events::update_available(app, version);
    }
    Ok(version)
}

/// Download and install the version that the latest check found, then
/// restart into it. On failure the update stays ready for another try.
pub async fn install(app: &AppHandle) -> Result<()> {
    let _busy = Busy::start(app)?;
    let update = app
        .state::<Updates>()
        .pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| Error::msg("No update is ready. Check for updates first."))?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| {
            Error::msg(format!(
                "Could not install TinyDash {}: {error}. Try again, or download it from the Releases page.",
                update.version
            ))
        })?;
    app.restart()
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater> {
    let (Some(feed), Some(key), true) = (FEED, PUBLIC_KEY, supported()) else {
        return Err(Error::msg(
            "This copy of TinyDash cannot update itself. Download new versions from the Releases page.",
        ));
    };
    let feed = feed
        .parse()
        .map_err(|error| Error::msg(format!("The update address is not valid: {error}")))?;
    app.updater_builder()
        .endpoints(vec![feed])
        .and_then(|builder| builder.pubkey(key).timeout(TIMEOUT).build())
        .map_err(|error| Error::msg(format!("Could not start the updater: {error}")))
}

/// Marks a check or install as running until dropped.
struct Busy<'a>(&'a AtomicBool);

impl<'a> Busy<'a> {
    fn start(app: &'a AppHandle) -> Result<Self> {
        let busy = &app.state::<Updates>().inner().busy;
        if busy.swap(true, Ordering::AcqRel) {
            return Err(Error::msg(
                "TinyDash is already checking for or installing an update. Try again in a moment.",
            ));
        }
        Ok(Self(busy))
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_without_a_feed_and_key_never_updates() {
        // Tests, like local builds, are built without the release's values.
        if FEED.is_none() || PUBLIC_KEY.is_none() {
            assert!(!supported());
        }
    }
}
