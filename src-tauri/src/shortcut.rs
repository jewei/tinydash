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
    Builder, Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use crate::{
    actions,
    error::{Error, Result},
    platform,
    search::usage::Hotkeys,
    settings::Settings,
    state::State,
    window,
};

static REGISTRY: Registry = Registry::new();

/// The keys TinyDash holds now. Startup and the settings lock keep changes
/// one at a time. A key press reads it, so it is never locked while the OS
/// works.
struct Registry {
    /// The launcher shortcut, if any.
    launcher: Mutex<Option<Shortcut>>,
    /// The hotkeys, each with the result ID it runs.
    hotkeys: Mutex<Vec<(Shortcut, String)>>,
}

/// The OS side of a change, apart from the records so that tests can
/// refuse chosen keys. [`Plugin`] does it in the app.
trait Os {
    fn register(&self, shortcut: Shortcut) -> std::result::Result<(), String>;
    fn unregister(&self, shortcut: Shortcut) -> std::result::Result<(), String>;
}

struct Plugin<'a>(&'a AppHandle);

impl Os for Plugin<'_> {
    fn register(&self, shortcut: Shortcut) -> std::result::Result<(), String> {
        self.0
            .global_shortcut()
            .register(shortcut)
            .map_err(|error| error.to_string())
    }

    fn unregister(&self, shortcut: Shortcut) -> std::result::Result<(), String> {
        self.0
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if REGISTRY.is_launcher(shortcut) {
                toggle_launcher(app);
            } else if let Some(id) = REGISTRY.hotkey_target(shortcut) {
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
        .map_err(|error| Error::msg(format!("“{accelerator}” is not a valid shortcut: {error}.")))
}

/// Read the launcher shortcut. It takes the keys a hotkey can, and also
/// Shift+Space, as the recorder does (`acceleratorFromEvent`).
pub fn launcher_keys(accelerator: &str) -> Result<Shortcut> {
    let keys = parse(accelerator)?;
    if can_be_hotkey(&keys) || keys == Shortcut::new(Some(Modifiers::SHIFT), Code::Space) {
        return Ok(keys);
    }
    Err(Error::msg(format!(
        "“{accelerator}” would block typing in every app. The launcher shortcut needs a modifier such as Control or Alt, or a function key."
    )))
}

/// Replace a saved launcher shortcut that cannot be one, such as a plain
/// key in a hand-edited settings.json, with the default, and say why.
pub fn checked_at_startup(settings: &mut Settings) -> Option<String> {
    let error = launcher_keys(&settings.shortcut).err()?;
    settings.shortcut = Settings::default().shortcut;
    Some(format!(
        "{error} TinyDash uses {} until you choose other keys in Settings > General.",
        settings.shortcut
    ))
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
        " Another app may already use it, so choose other keys. On Wayland, assign a desktop shortcut that runs `tinydash` instead."
    } else {
        " Another app may already use it, so choose other keys."
    };
    Error::msg(format!("Could not register {accelerator}: {error}.{hint}"))
}

/// Replace the registered launcher shortcut. If the previous one cannot be
/// removed, it stays registered and nothing changes. If the new one cannot
/// be registered, nothing is registered, and the caller can register the
/// previous one again.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<()> {
    REGISTRY.register(&Plugin(app), accelerator)
}

/// A hotkey that could not be registered, and why.
pub struct HotkeyFailure {
    pub id: String,
    pub error: Error,
}

/// Make the registered hotkeys match `wanted`: release the ones it no
/// longer has or that changed, then register the new ones. Keys that are
/// already registered stay registered and run the result `wanted` gives
/// them, so calling it again changes nothing. Each failure is returned;
/// the others still apply.
pub fn sync(app: &AppHandle, wanted: &Hotkeys) -> Vec<HotkeyFailure> {
    REGISTRY.sync(&Plugin(app), wanted)
}

