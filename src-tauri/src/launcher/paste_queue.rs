//! A session-only queue of history IDs. Text is resolved just before dispatch.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, Manager};

use crate::providers::clipboard::{ClipboardEntry, ClipboardProvider, MAX_SELECTION_ENTRIES};

#[derive(Default)]
pub struct PasteQueueState(tauri::async_runtime::Mutex<Queue>);

#[derive(Clone, Default)]
struct Queue {
    ids: Vec<String>,
    position: usize,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum PasteQueueAction {
    Status,
    Start,
    Next,
    Skip,
    Cancel,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PasteQueueStatus {
    pub total: usize,
    pub position: usize,
    pub next: Option<ClipboardEntry>,
}

impl Queue {
    fn start(&mut self, ids: Vec<String>) -> Result<(), String> {
        if ids.is_empty() || ids.len() > MAX_SELECTION_ENTRIES {
            return Err("Select between 1 and 100 text entries.".into());
        }
        let mut seen = HashSet::new();
        if ids.iter().any(|id| {
            id.strip_prefix("clipboard:")
                .and_then(|value| value.parse::<i64>().ok())
                .is_none_or(|value| value <= 0 || *id != format!("clipboard:{value}"))
                || !seen.insert(id)
        }) {
            return Err("Select distinct clipboard text entries.".into());
        }
        self.ids = ids;
        self.position = 0;
        Ok(())
    }

    fn advance(&mut self) {
        self.position = (self.position + 1).min(self.ids.len());
    }

    fn resolve(&mut self, history: &ClipboardProvider) -> PasteQueueStatus {
        while self.position < self.ids.len() {
            if let Some(entry) = history.get(&self.ids[self.position]) {
                return PasteQueueStatus {
                    total: self.ids.len(),
                    position: self.position,
                    next: Some(entry.clone()),
                };
            }
            self.advance();
        }
        PasteQueueStatus {
            total: self.ids.len(),
            position: self.position,
            next: None,
        }
    }

    async fn dispatch(
        &mut self,
        operation: impl std::future::Future<Output = Result<(), String>>,
    ) -> Result<(), String> {
        operation.await?;
        self.advance();
        Ok(())
    }

    async fn status(&mut self, app: &AppHandle) -> Result<PasteQueueStatus, String> {
        let app = app.clone();
        let mut snapshot = self.clone();
        let status = tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<super::LauncherState>();
            let search = state
                .search
                .lock()
                .map_err(|_| "Clipboard history is unavailable.")?;
            Ok::<_, String>(snapshot.resolve(&search.clipboard))
        })
        .await
        .map_err(|error| error.to_string())??;
        self.position = status.position;
        Ok(status)
    }
}

pub fn command_action(id: &str) -> Option<PasteQueueAction> {
    match id {
        "command:paste-next" => Some(PasteQueueAction::Next),
        "command:paste-skip" => Some(PasteQueueAction::Skip),
        "command:paste-cancel" => Some(PasteQueueAction::Cancel),
        _ => None,
    }
}

