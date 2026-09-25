use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr};

use windows_sys::{
    Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{
            FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
            SHGetKnownFolderPath,
        },
    },
    core::GUID,
};

use crate::{
    error::{Error, Result},
    providers::apps::AppEntry,
};

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut raw = ptr::null_mut();
    // SAFETY: The API writes a null-terminated UTF-16 allocation into raw. We copy
    // it before releasing it with the allocator required by SHGetKnownFolderPath.
    unsafe {
        let status = SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut raw);
        let result = if status >= 0 && !raw.is_null() {
            let mut length = 0;
            while *raw.add(length) != 0 {
                length += 1;
            }
            Some(PathBuf::from(OsString::from_wide(
                std::slice::from_raw_parts(raw, length),
            )))
        } else {
            tracing::warn!(status, "An application shortcut folder is unavailable");
            None
        };
        if !raw.is_null() {
            CoTaskMemFree(raw.cast());
        }
        result
    }
}

pub fn discover_apps() -> Result<Vec<AppEntry>> {
    let mut apps = Vec::new();
    let mut skipped = 0;
    for root in [
        FOLDERID_Programs,
        FOLDERID_CommonPrograms,
        FOLDERID_Desktop,
        FOLDERID_PublicDesktop,
    ]
    .iter()
    .filter_map(known_folder)
    {
        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(8)
            .into_iter()
            .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let path = entry.path();
            if !entry.file_type().is_file()
                || !is_app_extension(path.extension().and_then(|ext| ext.to_str()))
            {
                continue;
            }
            if let Some(name) = path.file_stem() {
                apps.push(AppEntry::new(
                    name.to_string_lossy().into_owned(),
                    path.to_owned(),
                    vec![],
                ));
            }
        }
    }
    if skipped > 0 {
        tracing::warn!(skipped, "Some application shortcuts could not be read");
    }
    Ok(apps)
}

fn is_app_extension(extension: Option<&str>) -> bool {
    extension.is_some_and(|ext| {
        ["lnk", "exe", "appref-ms"]
            .iter()
            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
    })
}

pub fn launch(entry: &AppEntry) -> Result<()> {
    // The official opener uses Windows shell activation, including .lnk arguments.
    tauri_plugin_opener::open_path(&entry.path, None::<&str>)
        .map_err(|error| Error::Launch(error.to_string()))
}

pub fn system_commands() -> Vec<crate::providers::system::SystemCommand> {
    crate::providers::system::SystemCommand::ALL.to_vec()
}

pub fn run_system_command(command: crate::providers::system::SystemCommand) -> Result<()> {
    use crate::providers::system::SystemCommand;
    use windows_sys::Win32::System::{Power::SetSuspendState, Shutdown::LockWorkStation};
    match command {
        SystemCommand::Settings => tauri_plugin_opener::open_url("ms-settings:", None::<&str>)
            .map_err(|error| Error::SystemCommand(error.to_string())),
        SystemCommand::Lock => {
            // SAFETY: No pointers; requests a lock of this interactive session.
            windows_result(unsafe { LockWorkStation() } != 0)
        }
        SystemCommand::ToggleAppearance => toggle_appearance(),
        SystemCommand::EmptyTrash => empty_recycle_bin(),
        SystemCommand::ShowDesktop => send_system_keys(&[
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_LWIN,
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_D,
        ]),
        SystemCommand::ToggleMute => {
            send_system_keys(&[windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_VOLUME_MUTE])
        }
        SystemCommand::Logout => {
            use windows_sys::Win32::System::Shutdown::{EWX_LOGOFF, ExitWindowsEx};
            // Logging out the current interactive session needs no shutdown
            // privilege. Do not force applications with unsaved work to close.
            windows_result(unsafe { ExitWindowsEx(EWX_LOGOFF, 0) } != 0)
        }
        _ => {
            // Token privileges belong to the process. Serialize their temporary
            // changes so simultaneous IPC requests cannot restore stale state.
            static POWER: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = POWER.lock().map_err(|_| {
                Error::SystemCommand("A power command failed. Restart TinyDash.".into())
            })?;
            let _privilege = ShutdownPrivilege::enable()?;
            // SAFETY: Fixed OS flags, no user pointers, privilege held for the call.
            if command == SystemCommand::Sleep {
                windows_result(unsafe { SetSuspendState(false, false, false) })
            } else {
                use windows_sys::Win32::System::Shutdown::{
                    ExitWindowsEx, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_APPLICATION,
                };
                let flags = shutdown_flags(command)?;
                windows_result(
                    unsafe {
                        ExitWindowsEx(
                            flags,
                            SHTDN_REASON_FLAG_PLANNED | SHTDN_REASON_MAJOR_APPLICATION,
                        )
                    } != 0,
                )
            }
        }
    }
}

