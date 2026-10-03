//! Bounded restore history for direct window commands and global shortcuts.
use super::{UtilitiesState, UtilityResult, WindowAction, desktop};
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub(super) struct State {
    busy: AtomicBool,
    history: Mutex<VecDeque<(desktop::WindowTarget, Instant)>>,
}

impl State {
    pub fn clear(&self) {
        if let Ok(mut history) = self.history.try_lock() {
            history.clear();
        }
    }
}

struct Operation(AppHandle);
impl Drop for Operation {
    fn drop(&mut self) {
        self.0
            .state::<UtilitiesState>()
            .placements
            .busy
            .store(false, Ordering::Release);
    }
}

fn reserve(app: &AppHandle) -> Option<Operation> {
    app.state::<UtilitiesState>()
        .placements
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Operation(app.clone()))
}

pub fn shortcut(app: &AppHandle, action: WindowAction) {
    // Discard key repeat while an operation is in progress; never queue moves.
    let Some(operation) = reserve(app) else {
        return;
    };
    let target = crate::launcher::paste::foreground_target();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = match target {
            Ok(id) => apply(operation, id, action, true).await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            let _ = crate::launcher::window::show(&app);
            let _ = app.emit("action-error", error);
        }
    });
}

pub async fn previous(app: &AppHandle, action: WindowAction) -> UtilityResult<()> {
    let operation = reserve(app).ok_or("A window placement is already in progress.")?;
    let id = crate::launcher::paste::previous_target(app)?;
    apply(operation, id, action, false).await
}

async fn apply(
    operation: Operation,
    id: u64,
    action: WindowAction,
    foreground_only: bool,
) -> UtilityResult<()> {
    super::blocking(move || {
        let app = &operation.0;
        let current = desktop::capture_target(id)?;
        if foreground_only && !current.is_focused()? {
            return Err("Focus changed. No window placement was requested.".into());
        }
        let state = app.state::<UtilitiesState>();
        let mut history = state
            .placements
            .history
            .lock()
            .map_err(|_| "Window history is unavailable")?;
        // Retain at most 16 windows. Old entries also expire on the next command.
        history.retain(|(_, used)| used.elapsed() < Duration::from_secs(30 * 60));
        let index = history
            .iter()
            .position(|(saved, _)| saved.same_window(&current));
        let target = if matches!(action, WindowAction::Restore) {
            history
                .get(index.ok_or(
                    "No saved position for this window. Place it with a window command first.",
                )?)
                .map(|(saved, _)| saved.clone())
                .ok_or("Window history is unavailable")?
        } else {
            if let Some(index) = index {
                let (saved, _) = history
                    .remove(index)
                    .ok_or("Window history is unavailable")?;
                history.push_back((saved, Instant::now()));
            } else {
                if history.len() == 16 {
                    history.pop_front();
                }
                // Save before mutation so a partial native failure can still be restored.
                history.push_back((current.clone(), Instant::now()));
            }
            current.clone()
        };
        #[cfg(target_os = "macos")]
        desktop::window(app, &target, action, foreground_only)?;
        #[cfg(not(target_os = "macos"))]
        desktop::window(&target, action, foreground_only)?;
        if matches!(action, WindowAction::Restore) {
            history.retain(|(saved, _)| !saved.same_window(&current));
        }
        Ok(())
    })
    .await
}
