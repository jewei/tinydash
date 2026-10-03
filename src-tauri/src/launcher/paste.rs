//! Explicit cross-application paste. Never send keys to an unverified foreground target.
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct PasteState(Mutex<Option<SavedTarget>>, tauri::async_runtime::Mutex<()>);

#[derive(Clone)]
pub(super) struct SavedTarget {
    id: u64,
    #[cfg(target_os = "macos")]
    application: objc2::rc::Retained<objc2_app_kit::NSRunningApplication>,
    #[cfg(not(target_os = "macos"))]
    identity: (u32, u64),
}

pub fn remember(app: &AppHandle) {
    let active = native::foreground().ok().flatten();
    if active.is_some_and(native::is_self) {
        return;
    }
    if let Some(state) = app.try_state::<PasteState>()
        && let Ok(mut saved) = state.0.lock()
    {
        // Never retain a target from an older activation when capture fails.
        *saved = None;
        let Some(target) = active else {
            return;
        };
        #[cfg(not(target_os = "macos"))]
        let Ok(identity) = native::identity(target) else {
            return;
        };
        #[cfg(target_os = "macos")]
        let Some(application) =
            objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(
                target as i32,
            )
        else {
            return;
        };
        *saved = Some(SavedTarget {
            id: target,
            #[cfg(target_os = "macos")]
            application,
            #[cfg(not(target_os = "macos"))]
            identity,
        });
    }
}

pub(super) fn prepare(app: &AppHandle) -> Result<SavedTarget, String> {
    native::available()?;
    saved_target(app)
}

pub fn previous_target(app: &AppHandle) -> Result<u64, String> {
    Ok(saved_target(app)?.id)
}

/// Capture a foreground identity without changing the saved paste target.
pub(super) fn foreground_target() -> Result<u64, String> {
    native::foreground()?
        .filter(|id| !native::is_self(*id))
        .ok_or_else(|| "Focus an application other than TinyDash first.".into())
}

fn saved_target(app: &AppHandle) -> Result<SavedTarget, String> {
    let state = app.state::<PasteState>();
    let target = state.0.lock().map_err(|_| "Paste target is unavailable.")?.clone()
        .ok_or("No previous application was captured. Open TinyDash from the app you want to paste into, or use Copy.")?;
    validate_target(&target)?;
    Ok(target)
}

fn validate_target(target: &SavedTarget) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    if native::identity(target.id)? != target.identity {
        return Err("The previous window belongs to a different process. Reopen TinyDash from the intended app.".into());
    }
    #[cfg(target_os = "macos")]
    if target.application.isTerminated()
        || objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(
            target.id as i32,
        )
        .as_ref()
            != Some(&target.application)
    {
        return Err("The previous app has closed. Reopen TinyDash from the intended app.".into());
    }
    Ok(())
}

