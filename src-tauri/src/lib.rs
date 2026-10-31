//! TinyDash: a small, keyboard-first desktop launcher.
//!
//! Start at [`run`]. `AGENTS.md` maps the modules, and `docs/architecture.md`
//! explains how they work together.

mod actions;
mod cli;
mod commands;
mod error;
mod events;
mod expander;
mod features;
mod hud;
mod images;
mod monitor;
mod platform;
mod preview;
mod refresh;
mod search;
mod settings;
mod shared;
mod shortcut;
mod spotlight;
mod state;
mod store;
mod system_clipboard;
mod timer;
mod tray;
mod updates;
mod watcher;
mod widgets;
mod window;

use tauri::{App, AppHandle, Manager};

use cli::Launch;
use state::{Dirs, State};

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", cli::USAGE);
        return;
    }
    let launch = match cli::parse(&args) {
        Ok(launch) => launch,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tinydash_lib=info".into()),
        )
        .init();

    // Before Tauri starts GTK, which opens the X display on Linux.
    platform::prepare_process();
    tauri::Builder::default()
        // Must be first: a second process hands over its arguments and exits.
        .plugin(tauri_plugin_single_instance::init(
            |app, args, _| match cli::parse(args.get(1..).unwrap_or_default()) {
                Ok(launch) => open(app, launch, true),
                Err(message) => tracing::warn!("{message}"),
            },
        ))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(shortcut::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_asynchronous_uri_scheme_protocol("icon", images::serve_icon)
        .register_asynchronous_uri_scheme_protocol("clip", images::serve_clipboard_image)
        .manage(watcher::Watcher::default())
        .manage(updates::Updates::default())
        .setup(move |app| {
            setup(app);
            open(app.handle(), launch, false);
            Ok(())
        })
        .on_window_event(window::on_event)
        .invoke_handler(tauri::generate_handler![
            commands::launcher_init,
            commands::search,
            commands::spotlight_files,
            commands::run_action,
            commands::preview,
            commands::hide_launcher,
            commands::drag_launcher,
            commands::get_settings,
            commands::update_settings,
            commands::pause_shortcut,
            commands::library_items,
            commands::save_library_item,
            commands::delete_library_item,
            commands::about,
            commands::check_for_update,
            commands::install_update,
            commands::widgets,
            commands::save_note,
            commands::hidden_results,
            commands::aliases,
            commands::unhide_result,
            commands::remove_alias,
            commands::hotkeys,
            commands::remove_hotkey,
        ])
        .build(tauri::generate_context!())
        .expect("TinyDash failed to start")
        .run(|app, event| {
            if platform::is_reopen(&event) {
                open(app, Launch::Launcher, true);
            }
            // Stop reading keys before the process ends; this forgets what
            // was typed.
            if matches!(event, tauri::RunEvent::Exit) {
                expander::stop();
            }
        });
}

/// The database of TinyDash 0.1, which this version does not read.
const LEGACY_DATABASE: &str = "tinydash.sqlite3";

