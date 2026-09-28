use serde::Serialize;

/// Stable categories for warnings the launcher must act on. Command failures
/// that are only displayed remain strings; do not classify their English text.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WarningCode {
    SettingsRead,
    ShortcutRegistration,
    ShortcutsUnavailable,
    ClipboardLimited,
    TrayUnavailable,
    StorageUnavailable,
    ClipboardUnavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherWarning {
    pub code: WarningCode,
    pub message: String,
    /// Whether repeating the originating operation can recover without restart
    /// or configuration repair. This is metadata, not an instruction to retry.
    pub retryable: bool,
}

impl LauncherWarning {
    pub fn new(code: WarningCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}