fn empty_recycle_bin() -> Result<()> {
    use windows_sys::Win32::UI::Shell::{
        SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW,
    };
    // The Rust action boundary already checked explicit confirmation. Null
    // selects this user's Recycle Bins on all drives, not other users' files.
    let result = unsafe {
        SHEmptyRecycleBinW(
            ptr::null_mut(),
            ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
    };
    if result >= 0 {
        Ok(())
    } else {
        Err(Error::SystemCommand(format!(
            "Windows could not empty the Recycle Bin (0x{:08X}).",
            result as u32
        )))
    }
}

fn send_system_keys(keys: &[u16]) -> Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    // A numbered result shortcut can still have a modifier held. Wait for its
    // release rather than sending a different shortcut or releasing the user's keys.
    let started = std::time::Instant::now();
    while [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(i32::from(*key)) } < 0)
    {
        if started.elapsed() > std::time::Duration::from_secs(2) {
            return Err(Error::SystemCommand(
                "Release the modifier keys, then try this command again.".into(),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let event = |key, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs: Vec<_> = keys
        .iter()
        .map(|key| event(*key, 0))
        .chain(keys.iter().rev().map(|key| event(*key, KEYEVENTF_KEYUP)))
        .collect();
    // SAFETY: The array contains initialized keyboard records and remains alive.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        // Release any keys inserted before an incomplete send.
        let releases: Vec<_> = keys
            .iter()
            .rev()
            .map(|key| event(*key, KEYEVENTF_KEYUP))
            .collect();
        unsafe {
            SendInput(
                releases.len() as u32,
                releases.as_ptr(),
                size_of::<INPUT>() as i32,
            );
        }
        return Err(Error::SystemCommand(
            "Windows could not send the system shortcut. Check desktop permissions and try again."
                .into(),
        ));
    }
    Ok(())
}

fn toggle_appearance() -> Result<()> {
    use windows_sys::{
        Win32::{
            Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
            System::Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD,
                RRF_RT_REG_DWORD, RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW,
                RegSetValueExW,
            },
            UI::WindowsAndMessaging::{
                HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
            },
        },
        core::w,
    };
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    let check = |status| {
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(Error::SystemCommand(
                std::io::Error::from_raw_os_error(status as i32).to_string(),
            ))
        }
    };
    let mut key = Key(ptr::null_mut());
    // SAFETY: All names are static, null-terminated UTF-16; buffers match DWORD.
    unsafe {
        check(RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key.0,
        ))?;
        let read = |name| -> Result<Option<u32>> {
            let mut value = 0u32;
            let mut size = size_of::<u32>() as u32;
            let status = RegGetValueW(
                key.0,
                ptr::null(),
                name,
                RRF_RT_REG_DWORD,
                ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            );
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            check(status)?;
            Ok(Some(value))
        };
        let system_name = w!("SystemUsesLightTheme");
        let apps_name = w!("AppsUseLightTheme");
        let previous = read(system_name)?;
        let next: u32 = u32::from(previous.unwrap_or(1) == 0);
        let write = |name, value: &u32| {
            check(RegSetValueExW(
                key.0,
                name,
                0,
                REG_DWORD,
                (value as *const u32).cast(),
                size_of::<u32>() as u32,
            ))
        };
        write(system_name, &next)?;
        if let Err(error) = write(apps_name, &next) {
            // Restore the first value if the second write fails.
            let restored = match previous {
                Some(value) => write(system_name, &value),
                None => check(RegDeleteValueW(key.0, system_name)),
            };
            if let Err(restore_error) = restored {
                return Err(Error::SystemCommand(format!(
                    "{error} Could not restore the previous appearance: {restore_error}"
                )));
            }
            return Err(error);
        }
        let mut result = 0;
        if SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            w!("ImmersiveColorSet") as isize,
            SMTO_ABORTIFHUNG,
            1000,
            &mut result,
        ) == 0
        {
            tracing::debug!(
                "Appearance changed; some applications did not acknowledge the refresh"
            );
        }
    }
    Ok(())
}