/// Load state and start background work. Problems become launcher warnings;
/// only a missing app folder location stops startup.
fn setup(app: &mut App) {
    platform::prepare_app(app);

    let paths = app.path();
    let dir = |dir: tauri::Result<std::path::PathBuf>| dir.expect("the OS reports app folders");
    let config_dir = dir(paths.app_config_dir());
    // Local, not roaming: on Windows, roaming profiles would copy the
    // clipboard history to a server. Elsewhere the two are the same folder.
    let data_dir = dir(paths.app_local_data_dir());
    let legacy_dir = dir(paths.app_data_dir());
    // The same source as every other home lookup (`std::env::home_dir`).
    let home_dir = std::env::home_dir().unwrap_or_default();

    let mut warnings = Vec::new();
    let settings::Loaded {
        mut settings,
        warning,
        file: settings_file,
    } = settings::load(&config_dir);
    warnings.extend(warning);
    warnings.extend(shortcut::checked_at_startup(&mut settings));
    if legacy_dir.join(LEGACY_DATABASE).exists() {
        warnings.push(format!(
            "This version starts with new history and pins. Data from TinyDash 0.1 is still in {}: delete {LEGACY_DATABASE} and recovery.tar there if you no longer need it.",
            legacy_dir.display()
        ));
    }
    let store = match store::Store::open(&data_dir) {
        Ok((store, warning)) => {
            warnings.extend(warning);
            store
        }
        Err(error) => {
            warnings.push(data_not_opened(&error));
            store::Store::in_memory()
        }
    };
    let shortcut = settings.shortcut.clone();
    let tray_visible = settings.show_tray_icon;
    let expand_snippets = settings.expand_snippets;
    app.manage(State::new(
        settings,
        settings_file,
        store,
        Dirs {
            config: config_dir,
            data: data_dir,
            home: home_dir,
        },
        warnings,
    ));

    let handle = app.handle();
    let state = handle.state::<State>();
    if let Err(error) = shortcut::register(handle, &shortcut) {
        state.warn(error.to_string());
    }
    for failure in shortcut::sync(handle, &state.hotkeys.get()) {
        state.warn(actions::hotkey_off(&state.snapshot(), &failure));
    }
    if let Err(error) = tray::set_visible(handle, tray_visible) {
        state.warn(format!(
            "The tray icon is unavailable: {error}. Open TinyDash with its shortcut instead."
        ));
    }
    if let Some(launcher) = handle.get_webview_window(window::LAUNCHER) {
        platform::prepare_launcher(&launcher);
    }
    if let Err(error) = hud::create(handle) {
        tracing::warn!(%error, "Copy messages are unavailable");
    }
    monitor::start(handle);
    if expand_snippets && let Err(error) = expander::start(handle, false) {
        state.warn(format!("Snippets do not expand as you type. {error}"));
    }
    timer::start(handle);
    watcher::watch(handle);
    refresh::apps(handle);
    refresh::files(handle);
    refresh::rates(handle, false);
    refresh::weather(handle, false);
}

/// The warning when saved data cannot open. Only a file error has no step of
/// its own, so only it gets one here; a second step would repeat the first.
fn data_not_opened(error: &error::Error) -> String {
    let step = match error {
        error::Error::Io(_) => " To try again, quit and reopen TinyDash.",
        _ => "",
    };
    format!("Could not open saved data, so changes this session will not be saved. {error}{step}")
}

/// Act on launch arguments. With no arguments, a second launch toggles the
/// launcher, so a desktop shortcut running `tinydash` works like the hotkey.
fn open(app: &AppHandle, launch: Launch, already_running: bool) {
    let result = match launch {
        Launch::Launcher if already_running => window::toggle(app),
        Launch::Launcher => window::show(app, None),
        Launch::Settings => window::open_settings(app),
        Launch::Category(category) => window::show(app, Some(category)),
        Launch::Background => Ok(()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "Could not open a window");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_webview_may_only_listen_to_events() {
        // Only Rust emits events; a webview that could emit them could fake
        // `settings:changed` or `launcher:shown` for the other windows.
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["core:event:allow-listen", "core:event:allow-unlisten"])
        );
    }

    #[test]
    fn the_data_warning_gives_one_next_step() {
        use crate::error::Error;
        use rusqlite::ffi;
        let file =
            super::data_not_opened(&std::io::Error::from(std::io::ErrorKind::NotFound).into());
        assert!(
            file.ends_with(". To try again, quit and reopen TinyDash."),
            "{file}"
        );
        let data = super::data_not_opened(&Error::from(rusqlite::Error::SqliteFailure(
            ffi::Error::new(ffi::SQLITE_IOERR),
            None,
        )));
        assert!(
            data.ends_with(". Try again, or restart TinyDash."),
            "{data}"
        );
        assert!(!data.contains("quit and reopen"), "{data}");
    }
}
