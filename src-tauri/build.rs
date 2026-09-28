fn main() {
    // An app manifest opts custom commands into Tauri's ACL enforcement.
    // Keep this inventory in sync with generate_handler!; acl_tests checks it.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "set_launcher_appearance",
        "sync_appearance",
        "launcher_ready",
        "get_settings",
        "save_settings",
        "choose_clipboard_history",
        "app_catalog",
        "set_app_preference",
        "preview_web_search",
        "open_settings",
        "set_shortcut_recording",
        "reveal_settings_path",
        "search",
        "set_pinned",
        "refresh_apps",
        "refresh_files",
        "refresh_currency",
        "quit_app",
        "execute_action",
        "clipboard_preview",
        "clear_clipboard_history",
        "edit_clipboard_history",
        "copy_clipboard_selection",
        "export_settings",
        "preview_settings_import",
        "save_clipboard_file",
        "reveal_backup",
        "check_update",
        "install_update",
        "hide_launcher",
        "reset_launcher_position",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("build Tauri app with command permissions");
}