/// Pause while a recorder takes new keys, in Settings or for a hotkey, so
/// pressing a shortcut that is set is recorded instead of acting. A pause
/// that fails leaves every shortcut on, as it was.
/// Takes the settings lock; never call it on the main thread.
pub fn pause(app: &AppHandle, paused: bool) -> Result<()> {
    let state = app.state::<State>();
    let _one_change_at_a_time = state
        .settings_change
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    REGISTRY.pause(
        &Plugin(app),
        paused,
        &state.hotkeys.get(),
        &state.settings.get().shortcut,
    )
}

impl Registry {
    const fn new() -> Self {
        Self {
            launcher: Mutex::new(None),
            hotkeys: Mutex::new(Vec::new()),
        }
    }

    fn launcher(&self) -> MutexGuard<'_, Option<Shortcut>> {
        self.launcher.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn hotkeys(&self) -> MutexGuard<'_, Vec<(Shortcut, String)>> {
        self.hotkeys.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_launcher(&self, shortcut: &Shortcut) -> bool {
        *self.launcher() == Some(*shortcut)
    }

    fn hotkey_target(&self, shortcut: &Shortcut) -> Option<String> {
        self.hotkeys()
            .iter()
            .find(|(registered, _)| registered == shortcut)
            .map(|(_, id)| id.clone())
    }

    /// See [`register`].
    fn register(&self, os: &impl Os, accelerator: &str) -> Result<()> {
        let shortcut = parse(accelerator)?;
        if self.is_launcher(&shortcut) {
            return Ok(());
        }
        self.unregister(os)?;
        os.register(shortcut)
            .map_err(|error| register_error(accelerator, error))?;
        *self.launcher() = Some(shortcut);
        Ok(())
    }

    /// The record changes only when the OS released the shortcut, so it
    /// always names the shortcut that is still active.
    fn unregister(&self, os: &impl Os) -> Result<()> {
        let Some(shortcut) = *self.launcher() else {
            return Ok(());
        };
        os.unregister(shortcut).map_err(|error| {
            Error::msg(format!(
                "Could not release the current shortcut: {error}. Restart TinyDash and try again."
            ))
        })?;
        *self.launcher() = None;
        Ok(())
    }

