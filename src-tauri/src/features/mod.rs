//! One module per launcher feature. Feature modules rank, format, and
//! describe actions, but never call Tauri, SQLite, the clipboard, or the OS.
//! Two indexers are the exceptions, and `refresh.rs` alone runs them:
//! `FileIndex::scan` reads folders and `currency::fetch` downloads rates.

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
