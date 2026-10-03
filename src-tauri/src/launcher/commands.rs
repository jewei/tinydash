//! Metadata for implemented native views. No dynamic registration or plugin runtime.
use super::{
    LauncherState,
    result::{Action, ResultKind, SearchResult},
};
use tauri::{AppHandle, Emitter, Manager};

const COMMANDS: &[(&str, &str, &str)] = &[
    (
        "paste-next",
        "Paste next queued entry",
        "Paste one text entry without opening the launcher",
    ),
    (
        "paste-skip",
        "Skip queued entry",
        "Skip the next entry in the paste queue",
    ),
    (
        "paste-cancel",
        "Cancel paste queue",
        "Release the current text paste queue",
    ),
    (
        "quicklinks",
        "Quicklinks",
        "Saved websites, files, folders and app links",
    ),
    ("snippets", "Snippets", "Reusable text and templates"),
    (
        "processes",
        "Quit a process",
        "Search running processes and safely stop one",
    ),
    ("colors", "Color picker", "Preview and convert colors"),
    ("awake", "Keep awake", "Prevent sleep for a limited time"),
    ("media", "Media controls", "Playback and volume controls"),
    (
        "windows",
        "Window management",
        "Arrange the previous application's window",
    ),
    (
        "window-left",
        "Move window left",
        "Place the active window in the left half",
    ),
    (
        "window-right",
        "Move window right",
        "Place the active window in the right half",
    ),
    (
        "window-maximize",
        "Maximize window",
        "Fill the available work area",
    ),
    (
        "window-center",
        "Center window",
        "Center the window at its current size",
    ),
    (
        "window-restore",
        "Restore window",
        "Restore the window's saved position and size",
    ),
    (
        "rich-clipboard",
        "Images and files clipboard",
        "Opt-in image and file history",
    ),
];

pub fn window_action(id: &str) -> Option<super::utilities::WindowAction> {
    use super::utilities::WindowAction;
    Some(match id {
        "command:window-left" => WindowAction::Left,
        "command:window-right" => WindowAction::Right,
        "command:window-maximize" => WindowAction::Maximize,
        "command:window-center" => WindowAction::Center,
        "command:window-restore" => WindowAction::Restore,
        _ => return None,
    })
}

pub fn catalog() -> Vec<SearchResult> {
    COMMANDS
        .iter()
        .map(|(id, title, subtitle)| SearchResult {
            id: format!("command:{id}"),
            kind: ResultKind::SystemCommand,
            title: (*title).into(),
            subtitle: (*subtitle).into(),
            path: None,
            score: 0,
            icon: None,
            primary_action: Action::Run,
            secondary_actions: vec![],
            pin: None,
            confirmation: None,
            detail: None,
        })
        .collect()
}

pub fn panel(id: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|(key, _, _)| id == format!("command:{key}"))
        .map(|(key, _, _)| *key)
}

pub fn open(app: &AppHandle, id: &str) -> Result<(), String> {
    let panel = panel(id).ok_or("Unknown native command.")?;
    super::window::show(app).map_err(|error| error.to_string())?;
    app.emit("open-panel", panel)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn item_catalog(app: AppHandle) -> Result<Vec<SearchResult>, String> {
    let mut items = app
        .state::<LauncherState>()
        .search
        .lock()
        .map_err(|_| "Search is unavailable.".to_string())?
        .item_catalog();
    items.extend(
        app.state::<super::library::LibraryState>()
            .search_items("")
            .unwrap_or_default()
            .into_iter()
            .map(library_result),
    );
    Ok(items)
}

pub fn library_result(item: super::library::LibraryItem) -> SearchResult {
    SearchResult {
        id: item.id,
        kind: ResultKind::SystemCommand,
        title: item.name,
        subtitle: match item.kind {
            super::library::LibraryKind::Quicklink => "Quicklink",
            super::library::LibraryKind::Snippet => "Snippet",
        }
        .into(),
        path: None,
        score: crate::ranking::STRONG_MATCH,
        icon: None,
        primary_action: Action::Run,
        secondary_actions: vec![],
        pin: None,
        confirmation: None,
        detail: None,
    }
}

/// Use the same action validation for global shortcuts as for launcher results.
pub async fn activate_shortcut(app: AppHandle, id: String) -> Result<(), String> {
    let result = item_catalog(app.clone())?
        .into_iter()
        .find(|result| result.id == id)
        .ok_or("The shortcut's item is no longer available.")?;
    if result.confirmation.is_some() {
        super::window::show(&app).map_err(|error| error.to_string())?;
        app.emit("confirm-item", result)
            .map_err(|error| error.to_string())
    } else {
        super::actions::execute_action(id, result.primary_action, None, app).await
    }
}
