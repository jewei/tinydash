use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Every failure that can cross the IPC boundary. The frontend receives the
/// `Display` text, so messages are written for the user.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("Could not read or write a file: {0}. Check that the disk is not full.")]
    Io(#[from] std::io::Error),
    #[error("Could not read or save data: {0}. Check that the disk is not full.")]
    Database(#[from] rusqlite::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Could not use the clipboard: {0}. Try again.")]
    Clipboard(#[from] arboard::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
