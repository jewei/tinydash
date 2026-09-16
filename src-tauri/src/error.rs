#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The application index is unavailable. Restart TinyDash.")]
    IndexUnavailable,
    #[error("This application is no longer in the index. Refresh the application list.")]
    AppNotFound,
    #[error("Search queries must contain at most 256 characters.")]
    QueryTooLong,
    #[error("Could not open the application: {0}")]
    Launch(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
