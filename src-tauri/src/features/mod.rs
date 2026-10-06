//! One module per launcher feature. Feature modules rank, format, and
//! describe actions. They never call Tauri, SQLite, the clipboard, or
//! windows, and never change anything in the OS; they may read cheap OS
//! state: the home folder path, the clock and time zone, and, for
//! passwords, the random source. Two indexers read more,
//! and `refresh.rs` alone runs them: `FileIndex::scan` reads folders and
//! `currency::fetch` downloads rates.

pub mod apps;
pub mod calculator;
pub mod clipboard;
pub mod currency;
pub mod datetime;
pub mod emoji;
pub mod files;
pub mod library;
pub mod password;
pub mod system;
pub mod url_cleaner;
pub mod web;
