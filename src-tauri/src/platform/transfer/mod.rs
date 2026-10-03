#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::drag;
#[cfg(target_os = "macos")]
pub use macos::drag;
#[cfg(target_os = "windows")]
pub use windows::drag;
