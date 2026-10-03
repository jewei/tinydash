//! Transfers resolve catalog IDs in Rust; the WebView never supplies a file path.
use super::{LauncherState, actions::ResolvedAction, result::Action};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Default)]
pub struct TransferState(AtomicBool);

pub fn active(app: &AppHandle) -> bool {
    app.try_state::<TransferState>()
        .is_some_and(|state| state.0.load(Ordering::Acquire))
}

struct Operation(AppHandle);
impl Drop for Operation {
    fn drop(&mut self) {
        self.0
            .state::<TransferState>()
            .0
            .store(false, Ordering::Release);
    }
}

fn reserve(window: &WebviewWindow) -> Result<Operation, String> {
    if window.label() != "main" || !window.is_visible().map_err(|e| e.to_string())? {
        return Err("Open the launcher before transferring an item.".into());
    }
    if cfg!(test) || std::env::var_os("TINYDASH_DISABLE_UTILITY_EFFECTS").is_some() {
        return Err("Native transfers are disabled in this test session.".into());
    }
    window
        .state::<TransferState>()
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "A native transfer is already open.")?;
    Ok(Operation(window.app_handle().clone()))
}

pub fn resolve_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let resolved = {
        let state = app.state::<LauncherState>();
        let search = state.search.lock().map_err(|_| "Search is unavailable.")?;
        search
            .resolve_action(
                id,
                if id.starts_with("app:") {
                    Action::Launch
                } else {
                    Action::Open
                },
            )
            .map_err(|e| e.to_string())?
    };
    let path = match resolved {
        ResolvedAction::File(entry, _) => {
            entry.validate().map_err(|e| e.to_string())?;
            PathBuf::from(entry.path)
        }
        ResolvedAction::Launch(entry) => entry.path,
        _ => return Err("Only file, folder, and app results can be dragged.".into()),
    };
    if !path.is_absolute() || !path.exists() {
        return Err("This item is no longer available. Refresh the results.".into());
    }
    // App results can live under platform-managed symlinks. Resolve them here.
    path.canonicalize().map_err(|e| e.to_string())
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum DragOutcome {
    Dropped,
    Cancelled,
}

pub type DragCompletion = Box<dyn FnOnce(Result<DragOutcome, String>) + Send>;

#[tauri::command]
pub async fn drag_result(window: WebviewWindow, id: String) -> Result<DragOutcome, String> {
    let operation = reserve(&window)?;
    let app = window.app_handle().clone();
    let path = tauri::async_runtime::spawn_blocking(move || resolve_path(&app, &id))
        .await
        .map_err(|e| e.to_string())??;
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let native_window = window.clone();
    window
        .run_on_main_thread(move || {
            let completed = sender.clone();
            if let Err(error) = crate::platform::transfer::drag(
                &native_window,
                path,
                Box::new(move |result| {
                    let _ = completed.try_send(result);
                }),
            ) {
                let _ = sender.try_send(Err(error));
            }
        })
        .map_err(|e| e.to_string())?;
    let outcome = receiver
        .recv()
        .await
        .ok_or("The native drag session ended without a result.")??;
    drop(operation);
    if matches!(outcome, DragOutcome::Dropped)
        && window.state::<LauncherState>().settings().hide_on_blur
    {
        super::window::hide(window.app_handle()).map_err(|e| e.to_string())?;
    }
    Ok(outcome)
}
