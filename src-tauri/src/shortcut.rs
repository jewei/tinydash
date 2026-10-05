//! The global shortcut that toggles the launcher.

use tauri::{AppHandle, Manager, Wry, plugin::TauriPlugin};
use tauri_plugin_global_shortcut::{Builder, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    error::{Error, Result},
    state::State,
    window,
};

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .with_handler(|app, _, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if let Err(error) = window::toggle(app) {
                tracing::warn!(%error, "Could not toggle the launcher");
            }
        })
        .build()
}

/// Replace the registered shortcut. On failure nothing is registered, and
/// the caller can register the previous one again.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<()> {
    let shortcut: Shortcut = accelerator
        .parse()
        .map_err(|error| Error::msg(format!("“{accelerator}” is not a valid shortcut: {error}")))?;
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().ok();
    shortcuts.register(shortcut).map_err(|error| {
        let hint = if cfg!(target_os = "linux") {
            " On Wayland, assign a desktop shortcut that runs `tinydash` instead."
        } else {
            " Another app may already use it."
        };
        Error::msg(format!("Could not register {accelerator}: {error}.{hint}"))
    })
}

/// Pause while Settings records a new shortcut, so pressing the current
/// one is recorded instead of toggling the launcher.
pub fn pause(app: &AppHandle, paused: bool) -> Result<()> {
    let state = app.state::<State>();
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if paused {
        app.global_shortcut()
            .unregister_all()
            .map_err(|error| Error::msg(error.to_string()))
    } else {
        register(app, &state.settings.get().shortcut)
    }
}
