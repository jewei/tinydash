//! Built-in utilities. Integration and lifecycle requirements: docs/reference/features/utilities.md.
#[path = "utilities/awake.rs"]
mod awake;
#[path = "utilities/color.rs"]
mod color;
#[path = "utilities/desktop.rs"]
mod desktop;
#[path = "utilities/process.rs"]
mod process;

use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub type UtilityResult<T> = Result<T, String>;

#[derive(Default)]
pub struct UtilitiesState {
    confirmation: Mutex<Option<PendingProcess>>,
    awake: Mutex<Option<awake::Session>>,
    window: Mutex<Option<desktop::WindowTarget>>,
}

struct PendingProcess {
    token: String,
    process: process::ProcessInfo,
    force: bool,
    expires: Instant,
}

impl UtilitiesState {
    /// Call on RunEvent::Exit, before exiting. Drop is a fallback, not the exit hook.
    pub fn shutdown(&self) {
        if let Ok(mut session) = self.awake.lock() {
            session.take();
        }
        if let Ok(mut confirmation) = self.confirmation.lock() {
            confirmation.take();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    processes: String,
    eyedropper: String,
    native_eyedropper: bool,
    awake: String,
    media: String,
    windows: String,
}

#[tauri::command]
pub fn utility_capabilities() -> Capabilities {
    desktop::capabilities()
}

// All external work is off the WebView/main thread. No worker is started by search.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> UtilityResult<T> + Send + 'static,
) -> UtilityResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn utility_processes() -> UtilityResult<Vec<process::ProcessInfo>> {
    blocking(process::list).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessConfirmation {
    token: String,
    process: process::ProcessInfo,
    force: bool,
}

#[tauri::command]
pub async fn utility_prepare_process(
    state: tauri::State<'_, UtilitiesState>,
    process: process::ProcessInfo,
    force: bool,
) -> UtilityResult<ProcessConfirmation> {
    let current = blocking(move || process::validate(&process)).await?;
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    *state
        .confirmation
        .lock()
        .map_err(|_| "Utilities state unavailable")? = Some(PendingProcess {
        token: token.clone(),
        process: current.clone(),
        force,
        expires: Instant::now() + Duration::from_secs(30),
    });
    Ok(ProcessConfirmation {
        token,
        process: current,
        force,
    })
}

fn take_confirmation(
    pending: &mut Option<PendingProcess>,
    token: &str,
    confirmed: bool,
) -> UtilityResult<PendingProcess> {
    // Consume even invalid/replayed attempts. Only one outstanding confirmation is retained.
    let request = pending
        .take()
        .ok_or("Confirmation missing or already used")?;
    if !confirmed || request.token != token || Instant::now() >= request.expires {
        return Err("Confirmation invalid or expired; select the process again".into());
    }
    Ok(request)
}

#[tauri::command]
pub async fn utility_confirm_process(
    state: tauri::State<'_, UtilitiesState>,
    token: String,
    confirmed: bool,
) -> UtilityResult<()> {
    let request = take_confirmation(
        &mut *state
            .confirmation
            .lock()
            .map_err(|_| "Utilities state unavailable")?,
        &token,
        confirmed,
    )?;
    blocking(move || process::terminate(&request.process, request.force)).await
}

#[tauri::command]
pub fn utility_cancel_process(state: tauri::State<'_, UtilitiesState>) {
    if let Ok(mut pending) = state.confirmation.lock() {
        pending.take();
    }
}

#[tauri::command]
pub fn utility_color(input: String) -> UtilityResult<color::Color> {
    color::parse(&input)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorFormat {
    Hex,
    Rgb,
    Hsl,
}

#[tauri::command]
pub async fn utility_copy_color(input: String, format: ColorFormat) -> UtilityResult<()> {
    blocking(move || {
        effects_allowed()?;
        let value = color::parse(&input)?;
        let text = match format {
            ColorFormat::Hex => value.hex,
            ColorFormat::Rgb => value.rgb,
            ColorFormat::Hsl => value.hsl,
        };
        crate::launcher::clipboard::write_secret(&text).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn utility_eyedropper(app: tauri::AppHandle) -> UtilityResult<Option<color::Color>> {
    effects_allowed()?;
    #[cfg(target_os = "macos")]
    {
        blocking(move || desktop::macos::eyedropper(app)).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(
            "Native sampling is unavailable on this platform; use the WebView picker if offered"
                .into(),
        )
    }
}

#[tauri::command]
pub fn utility_awake_status(
    state: tauri::State<'_, UtilitiesState>,
) -> UtilityResult<awake::Status> {
    let mut session = state
        .awake
        .lock()
        .map_err(|_| "Utilities state unavailable")?;
    if session.as_ref().is_some_and(|s| s.expired()) {
        session.take();
    }
    Ok(session
        .as_ref()
        .map_or_else(awake::Status::default, |s| s.status()))
}

#[tauri::command]
pub async fn utility_set_awake(
    state: tauri::State<'_, UtilitiesState>,
    minutes: u32,
) -> UtilityResult<awake::Status> {
    if minutes > 480 {
        return Err("Choose 1–480 minutes, or 0 to stop".into());
    }
    // Session startup/teardown is short and bounded; the dedicated owner thread holds the OS assertion.
    let mut session = state
        .awake
        .lock()
        .map_err(|_| "Utilities state unavailable")?;
    session.take();
    if minutes != 0 {
        *session = Some(awake::Session::start(minutes)?);
    }
    Ok(session
        .as_ref()
        .map_or_else(awake::Status::default, |s| s.status()))
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Mute,
}

#[tauri::command]
pub async fn utility_media(action: MediaAction) -> UtilityResult<()> {
    blocking(move || desktop::media(action)).await
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowAction {
    Left,
    Right,
    Maximize,
    Center,
    Restore,
}

#[tauri::command]
pub async fn utility_capture_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, UtilitiesState>,
    delayed: bool,
) -> UtilityResult<String> {
    // Resolve the parent's pre-activation target on demand, never the focused launcher.
    let saved = if delayed {
        None
    } else {
        Some(crate::launcher::paste::previous_target(&app)?)
    };
    let target = blocking(move || {
        if let Some(id) = saved {
            return desktop::capture_target(id);
        }
        std::thread::sleep(Duration::from_secs(3));
        desktop::capture_window()
    })
    .await?;
    let label = target.label();
    *state
        .window
        .lock()
        .map_err(|_| "Utilities state unavailable")? = Some(target);
    Ok(label)
}

#[tauri::command]
pub async fn utility_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, UtilitiesState>,
    action: WindowAction,
) -> UtilityResult<()> {
    let target = state
        .window
        .lock()
        .map_err(|_| "Utilities state unavailable")?
        .clone()
        .ok_or("Capture a target window first")?;
    #[cfg(target_os = "macos")]
    return blocking(move || desktop::window(&app, &target, action)).await;
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        blocking(move || desktop::window(&target, action)).await
    }
}

/// Bounded subprocess runner: no shell, no unbounded pipes, timeout kills and reaps the child.
/// Only fixed programs/scripts and validated numeric values are supplied by callers.
fn run(program: &str, args: &[&str]) -> UtilityResult<String> {
    use std::{
        io::{Read, Seek},
        process::{Command, Stdio},
    };
    effects_allowed()?;
    let mut output = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut error = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut command = Command::new(program);
    command
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|e| e.to_string())?)
        .stderr(error.try_clone().map_err(|e| e.to_string())?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|e| format!("{program}: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if Instant::now() >= deadline
            || output
                .metadata()
                .map(|m| m.len() > 2_000_000)
                .unwrap_or(true)
            || error.metadata().map(|m| m.len() > 64_000).unwrap_or(true)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program}: timed out or exceeded output limit"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut text = String::new();
    let file = if status.success() {
        &mut output
    } else {
        &mut error
    };
    file.rewind().map_err(|e| e.to_string())?;
    file.take(2_000_000)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!(
            "{program}: {}",
            text.trim().chars().take(600).collect::<String>()
        ));
    }
    Ok(text)
}

fn effects_allowed() -> UtilityResult<()> {
    if cfg!(test) || std::env::var_os("TINYDASH_DISABLE_UTILITY_EFFECTS").is_some() {
        Err("Native utility effects are disabled in this test session".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending() -> Option<PendingProcess> {
        Some(PendingProcess {
            token: "ticket".into(),
            process: process::ProcessInfo {
                pid: 999,
                identity: "identity".into(),
                name: "fixture".into(),
            },
            force: true,
            expires: Instant::now() + Duration::from_secs(30),
        })
    }
    #[test]
    fn confirmation_is_explicit_single_use_and_expires() {
        let mut p = pending();
        assert!(take_confirmation(&mut p, "ticket", false).is_err());
        assert!(p.is_none());
        let mut p = pending();
        assert!(take_confirmation(&mut p, "wrong", true).is_err());
        let mut p = pending();
        p.as_mut().unwrap().expires = Instant::now();
        assert!(take_confirmation(&mut p, "ticket", true).is_err());
        let mut p = pending();
        assert!(take_confirmation(&mut p, "ticket", true).is_ok());
        assert!(take_confirmation(&mut p, "ticket", true).is_err());
    }
    #[test]
    fn desktop_effects_are_blocked_in_tests() {
        assert!(effects_allowed().is_err());
    }
}
