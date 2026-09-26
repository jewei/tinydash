use super::*;
use std::ffi::c_void;
type Hwnd = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: WinRect,
    work: WinRect,
    flags: u32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> Hwnd;
    fn GetWindowThreadProcessId(window: Hwnd, pid: *mut u32) -> u32;
    fn GetWindowRect(window: Hwnd, rect: *mut WinRect) -> i32;
    fn IsWindow(window: Hwnd) -> i32;
    fn MonitorFromWindow(window: Hwnd, flags: u32) -> *mut c_void;
    fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    fn SetWindowPos(
        window: Hwnd,
        after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn ShowWindow(window: Hwnd, command: i32) -> i32;
}
#[derive(Clone)]
pub struct WindowTarget {
    id: usize,
    identity: super::super::process::ProcessInfo,
    original: Rect,
}
impl WindowTarget {
    pub fn label(&self) -> String {
        format!("{} (PID {})", self.identity.name, self.identity.pid)
    }
}
pub fn capture_window() -> UtilityResult<WindowTarget> {
    effects_allowed()?;
    capture_target(unsafe { GetForegroundWindow() } as u64)
}
pub fn capture_target(id: u64) -> UtilityResult<WindowTarget> {
    effects_allowed()?;
    let hwnd = usize::try_from(id).map_err(|_| "Invalid target window")? as Hwnd;
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    let identity = super::super::process::list()?
        .into_iter()
        .find(|p| p.pid == pid)
        .ok_or("Focus a non-system application other than TinyDash, then capture it")?;
    let mut r = WinRect::default();
    if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
        return Err("Cannot read target window bounds".into());
    }
    Ok(WindowTarget {
        id: hwnd as usize,
        identity,
        original: Rect {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        },
    })
}
pub fn window(target: &WindowTarget, action: WindowAction) -> UtilityResult<()> {
    effects_allowed()?;
    super::super::process::validate(&target.identity)?;
    let hwnd = target.id as Hwnd;
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    if unsafe { IsWindow(hwnd) } == 0 || pid != target.identity.pid {
        return Err("Target window no longer exists or changed identity".into());
    }
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: WinRect::default(),
        work: WinRect::default(),
        flags: 0,
    };
    if unsafe { GetMonitorInfoW(MonitorFromWindow(hwnd, 2), &mut info) } == 0 {
        return Err("Monitor work area unavailable".into());
    }
    let r = info.work;
    let rect = super::placement(
        target.original,
        Rect {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        },
        action,
    );
    unsafe {
        ShowWindow(hwnd, 9);
    }
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            0x0004 | 0x0010,
        )
    } == 0
    {
        return Err(
            "Window refused placement; elevated windows may require matching permissions".into(),
        );
    }
    Ok(())
}
pub fn media(action: MediaAction) -> UtilityResult<()> {
    effects_allowed()?;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, keybd_event};
    let key = match action {
        MediaAction::PlayPause => 0xB3,
        MediaAction::Next => 0xB0,
        MediaAction::Previous => 0xB1,
        MediaAction::VolumeUp => 0xAF,
        MediaAction::VolumeDown => 0xAE,
        MediaAction::Mute => 0xAD,
    };
    unsafe {
        keybd_event(key, 0, 0, 0);
        keybd_event(key, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}
