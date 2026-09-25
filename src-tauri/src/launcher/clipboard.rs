use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};

use tauri::{AppHandle, Emitter, Manager};

use super::LauncherState;
#[cfg(target_os = "linux")]
use crate::providers::clipboard::Observed;
use crate::{error::Error, providers::clipboard::ClipboardEntry};

#[derive(Default)]
pub struct Monitor {
    sender: OnceLock<SyncSender<()>>,
    stopped: AtomicBool,
    generation: AtomicU64,
    warning: Mutex<Option<String>>,
    #[cfg(target_os = "linux")]
    read_sequence: AtomicU64,
    #[cfg(target_os = "linux")]
    pending: Mutex<Option<(Observed, u64)>>,
}

impl Monitor {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.wake();
    }
    fn wake(&self) {
        if let Some(sender) = self.sender.get() {
            let _ = sender.try_send(());
        }
    }
    pub fn warning(&self) -> Option<String> {
        self.warning.lock().ok().and_then(|warning| warning.clone())
    }
    fn failed(&self) {
        if let Ok(mut warning) = self.warning.lock()
            && warning.is_none()
        {
            tracing::warn!("Could not read clipboard changes");
            *warning = Some(
                "Clipboard capture is unavailable. TinyDash will try again after the next change."
                    .into(),
            );
        }
    }
}

/// Copies a generated secret with the platform markers that tell TinyDash and
/// other clipboard managers not to record it. The markers stay with the
/// clipboard, so the value is also skipped after TinyDash restarts.
pub fn write_secret(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    let set = clipboard.set();
    #[cfg(target_os = "macos")]
    let set = arboard::SetExtApple::exclude_from_history(set);
    #[cfg(target_os = "windows")]
    let set = {
        use arboard::SetExtWindows;
        set.exclude_from_monitoring()
            .exclude_from_history()
            .exclude_from_cloud()
    };
    #[cfg(target_os = "linux")]
    let set = arboard::SetExtLinux::exclude_from_history(set);
    set.text(text)
}

pub fn changed(app: &AppHandle) {
    if let Err(error) = app.emit("clipboard-changed", ()) {
        tracing::debug!(%error, "No clipboard listener");
    }
}

pub fn start(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    if !state.settings().clipboard_history_enabled {
        return;
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    if state.clipboard.sender.set(sender).is_err() {
        return;
    }
    let worker_app = app.clone();
    let worker = std::thread::Builder::new()
        .name("clipboard-history".into())
        .spawn(move || {
            let state = worker_app.state::<LauncherState>();
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                let mut previous = None;
                loop {
                    if state.clipboard.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let generation = state.clipboard.generation();
                    if state.settings().clipboard_history_enabled {
                        match crate::platform::clipboard_snapshot(previous) {
                            Ok(Some((counter, observed))) => {
                                previous = Some(counter);
                                if let Ok(mut warning) = state.clipboard.warning.lock() {
                                    *warning = None;
                                }
                                state.storage.capture(&worker_app, observed, generation);
                            }
                            Ok(None) => {}
                            Err(_) => state.clipboard.failed(), // Never log clipboard contents.
                        }
                    } else {
                        previous = None;
                    }
                    if matches!(
                        receiver.recv_timeout(std::time::Duration::from_secs(1)),
                        Err(mpsc::RecvTimeoutError::Disconnected)
                    ) {
                        break;
                    }
                }
            }
            #[cfg(target_os = "linux")]
            while receiver.recv().is_ok() {
                if state.clipboard.stopped.load(Ordering::Acquire) {
                    break;
                }
                let pending = state
                    .clipboard
                    .pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.take());
                if let Some((observed, generation)) = pending {
                    state.storage.capture(&worker_app, observed, generation);
                }
            }
        });
    if worker.is_err() {
        state.clipboard.failed();
    }
    #[cfg(target_os = "linux")]
    if worker.is_ok() {
        let event_app = app.clone();
        if app
            .run_on_main_thread(move || {
                let callback_app = event_app.clone();
                crate::platform::watch_clipboard(move || request_linux(&callback_app));
                request_linux(&event_app);
            })
            .is_err()
        {
            state.clipboard.failed();
        }
    }
}

pub fn refresh(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    if !state.settings().clipboard_history_enabled || state.clipboard.sender.get().is_none() {
        return;
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    state.clipboard.wake();
    #[cfg(target_os = "linux")]
    {
        let app_clone = app.clone();
        if app
            .run_on_main_thread(move || request_linux(&app_clone))
            .is_err()
        {
            state.clipboard.failed();
        }
    }
}

#[cfg(target_os = "linux")]
fn request_linux(app: &AppHandle) {
    let state = app.state::<LauncherState>();
    if state.clipboard.stopped.load(Ordering::Acquire)
        || !state.settings().clipboard_history_enabled
    {
        return;
    }
    let generation = state.clipboard.generation();
    let sequence = state
        .clipboard
        .read_sequence
        .fetch_add(1, Ordering::AcqRel)
        .wrapping_add(1);
    let app = app.clone();
    crate::platform::read_clipboard(move |observed| {
        let state = app.state::<LauncherState>();
        if sequence != state.clipboard.read_sequence.load(Ordering::Acquire) {
            return;
        }
        // Keep only the latest pending text. A burst of owner changes cannot
        // allocate an unbounded queue while SQLite is busy.
        if let Ok(mut pending) = state.clipboard.pending.lock() {
            *pending = Some((observed, generation));
        }
        state.clipboard.wake();
    });
}

#[tauri::command]
pub async fn clipboard_preview(id: String, app: AppHandle) -> Result<ClipboardEntry, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LauncherState>()
            .search
            .lock()
            .map_err(|_| Error::IndexUnavailable.to_string())?
            .clipboard_entry(&id)
            .cloned()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn clear_clipboard_history(
    keep_pinned: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let storage = &app.state::<LauncherState>().storage;
        if keep_pinned.unwrap_or(false) {
            storage.clear_unpinned_clipboard(&app)
        } else {
            storage.delete_clipboard(&app, None)
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn edit_clipboard_history(
    id: String,
    text: String,
    app: AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LauncherState>()
            .storage
            .edit_clipboard(&app, &id, text)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn copy_clipboard_selection(
    ids: Vec<String>,
    separator: String,
    app: AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<LauncherState>()
            .storage
            .copy_clipboard_selection(&app, &ids, &separator)
    })
    .await
    .map_err(|error| error.to_string())?
}
