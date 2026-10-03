//! These tests use the generated production context, not mock_context (which has
//! no app manifest). MockRuntime replaces the OS, not Tauri's IPC authorization.
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tauri::{
    Listener, WebviewWindow, WebviewWindowBuilder,
    ipc::{CallbackFn, InvokeBody},
    test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder},
    webview::InvokeRequest,
};

// This is intentionally a test in the library harness, not a separate binary:
// tauri-build's default resource linking already covers binary targets.
#[cfg(all(target_os = "windows", target_env = "msvc"))]
#[test]
fn windows_ipc_test_executable_loads_common_controls_v6() {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    let module_name = "comctl32.dll\0".encode_utf16().collect::<Vec<_>>();
    // SAFETY: the module name is NUL-terminated; GetModuleHandleW does not
    // transfer ownership, and GetProcAddress only inspects the loaded module.
    unsafe {
        let module = GetModuleHandleW(module_name.as_ptr());
        assert!(!module.is_null(), "Tauri must load Common Controls");
        assert!(
            GetProcAddress(module, c"TaskDialogIndirect".as_ptr().cast()).is_some(),
            "the test executable must select Common Controls v6, just like the app"
        );
    }
}

const MAIN_ONLY: &[&str] = &[
    "paste_result",
    "drag_result",
    "share_result",
    "paste_queue",
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
    "utility_prepare_app",
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
    "set_launcher_appearance",
    "launcher_ready",
    "choose_clipboard_history",
    "set_app_preference",
    "open_settings",
    "search",
    "cancel_search",
    "set_pinned",
    "refresh_apps",
    "refresh_files",
    "refresh_currency",
    "quit_app",
    "execute_action",
    "clipboard_preview",
    "edit_clipboard_history",
    "copy_clipboard_selection",
    "save_clipboard_file",
    "hide_launcher",
    "reset_launcher_position",
];
const SETTINGS_ONLY: &[&str] = &[
    "get_settings",
    "save_settings",
    "item_catalog",
    "preview_web_search",
    "set_shortcut_recording",
    "reveal_settings_path",
    "export_settings",
    "preview_settings_import",
    "reveal_backup",
    "check_update",
    "install_update",
];
const SHARED: &[&str] = &["clear_clipboard_history", "sync_appearance", "app_catalog"];

fn commands() -> Vec<&'static str> {
    MAIN_ONLY
        .iter()
        .chain(SETTINGS_ONLY)
        .chain(SHARED)
        .copied()
        .collect()
}

fn window(app: &tauri::App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
    WebviewWindowBuilder::new(app, label, Default::default())
        .build()
        .unwrap()
}

fn invoke(
    window: &WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
    remote: bool,
) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if remote {
                "https://untrusted.example/"
            } else {
                // Tauri 3's MockRuntime uses this scheme on every OS. Wry's
                // Windows HTTP mapping belongs to the real desktop checks.
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

fn denied(result: Result<Value, Value>, command: &str, label: &str) {
    let error = result.expect_err(&format!("{label} must not invoke {command}"));
    assert!(
        error
            .as_str()
            .is_some_and(|error| error.contains("not allowed")),
        "{label}/{command}: expected ACL rejection, not a missing handler or bad arguments: {error}"
    );
}

#[test]
fn registered_commands_and_manifest_match_the_test_matrix() {
    let handler = include_str!("lib.rs")
        .split(".invoke_handler(tauri::generate_handler![")
        .nth(1)
        .unwrap()
        .split("])")
        .next()
        .unwrap();
    let mut registered = handler
        .split(',')
        .filter_map(|name| {
            let name = name.trim();
            (!name.is_empty()).then(|| name.rsplit("::").next().unwrap())
        })
        .collect::<Vec<_>>();
    let mut manifest = include_str!("../build.rs")
        .split(".commands(&[")
        .nth(1)
        .unwrap()
        .split("])")
        .next()
        .unwrap()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.trim_matches('"'))
        .collect::<Vec<_>>();
    let mut expected = commands();
    registered.sort_unstable();
    manifest.sort_unstable();
    expected.sort_unstable();
    assert_eq!(
        registered, expected,
        "review the window grants for every registered command"
    );
    assert_eq!(manifest, expected, "every command must be in AppManifest");
}