    /// See [`sync`].
    fn sync(&self, os: &impl Os, wanted: &Hotkeys) -> Vec<HotkeyFailure> {
        let mut failures = Vec::new();
        let wanted: Vec<(&str, &str, Result<Shortcut>)> = wanted
            .iter()
            .map(|(id, accelerator)| (id, accelerator, parse(accelerator)))
            .collect();
        let stale: Vec<(Shortcut, String)> = self
            .hotkeys()
            .iter()
            .filter(|(shortcut, id)| {
                !wanted
                    .iter()
                    .any(|(want, _, parsed)| want == id && parsed.as_ref().ok() == Some(shortcut))
            })
            .cloned()
            .collect();
        for (shortcut, id) in stale {
            match os.unregister(shortcut) {
                Ok(()) => self
                    .hotkeys()
                    .retain(|(registered, _)| *registered != shortcut),
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
            // TinyDash may still hold these keys for another result, after
            // its release failed; they run this result now.
            if let Some((_, owner)) = self
                .hotkeys()
                .iter_mut()
                .find(|(registered, _)| *registered == shortcut)
            {
                *owner = id.to_owned();
                continue;
            }
            match os.register(shortcut) {
                Ok(()) => self.hotkeys().push((shortcut, id.to_owned())),
                Err(error) => failures.push(HotkeyFailure {
                    id: id.to_owned(),
                    error: register_error(accelerator, error),
                }),
            }
        }
        failures
    }

    /// See [`pause`]. `hotkeys` and `launcher` are the saved ones, which a
    /// resume turns on again.
    fn pause(&self, os: &impl Os, paused: bool, hotkeys: &Hotkeys, launcher: &str) -> Result<()> {
        if !paused {
            self.turn_on_hotkeys(os, hotkeys);
            return self.register(os, launcher);
        }
        let released = match self.sync(os, &Hotkeys::default()).into_iter().next() {
            Some(failure) => Err(failure.error),
            None => self.unregister(os),
        };
        if released.is_err() {
            // The recorder stops on this error and never resumes, so turn
            // the released hotkeys back on. The launcher shortcut is still
            // on.
            self.turn_on_hotkeys(os, hotkeys);
        }
        released
    }

    /// Register the hotkeys again after a pause. The recorder shows a pause
    /// error as the launcher shortcut's, so a hotkey that stays off, such
    /// as one that failed at startup, is only logged.
    fn turn_on_hotkeys(&self, os: &impl Os, hotkeys: &Hotkeys) {
        for failure in self.sync(os, hotkeys) {
            tracing::warn!(id = %failure.id, error = %failure.error, "A hotkey is off");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_shortcut_takes_only_keys_that_never_block_typing() {
        for keys in [
            "Control+Shift+Space",
            "Alt+Space",
            "Super+KeyK",
            "F5",
            "Shift+F12",
            "Shift+Space",
        ] {
            assert!(launcher_keys(keys).is_ok(), "{keys}");
        }
        for keys in ["KeyA", "Shift+KeyA", "Space", "Enter", "Shift+Enter"] {
            let error = launcher_keys(keys).unwrap_err().to_string();
            assert!(error.contains("would block typing"), "{keys}: {error}");
        }
        assert!(launcher_keys("Alt+Nothing").is_err());
    }

    #[test]
    fn a_saved_shortcut_that_blocks_typing_gives_way_to_the_default() {
        let mut plain = Settings {
            shortcut: "KeyA".into(),
            ..Settings::default()
        };
        let warning = checked_at_startup(&mut plain).unwrap();
        assert_eq!(plain.shortcut, Settings::default().shortcut);
        assert!(
            warning.starts_with("“KeyA” would block typing")
                && warning.contains("Control+Shift+Space"),
            "{warning}"
        );

        let mut kept = Settings {
            shortcut: "Shift+Space".into(),
            ..Settings::default()
        };
        assert_eq!(checked_at_startup(&mut kept), None);
        assert_eq!(kept.shortcut, "Shift+Space");
    }

    /// An OS that holds the keys registered with it, and refuses to
    /// register or release the keys a test chooses.
    #[derive(Default)]
    struct FakeOs {
        held: std::cell::RefCell<Vec<Shortcut>>,
        refuse_register: Vec<Shortcut>,
        refuse_release: Vec<Shortcut>,
    }

    impl Os for FakeOs {
        fn register(&self, shortcut: Shortcut) -> std::result::Result<(), String> {
            if self.refuse_register.contains(&shortcut) || self.held.borrow().contains(&shortcut) {
                return Err("another app uses it".into());
            }
            self.held.borrow_mut().push(shortcut);
            Ok(())
        }

        fn unregister(&self, shortcut: Shortcut) -> std::result::Result<(), String> {
            if self.refuse_release.contains(&shortcut) {
                return Err("the OS refused".into());
            }
            self.held.borrow_mut().retain(|held| *held != shortcut);
            Ok(())
        }
    }

    const LAUNCHER: &str = "Control+Shift+Space";

    fn keys(accelerator: &str) -> Shortcut {
        parse(accelerator).unwrap()
    }

    fn wanted(pairs: &[(&str, &str)]) -> Hotkeys {
        Hotkeys::new(
            pairs
                .iter()
                .map(|(id, accelerator)| ((*id).to_owned(), (*accelerator).to_owned())),
        )
    }

    /// Whether the OS holds exactly these keys, in any order.
    fn holds(os: &FakeOs, accelerators: &[&str]) -> bool {
        let held = os.held.borrow();
        held.len() == accelerators.len()
            && accelerators
                .iter()
                .all(|accelerator| held.contains(&keys(accelerator)))
    }

    fn target(registry: &Registry, accelerator: &str) -> Option<String> {
        registry.hotkey_target(&keys(accelerator))
    }

    /// Two hotkeys and the launcher shortcut, all on.
    fn saved(os: &FakeOs) -> (Registry, Hotkeys) {
        let registry = Registry::new();
        let hotkeys = wanted(&[("app:a", "Alt+KeyA"), ("app:b", "Alt+KeyB")]);
        registry.register(os, LAUNCHER).unwrap();
        assert!(registry.sync(os, &hotkeys).is_empty());
        (registry, hotkeys)
    }

    #[test]
    fn keys_still_held_for_a_deleted_result_run_the_result_that_takes_them() {
        let os = FakeOs {
            refuse_release: vec![keys("Alt+KeyK")],
            ..FakeOs::default()
        };
        let registry = Registry::new();
        assert!(
            registry
                .sync(&os, &wanted(&[("snippet:5", "Alt+KeyK")]))
                .is_empty()
        );
        let deleted = registry.sync(&os, &Hotkeys::default());
        assert_eq!(deleted.len(), 1);
        assert_eq!(target(&registry, "Alt+KeyK").as_deref(), Some("snippet:5"));

        let failures = registry.sync(&os, &wanted(&[("app:safari", "Alt+KeyK")]));
        assert!(failures.iter().all(|failure| failure.id == "snippet:5"));
        assert_eq!(target(&registry, "Alt+KeyK").as_deref(), Some("app:safari"));
        assert!(holds(&os, &["Alt+KeyK"]));
    }

    #[test]
    fn a_pause_whose_hotkey_release_fails_keeps_every_shortcut_on() {
        let os = FakeOs {
            refuse_release: vec![keys("Alt+KeyB")],
            ..FakeOs::default()
        };
        let (registry, hotkeys) = saved(&os);
        assert!(registry.pause(&os, true, &hotkeys, LAUNCHER).is_err());
        assert!(registry.is_launcher(&keys(LAUNCHER)));
        assert_eq!(target(&registry, "Alt+KeyA").as_deref(), Some("app:a"));
        assert_eq!(target(&registry, "Alt+KeyB").as_deref(), Some("app:b"));
        assert!(holds(&os, &["Alt+KeyA", "Alt+KeyB", LAUNCHER]));
    }

    #[test]
    fn a_pause_whose_launcher_release_fails_turns_the_hotkeys_back_on() {
        let os = FakeOs {
            refuse_release: vec![keys(LAUNCHER)],
            ..FakeOs::default()
        };
        let (registry, hotkeys) = saved(&os);
        assert!(registry.pause(&os, true, &hotkeys, LAUNCHER).is_err());
        assert!(registry.is_launcher(&keys(LAUNCHER)));
        assert_eq!(target(&registry, "Alt+KeyA").as_deref(), Some("app:a"));
        assert_eq!(target(&registry, "Alt+KeyB").as_deref(), Some("app:b"));
        assert!(holds(&os, &["Alt+KeyA", "Alt+KeyB", LAUNCHER]));
    }

    #[test]
    fn a_resume_reports_only_the_launcher_shortcut() {
        let os = FakeOs::default();
        let (registry, hotkeys) = saved(&os);
        assert!(registry.pause(&os, true, &hotkeys, LAUNCHER).is_ok());
        assert!(holds(&os, &[]));

        // Another app takes the keys of a hotkey during the pause.
        let os = FakeOs {
            refuse_register: vec![keys("Alt+KeyB")],
            ..FakeOs::default()
        };
        assert!(registry.pause(&os, false, &hotkeys, LAUNCHER).is_ok());
        assert!(registry.is_launcher(&keys(LAUNCHER)));
        assert_eq!(target(&registry, "Alt+KeyA").as_deref(), Some("app:a"));
        assert_eq!(target(&registry, "Alt+KeyB"), None);
        assert!(holds(&os, &["Alt+KeyA", LAUNCHER]));

        let os = FakeOs {
            refuse_register: vec![keys("Alt+Space")],
            ..FakeOs::default()
        };
        let registry = Registry::new();
        let error = registry
            .pause(&os, false, &hotkeys, "Alt+Space")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Could not register Alt+Space"),
            "{error}"
        );
    }
}
