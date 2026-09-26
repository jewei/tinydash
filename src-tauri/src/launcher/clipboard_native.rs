//! Native clipboard provenance and bounded rich formats. No shell commands,
//! network requests, file-content reads, or text impersonating another format.
use super::{Payload, SourceApp};

#[cfg(target_os = "macos")]
pub fn source_app() -> Option<SourceApp> {
    use objc2_app_kit::NSWorkspace;
    objc2::rc::autoreleasepool(|_| {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        Some(SourceApp {
            id: app.bundleIdentifier()?.to_string(),
            name: app
                .localizedName()
                .map(|name| name.to_string())
                .unwrap_or_default(),
        })
    })
}

#[cfg(target_os = "windows")]
pub fn source_app() -> Option<SourceApp> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            DataExchange::GetClipboardOwner,
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::GetWindowThreadProcessId,
    };
    // Clipboard owner is more accurate than the current foreground application.
    // Protected processes and clipboard brokers may not expose an identity.
    unsafe {
        let window = GetClipboardOwner();
        if window.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buffer = [0u16; 32768];
        let mut size = buffer.len() as u32;
        let success = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size);
        CloseHandle(process);
        if success == 0 {
            return None;
        }
        let id = String::from_utf16(&buffer[..size as usize]).ok()?;
        let name = id.rsplit(['\\', '/']).next().unwrap_or(&id).to_owned();
        Some(SourceApp { id, name })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn source_app() -> Option<SourceApp> {
    // GTK does not expose a reliable cross-process application identity on
    // Wayland. Configured exclusions fail closed rather than silently leaking.
    None
}

#[cfg(target_os = "macos")]
pub fn counter() -> u64 {
    objc2::rc::autoreleasepool(|_| {
        objc2_app_kit::NSPasteboard::generalPasteboard().changeCount() as u64
    })
}
#[cfg(target_os = "windows")]
pub fn counter() -> u64 {
    // SAFETY: Win32 sequence read has no pointer parameters.
    unsafe { u64::from(windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber()) }
}

#[cfg(target_os = "macos")]
pub fn read(expected: u64, images: bool, files: bool) -> anyhow::Result<Option<Payload>> {
    use crate::providers::clipboard::is_secret_format;
    use anyhow::ensure;
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;
    if !images && !files {
        return Ok(None);
    }
    objc2::rc::autoreleasepool(|_| {
        let clipboard = NSPasteboard::generalPasteboard();
        ensure!(
            clipboard.changeCount() as u64 == expected,
            "Clipboard changed during read."
        );
        let types = clipboard.types();
        if types
            .as_ref()
            .is_some_and(|types| types.iter().any(|kind| is_secret_format(&kind.to_string())))
        {
            return Ok(None);
        }
        if source_app().is_some_and(|source| {
            ["com.apple.Passwords", "com.apple.keychainaccess"].contains(&source.id.as_str())
        }) {
            return Ok(None);
        }
        let payload = if files {
            // Finder still supplies the native filenames property list. Unlike
            // URI-looking plain text, this is an explicit file clipboard format.
            let kind = NSString::from_str("NSFilenamesPboardType");
            if let Some(data) = clipboard.dataForType(&kind) {
                ensure!(
                    data.len() <= super::MAX_FILE_BYTES,
                    "File references are too large."
                );
                let plist = plist::Value::from_reader(std::io::Cursor::new(data.to_vec()))?;
                let values = plist
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("Invalid file references."))?;
                ensure!(
                    values.len() <= super::MAX_FILES,
                    "Too many file references."
                );
                let paths = values
                    .iter()
                    .map(|value| {
                        value
                            .as_string()
                            .map(std::path::PathBuf::from)
                            .ok_or_else(|| anyhow::anyhow!("Invalid file reference."))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                super::valid_files(&paths)?;
                Some(Payload::Files(paths))
            } else {
                // A flat public.file-url read returns only the first item of a
                // multi-file clipboard. Skip URL-only sources rather than
                // silently saving an incomplete selection.
                None
            }
        } else {
            None
        };
        let payload = if payload.is_none() && images {
            if let Some(data) = clipboard.dataForType(&NSString::from_str("public.png")) {
                ensure!(
                    data.len() <= super::MAX_IMAGE_BYTES,
                    "PNG image is too large."
                );
                let bytes = data.to_vec();
                super::png_dimensions(&bytes)?;
                Some(Payload::Png(bytes))
            } else {
                None
            }
        } else {
            payload
        };
        ensure!(
            clipboard.changeCount() as u64 == expected,
            "Clipboard changed during read."
        );
        Ok(payload)
    })
}

#[cfg(target_os = "windows")]
pub fn read(_expected: u64, _images: bool, _files: bool) -> anyhow::Result<Option<Payload>> {
    // Do not decode unrestricted CF_DIB buffers or treat file paths as text.
    // The command response explicitly reports this unsupported capture path.
    Ok(None)
}

#[cfg(target_os = "macos")]
pub fn write(payload: &Payload) -> anyhow::Result<()> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSData, NSString};
    let (kind, bytes) = match payload {
        Payload::Png(bytes) => {
            super::png_dimensions(bytes)?;
            ("public.png", bytes.clone())
        }
        Payload::Files(paths) => {
            super::valid_files(paths)?;
            let value = plist::Value::Array(
                paths
                    .iter()
                    .map(|path| plist::Value::String(path.to_string_lossy().into_owned()))
                    .collect(),
            );
            let mut bytes = Vec::new();
            value.to_writer_binary(&mut bytes)?;
            ("NSFilenamesPboardType", bytes)
        }
    };
    objc2::rc::autoreleasepool(|_| {
        let clipboard = NSPasteboard::generalPasteboard();
        let data = NSData::with_bytes(&bytes);
        clipboard.clearContents();
        anyhow::ensure!(
            clipboard.setData_forType(Some(&data), &NSString::from_str(kind)),
            "Could not copy the saved clipboard format."
        );
        Ok(())
    })
}

#[cfg(not(target_os = "macos"))]
pub fn write(_payload: &Payload) -> anyhow::Result<()> {
    anyhow::bail!("Copying saved images and files is currently supported only on macOS.")
}