pub(super) async fn paste_text(
    app: &AppHandle,
    text: String,
    saved: SavedTarget,
) -> Result<(), String> {
    let state = app.state::<PasteState>();
    let _operation = state
        .1
        .try_lock()
        .map_err(|_| "A paste is already in progress. Try again.")?;
    native::available()?;
    validate_target(&saved)?;
    if text.len() > 65_536 {
        return Err("Paste text exceeds 64 KiB.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        super::clipboard::write_secret(&text).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    paste_current(app, saved).await
}

async fn paste_current(app: &AppHandle, saved: SavedTarget) -> Result<(), String> {
    native::available()?;
    let previous = saved.id;
    // A click elsewhere must not send clipboard contents into an unrelated app.
    let active = native::foreground()?;
    if active.is_some_and(|active| active != previous && !native::is_self(active)) {
        return Err(
            "Focus changed. Nothing was pasted; use Copy or reopen TinyDash from the intended app."
                .into(),
        );
    }
    super::window::dismiss(app).map_err(|error| error.to_string())?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        validate_target(&saved)?;
        if native::foreground()?
            .is_some_and(|active| active != previous && !native::is_self(active))
        {
            return Err("Focus changed; paste canceled. The content is copied.".into());
        }
        native::activate(previous)?;
        let started = std::time::Instant::now();
        loop {
            if native::foreground()? == Some(previous) {
                break;
            }
            if started.elapsed() >= std::time::Duration::from_millis(750) {
                return Err(
                    "Could not restore the previous app. The content is copied; paste it manually."
                        .into(),
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        #[cfg(target_os = "windows")]
        native::wait_for_modifiers()?;
        validate_target(&saved)?;
        native::send(previous)?;
        // Keep another direct paste from replacing the clipboard before the
        // target handles its key event. Dispatch is not receipt confirmation.
        std::thread::sleep(std::time::Duration::from_millis(250));
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?;
    if result.is_err() {
        // Surface failure instead of losing the error in a hidden launcher.
        let _ = super::window::show(app);
    }
    result
}

#[tauri::command]
pub async fn paste_result(app: AppHandle, id: String) -> Result<(), String> {
    let target = prepare(&app)?;
    let text = {
        let state = app.state::<super::LauncherState>();
        let search = state.search.lock().map_err(|_| "Search is unavailable.")?;
        match search
            .resolve_action(&id, super::result::Action::Copy)
            .map_err(|error| error.to_string())?
        {
            super::actions::ResolvedAction::Copy(text) => text,
            _ => return Err("This result cannot be pasted.".into()),
        }
    };
    paste_text(&app, text, target).await
}

#[cfg(target_os = "macos")]
mod native {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
    use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventTapLocation};
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    pub fn available() -> Result<(), String> {
        if unsafe { AXIsProcessTrusted() } {
            Ok(())
        } else {
            Err("Direct paste needs Accessibility access in System Settings → Privacy & Security → Accessibility. You can still use Copy.".into())
        }
    }
    pub fn foreground() -> Result<Option<u64>, String> {
        Ok(NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier() as u64))
    }
    pub fn is_self(pid: u64) -> bool {
        pid == u64::from(std::process::id())
    }
    pub fn activate(pid: u64) -> Result<(), String> {
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
            .ok_or("The previous app has closed.")?;
        if app.isTerminated() {
            return Err("The previous app has closed.".into());
        }
        if foreground()?.is_some_and(|active| active != pid && !is_self(active)) {
            return Err("Focus changed; paste canceled. The content is copied.".into());
        }
        if foreground()? != Some(pid)
            && !app.activateWithOptions(NSApplicationActivationOptions::empty())
        {
            return Err("Could not activate the previous app. Use Copy instead.".into());
        }
        Ok(())
    }
    pub fn send(pid: u64) -> Result<(), String> {
        available()?;
        if foreground()? != Some(pid) {
            return Err("Focus changed; paste canceled.".into());
        }
        let down =
            CGEvent::new_keyboard_event(None, 9, true).ok_or("Could not create paste event.")?;
        let up =
            CGEvent::new_keyboard_event(None, 9, false).ok_or("Could not create paste event.")?;
        CGEvent::set_flags(Some(&down), CGEventFlags::MaskCommand);
        CGEvent::set_flags(Some(&up), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&down));
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&up));
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod native {
    use windows_sys::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};
    pub fn available() -> Result<(), String> {
        Ok(())
    }
    pub fn foreground() -> Result<Option<u64>, String> {
        let window = unsafe { GetForegroundWindow() };
        Ok((!window.is_null()).then_some(window as usize as u64))
    }
    pub fn identity(window: u64) -> Result<(u32, u64), String> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, FILETIME},
            System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        };
        unsafe {
            let mut pid = 0;
            if GetWindowThreadProcessId(window as usize as _, &mut pid) == 0 || pid == 0 {
                return Err("The previous window has closed.".into());
            }
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err("Cannot verify the previous application's identity. Use Copy.".into());
            }
            let [mut created, mut exited, mut kernel, mut user]: [FILETIME; 4] = std::mem::zeroed();
            let ok = GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user);
            CloseHandle(handle);
            if ok == 0 {
                return Err(
                    "Cannot verify the previous application's start time. Use Copy.".into(),
                );
            }
            Ok((
                pid,
                (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
            ))
        }
    }
    pub fn is_self(window: u64) -> bool {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(window as usize as _, &mut pid);
        }
        pid == std::process::id()
    }
    pub fn activate(window: u64) -> Result<(), String> {
        if unsafe { IsWindow(window as usize as _) } == 0 {
            return Err("The previous window has closed.".into());
        }
        if unsafe { SetForegroundWindow(window as usize as _) } == 0 {
            return Err(
                "Windows prevented focus restoration. The content is copied; paste manually."
                    .into(),
            );
        }
        Ok(())
    }
    pub fn wait_for_modifiers() -> Result<(), String> {
        // An item hotkey can still be held when insertion reaches this worker.
        // Windows combines synthetic input with the physical modifier state.
        let started = std::time::Instant::now();
        while [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|key| unsafe { GetAsyncKeyState(i32::from(*key)) } < 0)
        {
            if started.elapsed() >= std::time::Duration::from_millis(750) {
                return Err(
                    "Release the shortcut keys, then try again. The content is copied.".into(),
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        Ok(())
    }
    pub fn send(window: u64) -> Result<(), String> {
        if foreground()? != Some(window) {
            return Err("Focus changed; paste canceled.".into());
        }
        let key = |vk, flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let events = [
            key(VK_CONTROL, 0),
            key(0x56, 0),
            key(0x56, KEYEVENTF_KEYUP),
            key(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        if unsafe {
            SendInput(
                events.len() as u32,
                events.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        } != events.len() as u32
        {
            return Err("Windows blocked paste, possibly into an elevated app. The content is copied; paste manually.".into());
        }
        Ok(())
    }
}

#[cfg(any(target_os = "linux", test))]
fn linux_start_time(stat: &str) -> Option<u64> {
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn process_start_identity_handles_spaces_and_parentheses() {
        let rest = std::iter::repeat_n("0", 19)
            .chain(["4242", "0"])
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            super::linux_start_time(&format!("123 (a ) process) {rest}")),
            Some(4242)
        );
        assert_eq!(super::linux_start_time("123 (bad) 0"), None);
        assert_eq!(super::linux_start_time("malformed"), None);
    }
}

#[cfg(target_os = "linux")]
mod native {
    fn run(args: &[&str]) -> Result<String, String> {
        crate::platform::system_process::output("xdotool", args).map_err(|_| "Direct paste on X11 requires xdotool and a working desktop session. Use Copy instead.".into())
    }
    pub fn available() -> Result<(), String> {
        if crate::platform::is_wayland() {
            Err(
                "Wayland does not permit portable cross-app paste. Use Copy and paste manually."
                    .into(),
            )
        } else {
            run(&["version"]).map(|_| ())
        }
    }
    pub fn foreground() -> Result<Option<u64>, String> {
        if crate::platform::is_wayland() {
            return Ok(None);
        }
        Ok(run(&["getactivewindow"])?.parse().ok())
    }
    pub fn identity(window: u64) -> Result<(u32, u64), String> {
        let pid = run(&["getwindowpid", &window.to_string()])?
            .parse::<u32>()
            .map_err(|_| "Cannot verify the previous window's process.")?;
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_err(|_| "The previous window's process has closed.")?;
        let start = super::linux_start_time(&stat)
            .ok_or("Cannot verify the previous application's start time.")?;
        Ok((pid, start))
    }
    pub fn is_self(window: u64) -> bool {
        run(&["getwindowpid", &window.to_string()])
            .ok()
            .and_then(|pid| pid.parse::<u32>().ok())
            == Some(std::process::id())
    }
    pub fn activate(window: u64) -> Result<(), String> {
        run(&["windowactivate", &window.to_string()]).map(|_| ())
    }
    pub fn send(window: u64) -> Result<(), String> {
        if foreground()? != Some(window) {
            return Err("Focus changed; paste canceled.".into());
        }
        run(&["key", "--clearmodifiers", "ctrl+v"]).map(|_| ())
    }
}
