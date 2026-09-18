#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The search index is unavailable. Restart TinyDash.")]
    IndexUnavailable,
    #[error("This application is no longer in the index. Refresh the application list.")]
    AppNotFound,
    #[error("This file is no longer available. Refresh the file list.")]
    FileNotFound,
    #[error("This action is not available for the selected result.")]
    InvalidAction,
    #[error("Confirm this system command before running it.")]
    ConfirmationRequired,
    #[error("Could not run the system command: {0}")]
    SystemCommand(String),
    #[error("This result has expired. Search again to copy it.")]
    ResultExpired,
    #[error("Use at most 256 characters for a search, or 8,192 for a URL.")]
    QueryTooLong,
    #[error("Could not open the application: {0}")]
    Launch(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
