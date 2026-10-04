use super::{UtilityResult, effects_allowed, run};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProcessInfo {
    pub pid: u32,
    pub identity: String,
    pub name: String,
}

fn safe(p: &ProcessInfo) -> bool {
    p.pid > 1
        && p.pid != std::process::id()
        && !p.name.to_ascii_lowercase().contains("tinydash")
        && !p.identity.is_empty()
        && p.identity.len() <= 1024
        && p.name.len() <= 4096
}

#[cfg(unix)]
unsafe extern "C" {
    fn getuid() -> u32;
    #[cfg(target_os = "macos")]
    fn kill(pid: i32, signal: i32) -> i32;
}

#[cfg(unix)]
fn parse_ps(text: &str, uid: u32) -> Vec<ProcessInfo> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let owner: u32 = parts.next()?.parse().ok()?;
            // Never offer root/system-owned processes, even when launched as root.
            if owner == 0 || owner != uid {
                return None;
            }
            let start = (0..5)
                .map(|_| parts.next())
                .collect::<Option<Vec<_>>>()?
                .join(" ");
            let name = parts.collect::<Vec<_>>().join(" ");
            let p = ProcessInfo {
                pid,
                identity: format!("{owner}:{start}"),
                name,
            };
            safe(&p).then_some(p)
        })
        .take(4096)
        .collect()
}

#[cfg(unix)]
pub fn list() -> UtilityResult<Vec<ProcessInfo>> {
    let text = run("/bin/ps", &["-axo", "pid=,uid=,lstart=,comm="])?;
    let mut rows = parse_ps(&text, unsafe { getuid() });
    rows.retain_mut(|row| {
        // Native start times remove ps's one-second timestamp ambiguity.
        match native_start(row.pid) {
            Ok(start) => {
                row.identity.push(':');
                row.identity.push_str(&start);
                true
            }
            Err(_) => false,
        }
    });
    rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.pid.cmp(&b.pid)));
    Ok(rows)
}

#[cfg(target_os = "linux")]
fn native_start(pid: u32) -> UtilityResult<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|e| e.to_string())?;
    stat.rsplit_once(") ")
        .and_then(|(_, rest)| rest.split_whitespace().nth(19))
        .map(str::to_owned)
        .ok_or("Process identity unavailable".into())
}

#[cfg(target_os = "macos")]
fn native_start(pid: u32) -> UtilityResult<String> {
    #[repr(C)]
    struct BsdInfo {
        prefix: [u32; 12],
        comm: [u8; 16],
        name: [u8; 32],
        fields: [u32; 6],
        seconds: u64,
        micros: u64,
    }
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut std::ffi::c_void,
            size: i32,
        ) -> i32;
    }
    let mut info = std::mem::MaybeUninit::<BsdInfo>::uninit();
    let size = std::mem::size_of::<BsdInfo>() as i32;
    if unsafe { proc_pidinfo(pid as i32, 3, 0, info.as_mut_ptr().cast(), size) } != size {
        return Err("Process start time unavailable".into());
    }
    let info = unsafe { info.assume_init() };
    if info.prefix[5] != unsafe { getuid() } || info.prefix[5] == 0 {
        return Err("Process owner changed".into());
    }
    Ok(format!("{}:{}", info.seconds, info.micros))
}

#[cfg(target_os = "windows")]
pub fn list() -> UtilityResult<Vec<ProcessInfo>> {
    // Same-session, non-elevated processes only. Access-denied entries are not actionable.
    let text = run(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"$ErrorActionPreference='Stop'; $me=Get-Process -Id $PID; $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $rows=@(Get-CimInstance Win32_Process | ForEach-Object { $p=$_; try { if ($p.SessionId -eq $me.SessionId -and $p.ProcessId -gt 1) { $owner=Invoke-CimMethod -InputObject $p -MethodName GetOwnerSid; if ($owner.Sid -eq $sid -and $sid -ne 'S-1-5-18') { $q=Get-Process -Id $p.ProcessId -ErrorAction Stop; @{pid=[uint32]$q.Id; identity=$q.StartTime.ToUniversalTime().Ticks.ToString(); name=$q.ProcessName} } } } catch {} }); ConvertTo-Json -Compress -InputObject $rows"#,
        ],
    )?;
    let mut rows: Vec<ProcessInfo> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    rows.retain(safe);
    rows.truncate(4096);
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

pub fn validate(expected: &ProcessInfo) -> UtilityResult<ProcessInfo> {
    if !safe(expected) {
        return Err("TinyDash and system processes are protected".into());
    }
    list()?
        .into_iter()
        .find(|p| p == expected)
        .ok_or("Process exited or its identity changed; refresh the list".into())
}

pub fn terminate(expected: &ProcessInfo, force: bool) -> UtilityResult<()> {
    effects_allowed()?;
    let current = validate(expected)?;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        unsafe extern "C" {
            fn syscall(number: std::ffi::c_long, ...) -> std::ffi::c_long;
        }
        // pidfd syscalls use the same numbers on supported Linux x86_64/aarch64 targets.
        // Refuse old kernels rather than fall back to a racy PID-only signal.
        let raw = unsafe { syscall(434, current.pid, 0u32) };
        if raw < 0 {
            return Err(format!(
                "Safe process handles require Linux 5.3+: {}",
                std::io::Error::last_os_error()
            ));
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        validate(&current)?;
        if unsafe {
            syscall(
                424,
                fd.as_raw_fd(),
                if force { 9i32 } else { 15i32 },
                std::ptr::null::<std::ffi::c_void>(),
                0u32,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        // Exact owner/start/name revalidation occurs immediately before the signal. Unix signals
        // cannot atomically bind a macOS PID to a start time; a very small PID reuse race remains.
        let pid = i32::try_from(current.pid).map_err(|_| "Invalid process ID")?;
        if unsafe { kill(pid, if force { 9 } else { 15 }) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let ticks: u64 = current
            .identity
            .parse()
            .map_err(|_| "Invalid process identity")?;
        // Holding Process.Handle pins the kernel process object across the final identity check.
        let action = if force {
            "$p.Kill()"
        } else {
            "if (-not $p.CloseMainWindow()) { throw 'No main window accepted a graceful quit; use Force kill only if necessary' }"
        };
        run("powershell.exe", &["-NoProfile","-NonInteractive","-Command", &format!("$ErrorActionPreference='Stop'; $p=Get-Process -Id {}; $handle=$p.Handle; if ($p.StartTime.ToUniversalTime().Ticks -ne {ticks}) {{ throw 'Process identity changed' }}; {action}",current.pid)]).map(|_|())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_system_and_self() {
        for pid in [0, 1, std::process::id()] {
            assert!(!safe(&ProcessInfo {
                pid,
                identity: "start".into(),
                name: "fixture".into()
            }));
        }
        assert!(!safe(&ProcessInfo {
            pid: 999,
            identity: "start".into(),
            name: "/Applications/TinyDash.app/Contents/MacOS/tinydash".into()
        }));
    }
    #[cfg(unix)]
    #[test]
    fn parses_identity_and_filters_owners() {
        let rows = parse_ps(
            "42 501 Mon Sep 21 10:20:30 2026 /Applications/Some App\n43 0 Mon Sep 21 10:20:30 2026 root\n44 502 Mon Sep 21 10:20:30 2026 other",
            501,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "/Applications/Some App");
        assert_eq!(rows[0].identity, "501:Mon Sep 21 10:20:30 2026");
    }
}
