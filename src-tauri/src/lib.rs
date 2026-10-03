#[cfg(test)]
mod acl_tests;
mod appearance;
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
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use launcher::{
    LauncherState,
    warning::{LauncherWarning, WarningCode},
    window,
};

pub(crate) fn set_menu_bar_visible(app: &tauri::AppHandle, visible: bool) -> tauri::Result<()> {
    // Other desktops retain their tray regardless of this macOS preference.
    let visible = !cfg!(target_os = "macos") || visible;
    if let Some(tray) = app.tray_by_id("launcher") {
        tray.set_visible(visible)
    } else if visible {
        setup_tray(app)
    } else {
        Ok(())
    }
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
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

// Share the production ACL context with MockRuntime authorization tests.
// A single macro expansion also avoids duplicate macOS embedded plist symbols.
fn app_context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
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
        .runtime(tauri_runtime_wry::Wry::default())
        .manage(launcher::startup::Startup(std::sync::Mutex::new(request)))
        .manage(launcher::updates::UpdateState::default())
        .manage(launcher::paste::PasteState::default())
        .manage(launcher::transfer::TransferState::default())
        .manage(launcher::paste_queue::PasteQueueState::default())
        .manage(launcher::utilities::UtilitiesState::default())
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
            // Stay out of the Dock even when the menu bar icon is disabled or fails.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let mut warnings = Vec::new();
            let config = app.path().app_config_dir().map_err(anyhow::Error::from)
                .and_then(|directory| settings::load(&directory));
            let settings = match config {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::warn!(%error, "Using default settings");
                    warnings.push(LauncherWarning::new(WarningCode::SettingsRead, "Could not read settings. TinyDash is using the default settings.", false));
                    settings::Settings::fresh_install()
                }
            };

            if platform::is_wayland() {
                warnings.push(LauncherWarning::new(WarningCode::ShortcutsUnavailable, "Global shortcuts need X11. On Wayland, assign a desktop shortcut to start TinyDash.", false));
                if settings.clipboard_history_enabled {
                    warnings.push(LauncherWarning::new(WarningCode::ClipboardLimited, "Wayland can limit background clipboard access. Open TinyDash after copying text if an entry is missing.", false));
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
                        } else if let Some((id, _)) = settings.item_preferences.iter().find(|(_, item)| !item.disabled && !item.shortcut.is_empty() && matches(&item.shortcut)) {
                            if let Some(action) = launcher::commands::window_action(id) {
                                launcher::utilities::window_placement::shortcut(app, action);
                                return;
                            }
                            // Capture at the key press, before any asynchronous
                            // metadata or template work can outlive the foreground app.
                            if id.starts_with("library:") || id == "command:paste-next" {
                                launcher::paste::remember(app);
                            }
                            let app = app.clone();
                            let id = id.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(error) = launcher::commands::activate_shortcut(app.clone(), id).await {
                                    let _ = window::show(&app);
                                    use tauri::Emitter;
                                    let _ = app.emit("action-error", error);
                                }
                            });
                            return;
                        } else { return; };
                        if let Err(error) = result { tracing::warn!(%error, "Could not open launcher"); }
                    }).build()
                );
                if let Err(error) = shortcut_result {
                    tracing::warn!(%error, "Global shortcut is unavailable");
                    warnings.push(LauncherWarning::new(WarningCode::ShortcutsUnavailable, format!("Could not register {}. Start TinyDash again and open Settings to change the shortcut.", settings.shortcut), false));
                } else {
                    for shortcut in settings.shortcuts() {
                        if let Err(error) = app.global_shortcut().register(shortcut) {
                            tracing::warn!(%error, shortcut, "Global shortcut is unavailable");
                            warnings.push(LauncherWarning::new(WarningCode::ShortcutRegistration, format!("Could not register {shortcut}. Start TinyDash again and open Settings to change the shortcut."), true));
                        }
                    }
                }
            }

            if let Err(error) = set_menu_bar_visible(app.handle(), settings.show_menu_bar_icon) {
                tracing::warn!(%error, "Tray icon is unavailable");
                warnings.push(LauncherWarning::new(WarningCode::TrayUnavailable, "The tray icon is unavailable. Start TinyDash again to show the running launcher.", false));
                #[cfg(not(target_os = "macos"))]
                if let Some(window) = app.get_webview_window("main") { window.set_skip_taskbar(false)?; }
            }
            let library = launcher::library::LibraryState::open(app.path().app_data_dir()?)
                .unwrap_or_else(|error| {
                    warnings.push(LauncherWarning::new(WarningCode::StorageUnavailable, error.clone(), false));
                    launcher::library::LibraryState::unavailable(error)
                });
            app.manage(library);
            app.manage(LauncherState::new(settings, warnings));
            launcher::scan_apps(app.handle());
            launcher::app_watch::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
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
                if !launcher::transfer::active(window.app_handle()) && window.app_handle().try_state::<LauncherState>().is_some_and(|state| state.settings().hide_on_blur) => {
                if let Err(error) = window::hide(window.app_handle()) { tracing::warn!(%error, "Could not hide launcher"); }
            }
            _ => {}
        }})
        .invoke_handler(tauri::generate_handler![
            window::set_launcher_appearance,
            appearance::sync_appearance,
            launcher::launcher_ready,
            launcher::preferences::get_settings,
            launcher::commands::item_catalog,
            launcher::paste::paste_result,
            launcher::transfer::drag_result,
            launcher::paste_queue::paste_queue,
            launcher::library::library_list,
            launcher::library::library_get,
            launcher::library::library_save,
            launcher::library::library_delete,
            launcher::library::library_execute,
            launcher::file_actions::file_preview,
            launcher::file_actions::execute_file_action,
            launcher::utilities::utility_capabilities,
            launcher::utilities::utility_processes,
            launcher::utilities::utility_prepare_process,
            launcher::utilities::utility_prepare_app,
            launcher::utilities::utility_confirm_process,
            launcher::utilities::utility_cancel_process,
            launcher::utilities::utility_color,
            launcher::utilities::utility_copy_color,
            launcher::utilities::utility_eyedropper,
            launcher::utilities::utility_awake_status,
            launcher::utilities::utility_set_awake,
            launcher::utilities::utility_media,
            launcher::utilities::utility_capture_window,
            launcher::utilities::utility_window,
            launcher::preferences::save_settings,
            launcher::preferences::choose_clipboard_history,
            launcher::preferences::app_catalog,
            launcher::preferences::set_app_preference,
            launcher::preferences::preview_web_search,
            launcher::preferences::open_settings,
            launcher::preferences::set_shortcut_recording,
            launcher::preferences::reveal_settings_path,
            launcher::search,
            launcher::cancel_search,
            launcher::set_pinned,
            launcher::refresh_apps,
            launcher::files::refresh_files,
            launcher::currency::refresh_currency,
            launcher::quit_app,
            launcher::actions::execute_action,
            launcher::clipboard::clipboard_preview,
            launcher::clipboard::formats::rich_clipboard_history,
            launcher::clipboard::formats::rich_clipboard_preview,
            launcher::clipboard::formats::copy_rich_clipboard,
            launcher::clipboard::formats::delete_rich_clipboard,
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
            window::reset_launcher_position,
        ])
        .build(app_context())
        .context("Build the desktop launcher")?;
    app.run(|_app, _event| {
        if matches!(_event, tauri::RunEvent::Exit) {
            _app.state::<launcher::utilities::UtilitiesState>()
                .shutdown();
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
