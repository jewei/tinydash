use rusqlite::ErrorCode;
use serde::{Serialize, Serializer};
use std::io::ErrorKind;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Every failure that can cross the IPC boundary. The frontend receives the
/// `Display` text, so messages are written for the user.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("Could not read or write a file: {0}. {step}", step = file_next_step(.0))]
    Io(#[from] std::io::Error),
    #[error("Could not read or save data: {0}. {step}", step = data_next_step(.0))]
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

/// The step that fits the cause. The files TinyDash reads and writes itself
/// are its settings and its data.
fn file_next_step(error: &std::io::Error) -> &'static str {
    match error.kind() {
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
            "Free some disk space, then try again."
        }
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => {
            "Check that you can write to TinyDash's settings and data folders."
        }
        _ => "Try again, or restart TinyDash.",
    }
}

fn data_next_step(error: &rusqlite::Error) -> &'static str {
    match error.sqlite_error_code() {
        Some(ErrorCode::DiskFull) => "Free some disk space, then try again.",
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
            "The saved data is damaged. Quit TinyDash, then move tinydash.db out of its data folder to start with new data."
        }
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            "Close other programs that use tinydash.db, then try again."
        }
        Some(ErrorCode::ReadOnly | ErrorCode::PermissionDenied | ErrorCode::CannotOpen) => {
            "Check that you can write to TinyDash's data folder."
        }
        _ => "Try again, or restart TinyDash.",
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_error(kind: ErrorKind) -> String {
        Error::from(std::io::Error::from(kind)).to_string()
    }

    fn data_error(code: std::ffi::c_int) -> String {
        Error::from(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(code),
            None,
        ))
        .to_string()
    }

    #[test]
    fn a_file_error_says_to_free_space_only_when_the_disk_is_full() {
        assert!(
            file_error(ErrorKind::StorageFull).ends_with("Free some disk space, then try again.")
        );
        let denied = file_error(ErrorKind::PermissionDenied);
        assert!(
            denied.ends_with("Check that you can write to TinyDash's settings and data folders."),
            "{denied}"
        );
        for kind in [ErrorKind::WouldBlock, ErrorKind::NotFound, ErrorKind::Other] {
            let error = file_error(kind);
            assert!(
                error.ends_with("Try again, or restart TinyDash."),
                "{error}"
            );
        }
    }

    #[test]
    fn a_data_error_gives_the_step_for_its_cause() {
        use rusqlite::ffi;
        let cases = [
            (ffi::SQLITE_FULL, "Free some disk space, then try again."),
            (
                ffi::SQLITE_NOTADB,
                "move tinydash.db out of its data folder",
            ),
            (
                ffi::SQLITE_CORRUPT,
                "move tinydash.db out of its data folder",
            ),
            (
                ffi::SQLITE_BUSY,
                "Close other programs that use tinydash.db",
            ),
            (
                ffi::SQLITE_READONLY,
                "Check that you can write to TinyDash's data folder.",
            ),
            (ffi::SQLITE_IOERR, "Try again, or restart TinyDash."),
        ];
        for (code, step) in cases {
            let error = data_error(code);
            assert!(error.contains(step), "{code}: {error}");
            assert_eq!(
                error.contains("disk space"),
                code == ffi::SQLITE_FULL,
                "{error}"
            );
        }
    }
}
