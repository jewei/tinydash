#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::{drag, share};
#[cfg(target_os = "macos")]
pub use macos::{drag, share};
#[cfg(target_os = "windows")]
pub use windows::{drag, share, share_focus_changed};
