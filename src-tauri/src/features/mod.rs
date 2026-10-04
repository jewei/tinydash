//! One module per launcher feature. Feature modules are pure: they rank,
//! format, and describe actions, but never touch Tauri or the OS directly.

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
