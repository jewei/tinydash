fn main() {
    // An app manifest opts custom commands into Tauri's ACL enforcement.
    // Keep this inventory in sync with generate_handler!; acl_tests checks it.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "paste_result",
        "library_list",
        "library_get",
        "library_save",
        "library_delete",
        "library_execute",
        "file_preview",
        "execute_file_action",
        "utility_capabilities",
        "utility_processes",
        "utility_prepare_process",
        "utility_confirm_process",
        "utility_cancel_process",
        "utility_color",
        "utility_copy_color",
        "utility_eyedropper",
        "utility_awake_status",
        "utility_set_awake",
        "utility_media",
        "utility_capture_window",
        "utility_window",
        "rich_clipboard_history",
        "rich_clipboard_preview",
        "copy_rich_clipboard",
        "delete_rich_clipboard",
        "item_catalog",
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
        "cancel_search",
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
    let mut attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // tauri-winres/embed-resource attach Tauri's manifest only to bin
        // targets. Our lib unit-test executable also links Tauri/Muda's v6
        // imports (notably TaskDialogIndirect) through the real IPC tests.
        // Without an embedded v6 dependency, Windows fails before the harness
        // starts with STATUS_ENTRYPOINT_NOT_FOUND. Apply it to every linked
        // target, and omit the resource manifest to avoid duplicate ID 1s.
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::try_build(attributes).expect("build Tauri app with command permissions");
}