#[tauri::command]
pub async fn paste_queue(
    app: AppHandle,
    action: PasteQueueAction,
    ids: Vec<String>,
) -> Result<PasteQueueStatus, String> {
    if action != PasteQueueAction::Start && !ids.is_empty() {
        return Err("Entry IDs are only accepted when starting a queue.".into());
    }
    let state = app.state::<PasteQueueState>();
    let mut queue = state
        .0
        .try_lock()
        .map_err(|_| "The paste queue is busy. Try again.")?;
    match action {
        PasteQueueAction::Start => {
            // Reject replacement so an accidental second start cannot discard work.
            if queue.status(&app).await?.next.is_some() {
                return Err("Cancel the current paste queue before starting another.".into());
            }
            let mut replacement = Queue::default();
            replacement.start(ids)?;
            // Do not silently accept a stale selection at creation.
            let check_app = app.clone();
            let selected = replacement.ids.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = check_app.state::<super::LauncherState>();
                let search = state
                    .search
                    .lock()
                    .map_err(|_| "Clipboard history is unavailable.")?;
                if selected.iter().any(|id| search.clipboard.get(id).is_none()) {
                    return Err(
                        "A selected entry was removed. Select the entries again.".to_string()
                    );
                }
                Ok(())
            })
            .await
            .map_err(|error| error.to_string())??;
            *queue = replacement;
        }
        PasteQueueAction::Next => {
            let target = super::paste::prepare(&app)?;
            let next = queue
                .status(&app)
                .await?
                .next
                .ok_or("No entries remain in the paste queue.")?;
            // Failed dispatch leaves this entry selected for an explicit retry.
            queue
                .dispatch(super::paste::paste_text(&app, next.content, target))
                .await?;
        }
        PasteQueueAction::Skip => {
            if queue.status(&app).await?.next.is_some() {
                queue.advance();
            }
        }
        PasteQueueAction::Cancel => *queue = Queue::default(),
        PasteQueueAction::Status => {}
    }
    let status = queue.status(&app).await?;
    if action != PasteQueueAction::Status
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.emit("paste-queue-changed", &status);
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history() -> ClipboardProvider {
        ClipboardProvider::new(
            (1..=3)
                .map(|id| ClipboardEntry {
                    id,
                    content: format!("Text {id}"),
                    created_at: id,
                    last_used_at: None,
                })
                .collect(),
        )
    }

    #[test]
    fn deleted_entries_are_skipped_and_cleared_text_is_never_retained() {
        let mut history = history();
        let mut queue = Queue::default();
        queue
            .start(vec![
                "clipboard:2".into(),
                "clipboard:1".into(),
                "clipboard:3".into(),
            ])
            .unwrap();
        assert_eq!(queue.resolve(&history).next.unwrap().content, "Text 2");
        history.remove(2);
        let status = queue.resolve(&history);
        assert_eq!(status.position, 1);
        assert_eq!(status.next.unwrap().id, 1);
        history.remove_many(&[1, 3]);
        let status = queue.resolve(&history);
        assert_eq!(status.position, status.total);
        assert!(status.next.is_none());
        assert!(queue.resolve(&history).next.is_none());
    }

    #[test]
    fn failed_dispatch_keeps_entry_and_success_advances_once() {
        tauri::async_runtime::block_on(async {
            let mut queue = Queue::default();
            queue
                .start(vec!["clipboard:2".into(), "clipboard:1".into()])
                .unwrap();
            assert!(
                queue
                    .dispatch(async { Err("Focus changed".into()) })
                    .await
                    .is_err()
            );
            assert_eq!(queue.resolve(&history()).next.unwrap().id, 2);
            queue.dispatch(async { Ok(()) }).await.unwrap();
            assert_eq!(queue.resolve(&history()).next.unwrap().id, 1);
        });
    }

    #[test]
    fn operation_lock_rejects_repeat_presses_and_replacement_during_dispatch() {
        let state = PasteQueueState::default();
        let operation = state.0.try_lock().unwrap();
        assert!(state.0.try_lock().is_err());
        drop(operation);
        assert!(state.0.try_lock().is_ok());
    }

    #[test]
    fn queue_keeps_selected_order_and_rejects_invalid_replacement() {
        let mut queue = Queue::default();
        queue
            .start(vec!["clipboard:3".into(), "clipboard:1".into()])
            .unwrap();
        for invalid in [
            vec![],
            vec!["file:1".into()],
            vec!["clipboard:0".into()],
            vec!["clipboard:01".into()],
            vec!["clipboard:1".into(); 2],
            (1..=101).map(|i| format!("clipboard:{i}")).collect(),
        ] {
            assert!(queue.start(invalid).is_err());
            assert_eq!(queue.ids, ["clipboard:3", "clipboard:1"]);
        }
        queue.advance();
        assert_eq!(queue.ids[queue.position], "clipboard:1");
        queue.advance();
        queue.advance();
        assert_eq!(queue.position, 2);
    }
}
