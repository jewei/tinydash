//! TinyDash: a small, keyboard-first desktop launcher.
//!
//! Start at [`run`]. `docs/architecture.md` maps the modules.

mod actions;
mod cli;
mod commands;
mod error;
mod events;
mod features;
mod images;
mod monitor;
mod platform;
mod refresh;
mod search;
mod settings;
mod shared;
mod shortcut;
mod state;
mod store;
mod system_clipboard;
mod tray;
mod watcher;
mod window;

use tauri::{App, AppHandle, Manager};

use cli::Launch;
use state::State;

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
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tinydash_lib=info".into()),
        )
        .init();

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
        .register_asynchronous_uri_scheme_protocol("icon", images::serve_icon)
        .register_asynchronous_uri_scheme_protocol("clip", images::serve_clipboard_image)
        .manage(watcher::Watcher::default())
        .setup(move |app| {
            setup(app);
            open(app.handle(), launch, false);
            Ok(())
        })
        .on_window_event(window::on_event)
        .invoke_handler(tauri::generate_handler![
            commands::launcher_init,
            commands::search,
            commands::run_action,
            commands::preview,
            commands::hide_launcher,
            commands::get_settings,
            commands::update_settings,
            commands::pause_shortcut,
            commands::library_items,
            commands::save_library_item,
            commands::delete_library_item,
            commands::about,
        ])
        .run(tauri::generate_context!())
        .expect("TinyDash failed to start");
}

/// Load state and start background work. Problems become launcher warnings;
/// only a missing app folder location stops startup.
fn setup(app: &mut App) {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let paths = app.path();
    let dir = |dir: tauri::Result<std::path::PathBuf>| dir.expect("the OS reports app folders");
    let config_dir = dir(paths.app_config_dir());
    let data_dir = dir(paths.app_data_dir());
    let home_dir = dir(paths.home_dir());

    let mut warnings = Vec::new();
    let (settings, warning) = settings::load(&config_dir);
    warnings.extend(warning);
    let store = store::Store::open(&data_dir).unwrap_or_else(|error| {
        warnings.push(format!(
            "Could not open saved data ({error}). Changes this session will not be saved."
        ));
        store::Store::in_memory()
    });
    let shortcut = settings.shortcut.clone();
    let tray_visible = settings.show_tray_icon;
    app.manage(State::new(
        settings,
        store,
        [config_dir, data_dir, home_dir],
        warnings,
    ));

    let handle = app.handle();
    let state = handle.state::<State>();
    if let Err(error) = shortcut::register(handle, &shortcut) {
        state.warn(error.to_string());
    }
    if let Err(error) = tray::set_visible(handle, tray_visible) {
        state.warn(format!("The tray icon is unavailable: {error}"));
    }
    if let Some(launcher) = handle.get_webview_window(window::LAUNCHER) {
        platform::prepare_launcher(&launcher);
    }
    monitor::start(handle);
    watcher::watch(handle);
    refresh::apps(handle);
    refresh::files(handle);
    refresh::rates(handle, false);
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
