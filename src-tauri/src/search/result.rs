use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::features::system::SystemCommand;

/// One row in the launcher. Results carry their own actions, so the frontend
/// renders them without knowing how each kind works.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResult {
    /// Stable key for pins and usage, such as `app:/Applications/Safari.app`.
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    pub icon: Icon,
    /// The first action runs on Enter, the second on Mod+Enter.
    pub actions: Vec<ResultAction>,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ResultKind {
    App,
    File,
    Folder,
    Clipboard,
    Snippet,
    Quicklink,
    Emoji,
    Calculation,
    DateTime,
    Password,
    Url,
    WebSearch,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Icon {
    /// The system icon for a file or app, served by the `icon:` protocol.
    File {
        path: String,
    },
    Emoji {
        glyph: String,
    },
    Symbol {
        name: Symbol,
    },
}

/// Built-in glyphs that the frontend draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Symbol {
    App,
    File,
    Folder,
    Text,
    Image,
    Files,
    Snippet,
    Link,
    Calculator,
    Clock,
    Key,
    Globe,
    Lock,
    Moon,
    Restart,
    Power,
    LogOut,
    Trash,
    Settings,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResultAction {
    pub label: String,
    pub action: Action,
    /// When set, the launcher asks for confirmation with this message first.
    pub confirm: Option<String>,
}

impl ResultAction {
    pub fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            action,
            confirm: None,
        }
    }
}

/// A result with its ranking score, before results from all sources merge.
pub struct Scored {
    pub score: u32,
    pub result: SearchResult,
}

/// What running a result does. Searches produce these; the frontend sends
/// the chosen one back, and `actions::run` checks it again before it runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Action {
    /// Start an indexed application.
    Launch {
        path: String,
    },
    /// Open an indexed file with its default app.
    Open {
        path: String,
    },
    /// Show a file or app in the file manager.
    Reveal {
        path: String,
    },
    /// Open an http(s) URL in the browser.
    OpenUrl {
        url: String,
    },
    Copy {
        text: String,
    },
    /// Copy text marked secret, so clipboard managers skip it.
    CopySecret {
        text: String,
    },
    /// Copy a clipboard history entry in its original format.
    CopyClip {
        id: i64,
    },
    DeleteClip {
        id: i64,
    },
    /// Delete every unpinned clipboard history entry.
    ClearClipboard,
    /// Copy a snippet with its placeholders filled.
    CopySnippet {
        id: i64,
    },
    OpenQuicklink {
        id: i64,
        query: String,
    },
    System {
        command: SystemCommand,
    },
    Pin {
        id: String,
    },
    Unpin {
        id: String,
    },
    /// Rescan apps and files, and download exchange rates now when they are on.
    Refresh,
    OpenSettings,
    /// Forget where the launcher was dragged, and center it.
    CenterLauncher,
    Quit,
}
