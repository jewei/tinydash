use super::{UtilityResult, effects_allowed};
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    active: bool,
    ends_at: Option<u64>,
    remaining_seconds: u64,
}

pub struct Session {
    stop: mpsc::Sender<()>,
    worker: Option<JoinHandle<()>>,
    until: Instant,
    ends_at: u64,
    active: Arc<AtomicBool>,
}
impl Session {
    pub fn start(minutes: u32) -> UtilityResult<Self> {
        effects_allowed()?;
        if !(1..=480).contains(&minutes) {
            return Err("Keep awake must last 1–480 minutes".into());
        }
        let duration = Duration::from_secs(u64::from(minutes) * 60);
        let (stop, stopped) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(false));
        let worker_active = active.clone();
        let worker = std::thread::Builder::new()
            .name("utility-awake".into())
            .spawn(move || match Assertion::acquire() {
                Ok(assertion) => {
                    worker_active.store(true, Ordering::Release);
                    if ready.send(Ok(())).is_ok() {
                        let _ = stopped.recv_timeout(duration);
                    }
                    drop(assertion);
                    worker_active.store(false, Ordering::Release);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
            .map_err(|e| e.to_string())?;
        match started.recv_timeout(Duration::from_secs(7)) {
            Ok(Ok(())) => Ok(Self {
                stop,
                worker: Some(worker),
                until: Instant::now() + duration,
                ends_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
                    + duration.as_millis() as u64,
                active,
            }),
            result => {
                drop(stop);
                let _ = worker.join();
                Err(result
                    .ok()
                    .and_then(Result::err)
                    .unwrap_or("Keep-awake service did not respond".into()))
            }
        }
    }
    pub fn expired(&self) -> bool {
        !self.active.load(Ordering::Acquire) || Instant::now() >= self.until
    }
    pub fn status(&self) -> Status {
        Status {
            active: !self.expired(),
            ends_at: Some(self.ends_at),
            remaining_seconds: self
                .until
                .saturating_duration_since(Instant::now())
                .as_secs(),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(target_os = "macos")]
struct Assertion(u32);
#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: *const std::ffi::c_void,
        level: u32,
        name: *const std::ffi::c_void,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}
#[cfg(target_os = "macos")]
impl Assertion {
    fn acquire() -> UtilityResult<Self> {
        use super::desktop::macos::Cf;
        let kind = Cf::string("PreventUserIdleSystemSleep")?;
        let name = Cf::string("TinyDash timed keep awake")?;
        let mut id = 0;
        let result = unsafe { IOPMAssertionCreateWithName(kind.0, 255, name.0, &mut id) };
        if result != 0 {
            Err(format!("Could not inhibit idle sleep ({result})"))
        } else {
            Ok(Self(id))
        }
    }
}
#[cfg(target_os = "macos")]
impl Drop for Assertion {
    fn drop(&mut self) {
        unsafe {
            IOPMAssertionRelease(self.0);
        }
    }
}

#[cfg(target_os = "windows")]
struct Assertion;
#[cfg(target_os = "windows")]
impl Assertion {
    fn acquire() -> UtilityResult<Self> {
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        if unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } == 0 {
            Err("Windows refused the idle-sleep assertion".into())
        } else {
            Ok(Self)
        }
    }
}
#[cfg(target_os = "windows")]
impl Drop for Assertion {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Power::SetThreadExecutionState(
                windows_sys::Win32::System::Power::ES_CONTINUOUS,
            );
        }
    }
}

#[cfg(target_os = "linux")]
struct Assertion(std::os::fd::OwnedFd);
#[cfg(target_os = "linux")]
impl Assertion {
    fn acquire() -> UtilityResult<Self> {
        use gio::{
            glib::variant::{Handle, ToVariant},
            prelude::*,
        };
        let bus = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE)
            .map_err(|e| e.to_string())?;
        let (reply, fds) = bus
            .call_with_unix_fd_list_sync(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                "Inhibit",
                Some(&("sleep:idle", "TinyDash", "Timed keep awake", "block").to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                5000,
                None::<&gio::UnixFDList>,
                gio::Cancellable::NONE,
            )
            .map_err(|e| format!("logind sleep inhibitor: {e}"))?;
        let handle = reply
            .child_value(0)
            .get::<Handle>()
            .ok_or("Missing inhibitor handle")?;
        let fd = fds
            .ok_or("Missing inhibitor file descriptor")?
            .get(handle.0)
            .map_err(|e| e.to_string())?;
        Ok(Self(fd))
    }
}
#[cfg(target_os = "linux")]
impl Drop for Assertion {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let _ = self.0.as_raw_fd(); /* OwnedFd closes on drop, including process exit. */
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tests_never_acquire_assertions() {
        assert!(Session::start(1).is_err());
        assert!(Session::start(481).is_err());
    }
}
