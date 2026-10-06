//! The global shortcut that toggles the launcher.
//!
//! The plugin calls the OS on the main thread and waits for it. `register`
//! and `unregister` take the plugin's lock only after that wait, so they are
//! safe from any thread. `unregister_all` holds the lock while it waits, and
//! a key press needs the lock, so it is never used.

use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Manager, Wry, plugin::TauriPlugin};
use tauri_plugin_global_shortcut::{Builder, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    error::{Error, Result},
    state::State,
    window,
};

/// The shortcut TinyDash registered, if any. Startup and the settings lock
/// keep changes to it one at a time; it is never locked during a wait.
static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .with_handler(|app, _, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            // On X11 this runs on the hotkey thread while the plugin holds its
            // lock, and a shortcut change on the main thread waits for that
            // thread. Waiting here for the main thread would freeze both.
            let handle = app.clone();
            let posted = app.run_on_main_thread(move || {
                if let Err(error) = window::toggle(&handle) {
                    tracing::warn!(%error, "Could not toggle the launcher");
                }
            });
            if let Err(error) = posted {
                tracing::warn!(%error, "Could not toggle the launcher");
            }
        })
        .build()
}

/// Replace the registered shortcut. If the previous one cannot be removed,
/// it stays registered and nothing changes. If the new one cannot be
/// registered, nothing is registered, and the caller can register the
/// previous one again.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<()> {
    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|error| Error::msg(format!("“{accelerator}” is not a valid shortcut: {error}")))?;
    if *registered() == Some(shortcut) {
        return Ok(());
    }
    unregister(app)?;
    app.global_shortcut().register(shortcut).map_err(|error| {
        let hint = if cfg!(target_os = "linux") {
            " Another app may already use it. On Wayland, assign a desktop shortcut that runs `tinydash` instead."
        } else {
            " Another app may already use it."
        };
        Error::msg(format!("Could not register {accelerator}: {error}.{hint}"))
    })?;
    *registered() = Some(shortcut);
    Ok(())
}

/// The record changes only when the OS released the shortcut, so it always
/// names the shortcut that is still active.
fn unregister(app: &AppHandle) -> Result<()> {
    let Some(shortcut) = *registered() else {
        return Ok(());
    };
    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|error| {
            Error::msg(format!(
                "Could not release the current shortcut: {error}. Restart TinyDash and try again."
            ))
        })?;
    *registered() = None;
    Ok(())
}

fn registered() -> MutexGuard<'static, Option<Shortcut>> {
    REGISTERED.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pause while Settings records a new shortcut, so pressing the current
/// one is recorded instead of toggling the launcher.
/// Takes the settings lock; never call it on the main thread.
pub fn pause(app: &AppHandle, paused: bool) -> Result<()> {
    let state = app.state::<State>();
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if paused {
        unregister(app)
    } else {
        register(app, &state.settings.get().shortcut)
    }
}
