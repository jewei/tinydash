mod currency;
mod db;
mod error;
mod launcher;
mod platform;
mod providers;
mod ranking;
mod settings;

use anyhow::Context;
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use launcher::{LauncherState, window};

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open TinyDash", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh applications", true, None::<&str>)?;
    let files = MenuItem::with_id(app, "files", "Refresh files", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit TinyDash", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &refresh, &files, &quit])?;
    // Two small dashes form a legible monochrome menu-bar icon.
    let mut rgba = vec![0_u8; 22 * 22 * 4];
    for y in 0..22 {
        for x in 0..22 {
            if ((6..9).contains(&y) && (4..15).contains(&x))
                || ((13..16).contains(&y) && (7..18).contains(&x))
            {
                let offset = (y * 22 + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&[175, 236, 214, 255]);
            }
        }
    }
    TrayIconBuilder::with_id("launcher")
        .icon(tauri::image::Image::new_owned(rgba, 22, 22))
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("TinyDash")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Err(error) = window::show(app) {
                    tracing::warn!(%error, "Could not show launcher");
                }
            }
            "refresh" => launcher::scan_apps(app),
            "files" => launcher::files::scan_files(app),
            "settings" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = launcher::preferences::open_settings(app).await {
                        tracing::warn!(%error, "Could not open settings");
                    }
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) && let Err(error) = window::show(tray.app_handle())
            {
                tracing::warn!(%error, "Could not show launcher");
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--help"] || args.as_slice() == ["-h"] {
        println!(
            "TinyDash [--settings | --background | --mode CATEGORY]\nCategories: all, apps, files, clipboard, calculator, system, emoji, password, timezone, url, web.\nA category command opens that category with an empty query."
        );
        return Ok(());
    }
    let request = launcher::startup::LaunchRequest::parse(args)?;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tinydash_lib=info".into()),
        )
        .try_init();
    let app = tauri::Builder::default()
        .manage(launcher::startup::Startup(std::sync::Mutex::new(request)))
        .manage(launcher::updates::UpdateState::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            match launcher::startup::LaunchRequest::parse(args.into_iter().skip(1)) {
                Ok(request) => launcher::startup::activate(app, request),
                Err(error) => tracing::warn!(%error, "Invalid launch command"),
            }
        }))
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().args(["--background"]).build())
        .plugin(tauri_plugin_updater::Builder::new().pubkey(option_env!("TAURI_UPDATER_PUBLIC_KEY").unwrap_or("")).build())
        .setup(|app| {
            let mut warnings = Vec::new();
            let config = app.path().app_config_dir().map_err(anyhow::Error::from)
                .and_then(|directory| settings::load(&directory));
            let settings = match config {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(%error, "Using default settings");
                    warnings.push("Could not read settings. TinyDash is using the default settings.".into());
                    settings::Settings::fresh_install()
                }
            };

            if platform::is_wayland() {
                warnings.push("Global shortcuts need X11. On Wayland, assign a desktop shortcut to start TinyDash.".into());
                if settings.clipboard_history_enabled {
                    warnings.push("Wayland can limit background clipboard access. Open TinyDash after copying text if an entry is missing.".into());
                }
            } else {
                let shortcut_result = app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
                        if event.state() != ShortcutState::Pressed { return; }
                        let Some(state) = app.try_state::<LauncherState>() else { return; };
                        if state.shortcut_recording.load(std::sync::atomic::Ordering::Acquire) { return; }
                        let settings = state.settings();
                        let matches = |value: &str| value.parse::<tauri_plugin_global_shortcut::Shortcut>().is_ok_and(|active| active.id() == shortcut.id());
                        let result = if matches(&settings.shortcut) {
                            window::toggle(app)
                        } else if let Some(binding) = settings.category_shortcuts.iter().find(|binding| matches(&binding.shortcut)) {
                            window::show_category(app, binding.mode)
                        } else { return; };
                        if let Err(error) = result { tracing::warn!(%error, "Could not open launcher"); }
                    }).build()
                );
                if let Err(error) = shortcut_result {
                    tracing::warn!(%error, "Global shortcut is unavailable");
                    warnings.push(format!("Could not register {}. Use the tray menu or change settings.json.", settings.shortcut));
                } else {
                    for shortcut in settings.shortcuts() {
                        if let Err(error) = app.global_shortcut().register(shortcut) {
                            tracing::warn!(%error, shortcut, "Global shortcut is unavailable");
                            warnings.push(format!("Could not register {shortcut}. Use the tray menu or change Settings."));
                        }
                    }
                }
            }

            if let Err(error) = setup_tray(app) {
                tracing::warn!(%error, "Tray icon is unavailable");
                warnings.push("The tray icon is unavailable. Start TinyDash again to show the running launcher.".into());
                if let Some(window) = app.get_webview_window("main") { window.set_skip_taskbar(false)?; }
            } else {
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            app.manage(LauncherState::new(settings, warnings));
            launcher::scan_apps(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Some(state) = window.app_handle().try_state::<LauncherState>() {
                state.icons.cancel_window(window.label(), || {
                    let _ = window.app_handle().emit("app-icons-ready", ());
                });
            }
            if window.label() == "settings" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        // Keep an unfinished settings form when its window closes.
                        api.prevent_close();
                        launcher::preferences::restore_shortcut_in_background(window.app_handle());
                        if let Err(error) = window.hide() { tracing::warn!(%error, "Could not hide settings"); }
                    }
                    tauri::WindowEvent::Focused(false) => launcher::preferences::restore_shortcut_in_background(window.app_handle()),
                    _ => {}
                }
                return;
            }
            match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if let Err(error) = window::dismiss(window.app_handle()) { tracing::warn!(%error, "Could not hide launcher"); }
            }
            tauri::WindowEvent::Focused(false)
                if window.app_handle().try_state::<LauncherState>().is_some_and(|state| state.settings().hide_on_blur) => {
                if let Err(error) = window::hide(window.app_handle()) { tracing::warn!(%error, "Could not hide launcher"); }
            }
            _ => {}
        }})
        .invoke_handler(tauri::generate_handler![
            launcher::launcher_ready,
            launcher::icons::app_icon,
            launcher::icons::cancel_app_icon,
            launcher::preferences::get_settings,
            launcher::preferences::save_settings,
            launcher::preferences::choose_clipboard_history,
            launcher::preferences::app_catalog,
            launcher::preferences::set_app_preference,
            launcher::preferences::preview_web_search,
            launcher::preferences::open_settings,
            launcher::preferences::set_shortcut_recording,
            launcher::preferences::reveal_settings_path,
            launcher::search,
            launcher::set_pinned,
            launcher::refresh_apps,
            launcher::files::refresh_files,
            launcher::currency::refresh_currency,
            launcher::quit_app,
            launcher::actions::execute_action,
            launcher::clipboard::clipboard_preview,
            launcher::clipboard::clear_clipboard_history,
            launcher::clipboard::edit_clipboard_history,
            launcher::clipboard::copy_clipboard_selection,
            launcher::portability::export_settings,
            launcher::portability::preview_settings_import,
            launcher::portability::save_clipboard_file,
            launcher::portability::reveal_backup,
            launcher::updates::check_update,
            launcher::updates::install_update,
            window::hide_launcher,
        ])
        .build(tauri::generate_context!())
        .context("Build the desktop launcher")?;
    app.run(|_app, _event| {
        if matches!(_event, tauri::RunEvent::Exit) {
            _app.state::<LauncherState>().clipboard.stop();
            _app.state::<LauncherState>().files.stop();
        }
        // Launch Services sends Reopen when an existing .app is opened again.
        // This is separate from starting a second executable process.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = _event
            && let Err(error) = window::show(_app)
        {
            tracing::warn!(%error, "Could not reopen launcher");
        }
    });
    Ok(())
}
