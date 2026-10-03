//! On-demand app-to-process resolution. Search never enumerates processes.
use super::{UtilityResult, effects_allowed, process};
use crate::providers::apps::AppEntry;
use std::path::PathBuf;

pub struct Target {
    // Bundle path on macOS; executable path on Windows and Linux.
    path: PathBuf,
}

pub fn resolve(entry: &AppEntry) -> UtilityResult<(process::ProcessInfo, Target)> {
    effects_allowed()?;
    let target = Target {
        path: target_path(entry)?,
    };
    let pids = matching_pids(&target)?;
    let mut matches = process::list()?
        .into_iter()
        .filter(|p| pids.contains(&p.pid));
    let current = matches.next().ok_or(
        "This app is not running, or its process cannot be accessed. Use Utilities to inspect processes.",
    )?;
    if matches.next().is_some() {
        return Err(
            "More than one process matches this app. Use Utilities to select a process.".into(),
        );
    }
    // Check the match again after obtaining its native process identity.
    if !matching_pids(&target)?.contains(&current.pid) {
        return Err("The app changed while checking it. Select the action again.".into());
    }
    Ok((current, target))
}

pub fn terminate(
    target: &Target,
    expected: &process::ProcessInfo,
    force: bool,
) -> UtilityResult<()> {
    effects_allowed()?;
    if !matching_pids(target)?.contains(&expected.pid) {
        return Err("The app exited or its path changed. Select the action again.".into());
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSRunningApplication;
        let pid = i32::try_from(expected.pid).map_err(|_| "Invalid process ID")?;
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .ok_or("The app has exited")?;
        process::validate(expected)?;
        // AppKit requests normal app termination, including its save dialogs.
        // Keep the native object alive across the final identity check and request.
        let sent = if force {
            app.forceTerminate()
        } else {
            app.terminate()
        };
        if sent {
            Ok(())
        } else {
            Err("The app did not accept the quit request.".into())
        }
    }
    #[cfg(not(target_os = "macos"))]
    process::terminate(expected, force)
}

#[cfg(target_os = "macos")]
fn target_path(entry: &AppEntry) -> UtilityResult<PathBuf> {
    if !entry
        .path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    {
        return Err("This result is not an application bundle.".into());
    }
    entry.path.canonicalize().map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn matching_pids(target: &Target) -> UtilityResult<Vec<u32>> {
    use objc2_app_kit::NSWorkspace;
    Ok(NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|app| !app.isTerminated())
        .filter_map(|app| {
            let path = PathBuf::from(app.bundleURL()?.path()?.to_string())
                .canonicalize()
                .ok()?;
            (path == target.path)
                .then(|| u32::try_from(app.processIdentifier()).ok())
                .flatten()
        })
        .take(4096)
        .collect())
}

#[cfg(not(target_os = "macos"))]
fn check_executable(path: PathBuf) -> UtilityResult<PathBuf> {
    if !path.is_absolute() || !path.is_file() {
        return Err(
            "The app executable cannot be resolved safely. Use Utilities to select its process."
                .into(),
        );
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // A launcher/runtime can host unrelated apps. Never infer its owner from a name.
    if [
        "env",
        "sh",
        "bash",
        "dash",
        "zsh",
        "fish",
        "cmd",
        "powershell",
        "pwsh",
        "wscript",
        "cscript",
        "rundll32",
        "dllhost",
        "java",
        "javaw",
        "python",
        "python3",
        "ruby",
        "perl",
        "node",
        "bun",
        "electron",
        "flatpak",
        "snap",
        "wine",
        "wine64",
        "gtk-launch",
        "gio",
        "xdg-open",
        "open",
    ]
    .contains(&name.as_str())
    {
        return Err("This app uses a shared launcher. Use Utilities to select its process.".into());
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn target_path(entry: &AppEntry) -> UtilityResult<PathBuf> {
    use gio::prelude::*;
    let app = gio_unix::DesktopAppInfo::from_filename(&entry.path)
        .ok_or("The desktop entry is no longer available")?;
    let command = app
        .commandline()
        .ok_or("The desktop entry has no executable command")?;
    let args = gio::glib::shell_parse_argv(command).map_err(|e| e.to_string())?;
    // Extra arguments can select a different app in a shared executable. Only
    // standard desktop field codes are accepted, and none are executed here.
    if args.iter().skip(1).any(|arg| {
        !matches!(
            arg.to_str(),
            Some("%f" | "%F" | "%u" | "%U" | "%i" | "%c" | "%k")
        )
    }) {
        return Err(
            "This desktop entry has app-specific arguments. Use Utilities to select its process."
                .into(),
        );
    }
    let executable = app.executable();
    let path = if executable.is_absolute() {
        executable
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .filter(|path| path.is_absolute())
            .map(|path| path.join(&executable))
            .find(|path| path.is_file())
            .ok_or("The app executable was not found")?
    };
    check_executable(path)
}

#[cfg(target_os = "linux")]
fn matching_pids(target: &Target) -> UtilityResult<Vec<u32>> {
    let entries = std::fs::read_dir("/proc").map_err(|e| e.to_string())?;
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
            let path = std::fs::read_link(entry.path().join("exe")).ok()?;
            (path == target.path).then_some(pid)
        })
        .take(4096)
        .collect())
}

#[cfg(target_os = "windows")]
fn quote_powershell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn target_path(entry: &AppEntry) -> UtilityResult<PathBuf> {
    let extension = entry
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if extension.eq_ignore_ascii_case("exe") {
        return check_executable(entry.path.clone());
    }
    if !extension.eq_ignore_ascii_case("lnk") {
        return Err(
            "This app uses a deployment launcher. Use Utilities to select its process.".into(),
        );
    }
    let script = format!(
        "$ErrorActionPreference='Stop'; $s=(New-Object -ComObject WScript.Shell).CreateShortcut({}); if ($s.Arguments) {{ throw 'This shortcut has arguments. Use Utilities to select its process.' }}; ConvertTo-Json -Compress -InputObject $s.TargetPath",
        quote_powershell(&entry.path.to_string_lossy())
    );
    let text = super::run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
    let path: String = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let path = PathBuf::from(path);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err(
            "This shortcut does not target an executable. Use Utilities to select its process."
                .into(),
        );
    }
    check_executable(path)
}

#[cfg(target_os = "windows")]
fn matching_pids(target: &Target) -> UtilityResult<Vec<u32>> {
    // Canonical paths have an extended-path prefix; Process.Path normally does not.
    let path = target.path.to_string_lossy();
    let path = path
        .strip_prefix(r"\\?\UNC\")
        .map(|s| format!(r"\\{s}"))
        .unwrap_or_else(|| path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned());
    let script = format!(
        "$ErrorActionPreference='Stop'; $target={}; $rows=@(Get-Process | ForEach-Object {{ try {{ if ($_.MainWindowHandle -ne 0 -and [String]::Equals($_.Path,$target,[StringComparison]::OrdinalIgnoreCase)) {{ [uint32]$_.Id }} }} catch {{}} }}); ConvertTo-Json -Compress -InputObject $rows",
        quote_powershell(&path)
    );
    let text = super::run(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