fn windows_result(success: bool) -> Result<()> {
    if success {
        Ok(())
    } else {
        Err(Error::SystemCommand(
            std::io::Error::last_os_error().to_string(),
        ))
    }
}

fn shutdown_flags(command: crate::providers::system::SystemCommand) -> Result<u32> {
    use crate::providers::system::SystemCommand;
    use windows_sys::Win32::System::Shutdown::{EWX_POWEROFF, EWX_REBOOT};
    // Do not force applications to close. Unsaved work can cancel the request.
    match command {
        SystemCommand::Restart => Ok(EWX_REBOOT),
        SystemCommand::Shutdown => Ok(EWX_POWEROFF),
        _ => Err(Error::InvalidAction),
    }
}

struct ShutdownPrivilege {
    token: windows_sys::Win32::Foundation::HANDLE,
    previous: windows_sys::Win32::Security::TOKEN_PRIVILEGES,
}

impl ShutdownPrivilege {
    fn enable() -> Result<Self> {
        use windows_sys::Win32::{
            Foundation::{ERROR_NOT_ALL_ASSIGNED, GetLastError, LUID},
            Security::{
                AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
                SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES,
                TOKEN_QUERY,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        };
        let mut privilege = Self {
            token: ptr::null_mut(),
            previous: TOKEN_PRIVILEGES::default(),
        };
        let mut luid = LUID::default();
        // SAFETY: Valid output pointers and an owned token, released on every
        // path. The buffer holds the one privilege that we request to change.
        unsafe {
            windows_result(
                OpenProcessToken(
                    GetCurrentProcess(),
                    TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                    &mut privilege.token,
                ) != 0,
            )?;
            windows_result(LookupPrivilegeValueW(ptr::null(), SE_SHUTDOWN_NAME, &mut luid) != 0)?;
            let requested = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            let mut length = 0;
            let success = AdjustTokenPrivileges(
                privilege.token,
                0,
                &requested,
                size_of::<TOKEN_PRIVILEGES>() as u32,
                &mut privilege.previous,
                &mut length,
            );
            let error = GetLastError();
            if success == 0 || error == ERROR_NOT_ALL_ASSIGNED {
                return Err(Error::SystemCommand(
                    std::io::Error::from_raw_os_error(error as i32).to_string(),
                ));
            }
        }
        Ok(privilege)
    }
}

impl Drop for ShutdownPrivilege {
    fn drop(&mut self) {
        use windows_sys::Win32::{Foundation::CloseHandle, Security::AdjustTokenPrivileges};
        if !self.token.is_null() {
            // SAFETY: This object owns the token and the state returned by Win32.
            unsafe {
                if self.previous.PrivilegeCount != 0
                    && AdjustTokenPrivileges(
                        self.token,
                        0,
                        &self.previous,
                        0,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    ) == 0
                {
                    tracing::warn!(error = %std::io::Error::last_os_error(), "Could not restore the shutdown privilege");
                }
                CloseHandle(self.token);
            }
        }
    }
}

// Read only after the native counter changes. No window or frequent text polling.
pub fn clipboard_snapshot(
    previous: Option<u64>,
) -> anyhow::Result<Option<(u64, crate::providers::clipboard::Observed)>> {
    use crate::providers::clipboard::{MAX_TEXT_BYTES, Observed, SECRET_FORMATS};
    use windows_sys::Win32::System::{
        DataExchange::{
            CloseClipboard, CountClipboardFormats, GetClipboardData, GetClipboardSequenceNumber,
            IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        },
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
    };
    const UNICODE_TEXT: u32 = 13; // CF_UNICODETEXT, defined by Win32.
    let format = |name: &str| {
        let name = name.encode_utf16().chain([0]).collect::<Vec<_>>();
        // Registration returns the existing ID for a known name, or 0.
        unsafe { RegisterClipboardFormatW(name.as_ptr()) }
    };
    // All pointers come from Win32. Clipboard ownership and the global memory
    // lock remain held until the bounded data has been copied.
    unsafe {
        let counter = u64::from(GetClipboardSequenceNumber());
        if previous == Some(counter) {
            return Ok(None);
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            anyhow::bail!("Clipboard is busy");
        }
        struct Close;
        impl Drop for Close {
            fn drop(&mut self) {
                unsafe {
                    CloseClipboard();
                }
            }
        }
        let _close = Close;
        let counter = u64::from(GetClipboardSequenceNumber());
        if CountClipboardFormats() == 0 {
            return Ok(Some((counter, Observed::Cleared)));
        }
        let available = |id: u32| id != 0 && IsClipboardFormatAvailable(id) != 0;
        // Windows documents a DWORD 0 in this format as "exclude from history".
        let history_excluded = || {
            let id = format("CanIncludeInClipboardHistory");
            if !available(id) {
                return false;
            }
            let handle = GetClipboardData(id);
            if handle.is_null() || GlobalSize(handle) < 4 {
                return false;
            }
            let pointer = GlobalLock(handle);
            if pointer.is_null() {
                return false;
            }
            let value = pointer.cast::<u32>().read_unaligned();
            GlobalUnlock(handle);
            value == 0
        };
        if SECRET_FORMATS.iter().any(|name| available(format(name))) || history_excluded() {
            return Ok(Some((counter, Observed::Secret)));
        }
        if IsClipboardFormatAvailable(UNICODE_TEXT) == 0 {
            return Ok(Some((counter, Observed::Other)));
        }
        let handle = GetClipboardData(UNICODE_TEXT);
        if handle.is_null() {
            anyhow::bail!("Clipboard text is unavailable");
        }
        let bytes = GlobalSize(handle);
        if bytes == 0 || bytes > (MAX_TEXT_BYTES + 1) * 2 || !bytes.is_multiple_of(2) {
            return Ok(Some((counter, Observed::Other)));
        }
        let pointer = GlobalLock(handle);
        if pointer.is_null() {
            anyhow::bail!("Clipboard text could not be locked");
        }
        let data = std::slice::from_raw_parts(pointer.cast::<u16>(), bytes / 2);
        let text = data
            .iter()
            .position(|value| *value == 0)
            .and_then(|end| String::from_utf16(&data[..end]).ok());
        GlobalUnlock(handle);
        Ok(Some((
            u64::from(GetClipboardSequenceNumber()),
            Observed::from_text(text),
        )))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shutdown_requests_never_force_apps_to_close() {
        use crate::providers::system::SystemCommand;
        use windows_sys::Win32::System::Shutdown::{
            EWX_FORCE, EWX_FORCEIFHUNG, EWX_POWEROFF, EWX_REBOOT,
        };
        for (command, expected) in [
            (SystemCommand::Restart, EWX_REBOOT),
            (SystemCommand::Shutdown, EWX_POWEROFF),
        ] {
            let flags = super::shutdown_flags(command).expect("flags");
            assert_eq!(flags, expected);
            assert_eq!(flags & (EWX_FORCE | EWX_FORCEIFHUNG), 0);
        }
        assert!(super::shutdown_flags(SystemCommand::Sleep).is_err());
        assert!(super::shutdown_flags(SystemCommand::Lock).is_err());
    }

    #[test]
    fn only_accepts_app_shortcuts_and_executables() {
        assert!(super::is_app_extension(Some("LNK")));
        assert!(super::is_app_extension(Some("appref-ms")));
        assert!(!super::is_app_extension(Some("url")));
        assert!(!super::is_app_extension(Some("txt")));
        assert!(!super::is_app_extension(None));
    }
}
