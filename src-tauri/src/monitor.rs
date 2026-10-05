//! Clipboard capture. A background thread checks the OS change counter
//! twice a second and reads content only when it changes.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::{events, platform, state::State};

const INTERVAL: Duration = Duration::from_millis(500);

/// Start capturing. Call on the main thread; Linux registers a GTK handler.
pub fn start(app: &AppHandle) {
    let handle = app.clone();
    platform::watch_clipboard(move || {
        handle
            .state::<State>()
            .settings
            .get()
            .clipboard_history_enabled
    });
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || capture_forever(&app));
    if let Err(error) = spawned {
        tracing::warn!(%error, "Clipboard history is unavailable");
    }
}

fn capture_forever(app: &AppHandle) {
    let state = app.state::<State>();
    // Content that was already on the clipboard at startup is not captured.
    let mut seen = platform::clipboard_change();
    loop {
        std::thread::sleep(INTERVAL);
        let change = platform::clipboard_change();
        if change == seen {
            continue;
        }
        seen = change;
        let settings = state.settings.get();
        if !settings.clipboard_history_enabled {
            continue;
        }
        let content = platform::read_clipboard(
            settings.clipboard_capture_images,
            settings.clipboard_capture_files,
        );
        // Skip content that changed while it was read; the next tick sees it.
        let Some(content) = content.filter(|c| c.fits() && platform::clipboard_change() == change)
        else {
            continue;
        };
        let now = chrono::Utc::now().timestamp();
        let saved = state
            .store
            .save_clip(&content, now, settings.clipboard_history_limit)
            .and_then(|()| state.reload_clipboard());
        match saved {
            Ok(()) => events::results_stale(app),
            Err(error) => tracing::warn!(%error, "Could not save a clipboard entry"),
        }
    }
}
