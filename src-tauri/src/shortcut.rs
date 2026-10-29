//! Global shortcuts: the one that toggles the launcher, and the hotkeys
//! that run a result from any app.
//!
//! The plugin calls the OS on the main thread and waits for it. `register`
//! and `unregister` take the plugin's lock only after that wait, so they are
//! safe from any thread. `unregister_all` holds the lock while it waits, and
//! a key press needs the lock, so it is never used.

use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Manager, Wry, plugin::TauriPlugin};
use tauri_plugin_global_shortcut::{
    Builder, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use crate::{
    actions,
    error::{Error, Result},
    platform,
    search::usage::Hotkeys,
    state::State,
    window,
};

/// The launcher shortcut TinyDash registered, if any. Startup and the
/// settings lock keep changes to it one at a time; it is never locked
/// during a wait.
static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

/// The hotkeys registered now, each with the result ID it runs. Changed
/// only by [`sync`], under the settings lock, like the launcher shortcut.
static HOTKEYS: Mutex<Vec<(Shortcut, String)>> = Mutex::new(Vec::new());

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if *registered() == Some(*shortcut) {
                toggle_launcher(app);
            } else if let Some(id) = hotkey_target(shortcut) {
                // A result's action may wait for the OS, such as launching
                // an app; the hotkey thread must stay free.
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(error) = actions::run_hotkey(&handle, &id) {
                        report_hotkey_error(&handle, &error);
                    }
                });
            }
        })
        .build()
}

/// A hotkey runs while the user is in another app, so its error goes to a
/// system notification. The "Copied" message is too small for an error and
/// shows a check mark.
fn report_hotkey_error(app: &AppHandle, error: &Error) {
    tracing::warn!(%error, "A hotkey did not run");
    if let Err(error) = platform::notify(app, "The hotkey did not run", &error.to_string()) {
        tracing::warn!(%error, "Could not show why the hotkey did not run");
    }
}

fn toggle_launcher(app: &AppHandle) {
    // On X11 this runs on the hotkey thread while the plugin holds its lock,
    // and a shortcut change on the main thread waits for that thread.
    // Waiting here for the main thread would freeze both.
    let handle = app.clone();
    let posted = app.run_on_main_thread(move || {
        if let Err(error) = window::toggle(&handle) {
            tracing::warn!(%error, "Could not toggle the launcher");
        }
    });
    if let Err(error) = posted {
        tracing::warn!(%error, "Could not toggle the launcher");
    }
}

/// Read accelerator text, such as `Control+Shift+Space`.
pub fn parse(accelerator: &str) -> Result<Shortcut> {
    accelerator
        .parse()
        .map_err(|error| Error::msg(format!("“{accelerator}” is not a valid shortcut: {error}")))
}

/// Whether keys can be a hotkey without blocking typing everywhere: they
/// hold Control, Alt, or Super (Command), or are a function key.
pub fn can_be_hotkey(keys: &Shortcut) -> bool {
    let strong = Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER;
    keys.mods.intersects(strong)
        || keys
            .key
            .to_string()
            .strip_prefix('F')
            .is_some_and(|n| n.parse::<u8>().is_ok())
}

fn register_error(accelerator: &str, error: impl std::fmt::Display) -> Error {
    let hint = if cfg!(target_os = "linux") {
        " Another app may already use it. On Wayland, assign a desktop shortcut that runs `tinydash` instead."
    } else {
        " Another app may already use it."
    };
    Error::msg(format!("Could not register {accelerator}: {error}.{hint}"))
}

/// Replace the registered shortcut. If the previous one cannot be removed,
/// it stays registered and nothing changes. If the new one cannot be
/// registered, nothing is registered, and the caller can register the
/// previous one again.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<()> {
    let shortcut = parse(accelerator)?;
    if *registered() == Some(shortcut) {
        return Ok(());
    }
    unregister(app)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|error| register_error(accelerator, error))?;
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

fn hotkeys() -> MutexGuard<'static, Vec<(Shortcut, String)>> {
    HOTKEYS.lock().unwrap_or_else(|e| e.into_inner())
}

fn hotkey_target(shortcut: &Shortcut) -> Option<String> {
    hotkeys()
        .iter()
        .find(|(registered, _)| registered == shortcut)
        .map(|(_, id)| id.clone())
}

/// A hotkey that could not be registered, and why.
pub struct HotkeyFailure {
    pub id: String,
    pub error: Error,
}

/// Make the registered hotkeys match `wanted`: release the ones it no
/// longer has or that changed, then register the new ones. Keys that are
/// already registered are left alone, so calling it again changes nothing.
/// Each failure is returned; the others still apply.
pub fn sync(app: &AppHandle, wanted: &Hotkeys) -> Vec<HotkeyFailure> {
    let mut failures = Vec::new();
    let wanted: Vec<(&str, &str, Result<Shortcut>)> = wanted
        .iter()
        .map(|(id, accelerator)| (id, accelerator, parse(accelerator)))
        .collect();
    let stale: Vec<(Shortcut, String)> = hotkeys()
        .iter()
        .filter(|(shortcut, id)| {
            !wanted
                .iter()
                .any(|(want, _, parsed)| want == id && parsed.as_ref().ok() == Some(shortcut))
        })
        .cloned()
        .collect();
    for (shortcut, id) in stale {
        match app.global_shortcut().unregister(shortcut) {
            Ok(()) => hotkeys().retain(|(registered, _)| *registered != shortcut),
            Err(error) => failures.push(HotkeyFailure {
                id,
                error: Error::msg(format!(
                    "Could not release a hotkey: {error}. Restart TinyDash and try again."
                )),
            }),
        }
    }
    for (id, accelerator, parsed) in wanted {
        let shortcut = match parsed {
            Ok(shortcut) => shortcut,
            Err(error) => {
                failures.push(HotkeyFailure {
                    id: id.to_owned(),
                    error,
                });
                continue;
            }
        };
        if hotkeys()
            .iter()
            .any(|(registered, _)| *registered == shortcut)
        {
            continue;
        }
        match app.global_shortcut().register(shortcut) {
            Ok(()) => hotkeys().push((shortcut, id.to_owned())),
            Err(error) => failures.push(HotkeyFailure {
                id: id.to_owned(),
                error: register_error(accelerator, error),
            }),
        }
    }
    failures
}

/// Pause while a recorder takes new keys, in Settings or for a hotkey, so
/// pressing a shortcut that is set is recorded instead of acting.
/// Takes the settings lock; never call it on the main thread.
pub fn pause(app: &AppHandle, paused: bool) -> Result<()> {
    let state = app.state::<State>();
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let wanted = if paused {
        Hotkeys::default()
    } else {
        Hotkeys::clone(&state.hotkeys.get())
    };
    let failure = sync(app, &wanted).into_iter().next();
    if paused {
        unregister(app)?;
    } else {
        register(app, &state.settings.get().shortcut)?;
    }
    failure.map_or(Ok(()), |failure| Err(failure.error))
}