#[test]
fn production_ipc_acl_enforces_every_app_command_by_window_and_origin() {
    let reached = Arc::new(Mutex::new(Vec::new()));
    let calls = reached.clone();
    let app = mock_builder()
        // A side-effect-free handler proves whether IPC reaches dispatch. It
        // deliberately accepts even unknown commands: ACL must reject first.
        .invoke_handler(move |invoke| {
            calls
                .lock()
                .unwrap()
                .push(invoke.message.command().to_owned());
            invoke.resolver.resolve("reached handler");
            true
        })
        .build(crate::app_context())
        .unwrap();
    for label in ["main", "settings", "untrusted", "settings-child"] {
        let webview = window(&app, label);
        for command in commands().into_iter().chain(["unlisted_future_command"]) {
            let allowed = match label {
                "main" => MAIN_ONLY.contains(&command) || SHARED.contains(&command),
                "settings" => SETTINGS_ONLY.contains(&command) || SHARED.contains(&command),
                _ => false,
            };
            for remote in [false, true] {
                reached.lock().unwrap().clear();
                let result = invoke(&webview, command, json!({}), remote);
                if allowed && !remote {
                    assert_eq!(
                        result.unwrap(),
                        json!("reached handler"),
                        "{label}/{command}"
                    );
                    assert_eq!(*reached.lock().unwrap(), [command]);
                } else {
                    denied(result, command, label);
                    assert!(
                        reached.lock().unwrap().is_empty(),
                        "unauthorized dispatch: {label}/{command}"
                    );
                }
            }
        }
    }
}

#[test]
fn frontend_can_subscribe_but_cannot_spoof_backend_events_or_call_plugins() {
    let app = mock_builder().build(crate::app_context()).unwrap();
    for label in ["main", "settings", "untrusted"] {
        let webview = window(&app, label);
        let subscription = invoke(
            &webview,
            "plugin:event|listen",
            json!({
                "event": "settings-changed", "target": {"kind": "Any"}, "handler": 0,
            }),
            false,
        );
        if label == "untrusted" {
            denied(subscription, "plugin:event|listen", label);
        } else {
            let event_id = subscription.unwrap();
            assert!(event_id.is_number());
            assert!(
                invoke(
                    &webview,
                    "plugin:event|unlisten",
                    json!({
                        "event": "settings-changed", "eventId": event_id,
                    }),
                    false
                )
                .is_ok()
            );
        }
        for command in [
            "plugin:event|emit",
            "plugin:event|emit_to",
            "plugin:updater|check",
            "plugin:updater|download_and_install",
            "plugin:opener|open_url",
            "plugin:dialog|open",
            "plugin:clipboard-manager|read_text",
            "plugin:window|create",
        ] {
            denied(
                invoke(
                    &webview,
                    command,
                    json!({
                        "event": "settings-changed", "payload": {},
                        "target": {"kind": "Any"},
                    }),
                    false,
                ),
                command,
                label,
            );
        }
    }
    // Check native drag permission through the same generated runtime authority;
    // actually dragging belongs to desktop tests, not this mock runtime.
    let mut context = crate::app_context::<MockRuntime>();
    let acl = context.runtime_authority_mut();
    for label in ["main", "settings", "untrusted"] {
        assert_eq!(
            acl.resolve_access(
                "plugin:window|start_dragging",
                label,
                label,
                &tauri::ipc::Origin::Local
            )
            .is_some(),
            label == "main"
        );
    }
}

#[test]
fn typed_appearance_relay_preserves_both_window_flows_without_arbitrary_events() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![crate::appearance::sync_appearance])
        .build(crate::app_context())
        .unwrap();
    let main = window(&app, "main");
    let settings = window(&app, "settings");
    let other = window(&app, "untrusted");
    let received = Arc::new(Mutex::new(Vec::new()));
    for webview in [&main, &settings, &other] {
        for event in [
            "appearance-changed",
            "compact-changed",
            "system-glass-changed",
            "settings-changed",
        ] {
            let received = received.clone();
            let label = webview.label().to_owned();
            webview.listen(event, move |message| {
                received.lock().unwrap().push((
                    label.clone(),
                    event.to_owned(),
                    serde_json::from_str::<Value>(message.payload()).unwrap(),
                ));
            });
        }
    }
    for source in [&main, &settings] {
        for (kind, value, event) in [
            ("appearance", json!("sage"), "appearance-changed"),
            ("compact", json!(true), "compact-changed"),
            ("systemGlass", json!(false), "system-glass-changed"),
        ] {
            received.lock().unwrap().clear();
            assert!(
                invoke(
                    source,
                    "sync_appearance",
                    json!({
                        "change": {"kind": kind, "value": value},
                    }),
                    false
                )
                .is_ok()
            );
            let mut actual = received.lock().unwrap().clone();
            actual.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(
                actual,
                vec![
                    ("main".into(), event.into(), value.clone()),
                    ("settings".into(), event.into(), value),
                ]
            );
        }
        for change in [
            json!({"kind": "settings-changed", "value": {}}),
            json!({"kind": "appearance", "value": "invalid-theme"}),
            json!({"kind": "compact", "value": "true"}),
            json!({"kind": "systemGlass", "value": null}),
        ] {
            received.lock().unwrap().clear();
            let error =
                invoke(source, "sync_appearance", json!({"change": change}), false).unwrap_err();
            assert!(error.as_str().unwrap().contains("invalid args"), "{error}");
            assert!(received.lock().unwrap().is_empty());
        }
    }
    denied(
        invoke(
            &other,
            "sync_appearance",
            json!({
                "change": {"kind": "appearance", "value": "sage"},
            }),
            false,
        ),
        "sync_appearance",
        "untrusted",
    );
}
