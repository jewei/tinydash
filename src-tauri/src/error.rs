use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Every failure that can cross the IPC boundary. The frontend receives the
/// `Display` text, so messages are written for the user.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Storage error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Clipboard error: {0}")]
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
