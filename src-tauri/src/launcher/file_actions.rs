//! File IPC accepts only indexed result IDs. All filesystem/OS work happens
//! after the search mutex is released. Preview data is bounded and never HTML.
use std::{fs::OpenOptions, io::Read, path::Path, process::Command, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::{LauncherState, actions::ResolvedAction, result::Action, search::SearchManager};
use crate::{
    error::Error,
    providers::{apps::AppEntry, files::FileEntry},
};

const TEXT_LIMIT: usize = 64 * 1024;
const IMAGE_LIMIT: usize = 2 * 1024 * 1024;
const IMAGE_PIXEL_LIMIT: u64 = 16_000_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    name: String,
    path: String,
    folder: bool,
    size: u64,
    modified_at: Option<u64>,
    readonly: bool,
    content: PreviewContent,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum PreviewContent {
    Text {
        text: String,
        truncated: bool,
    },
    Image {
        #[serde(rename = "dataUrl")]
        data_url: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FileAction {
    CopyPath,
    CopyFile,
    OpenWith,
    OpenTerminal,
    QuickLook,
    Trash,
}

fn resolve_file(search: &SearchManager, id: &str) -> Result<FileEntry, String> {
    match search
        .resolve_action(id, Action::Open)
        .map_err(|error| error.to_string())?
    {
        ResolvedAction::File(entry, _) => Ok(entry),
        _ => Err(Error::InvalidAction.to_string()),
    }
}

fn resolve_app(search: &SearchManager, id: Option<&str>) -> Result<AppEntry, String> {
    let id = id.ok_or("Choose an application before using Open With.")?;
    match search
        .resolve_action(id, Action::Launch)
        .map_err(|error| error.to_string())?
    {
        ResolvedAction::Launch(entry) => Ok(entry),
        _ => Err(Error::InvalidAction.to_string()),
    }
}

fn require_confirmation(action: FileAction, confirmed: bool) -> Result<(), String> {
    if action == FileAction::Trash && !confirmed {
        return Err("Confirm moving this item to Trash before continuing.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn file_preview(id: String, app: AppHandle) -> Result<FilePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = {
            let state = app.state::<LauncherState>();
            let search = state
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable.to_string())?;
            resolve_file(&search, &id)?
        };
        preview(&entry)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn execute_file_action(
    id: String,
    action: FileAction,
    app_id: Option<String>,
    confirmed: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    // Missing confirmation is never consent, even for direct IPC callers.
    require_confirmation(action, confirmed.unwrap_or(false))?;
    tauri::async_runtime::spawn_blocking(move || {
        let (entry, selected_app) = {
            let state = app.state::<LauncherState>();
            let search = state
                .search
                .lock()
                .map_err(|_| Error::IndexUnavailable.to_string())?;
            (
                resolve_file(&search, &id)?,
                if action == FileAction::OpenWith {
                    Some(resolve_app(&search, app_id.as_deref())?)
                } else {
                    None
                },
            )
        };
        entry.validate().map_err(|error| error.to_string())?;
        match action {
            FileAction::CopyPath => app
                .state::<LauncherState>()
                .storage
                .copy(&app, &id, entry.path),
            FileAction::CopyFile => copy_file(&entry),
            FileAction::OpenWith => open_with(&entry, &selected_app.expect("resolved app")),
            FileAction::OpenTerminal => open_terminal(&entry),
            FileAction::QuickLook => quick_look(&entry),
            FileAction::Trash => {
                trash::delete(&entry.path)
                    .map_err(|error| format!("Could not move this item to Trash: {error}"))?;
                super::files::scan_files(&app);
                Ok(())
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

fn unavailable(reason: &str) -> PreviewContent {
    PreviewContent::Unavailable {
        reason: reason.into(),
    }
}

fn preview(entry: &FileEntry) -> Result<FilePreview, String> {
    entry.validate().map_err(|error| error.to_string())?;
    let metadata = std::fs::metadata(&entry.path).map_err(|error| error.to_string())?;
    let content = if entry.folder {
        unavailable("Folder contents are not previewed.")
    } else {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Do not block on a substituted FIFO or follow a final symlink.
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_FLAG_OPEN_REPARSE_POINT: inspect links instead of following.
            options.custom_flags(0x0020_0000);
        }
        let mut file = options
            .open(&entry.path)
            .map_err(|error| error.to_string())?;
        // Validate again after opening. Special files and symlink substitutions
        // are never intentionally accepted as indexed regular files.
        entry.validate().map_err(|error| error.to_string())?;
        let opened_metadata = file.metadata().map_err(|error| error.to_string())?;
        if !opened_metadata.is_file() || opened_metadata.file_type().is_symlink() {
            return Err(Error::FileNotFound.to_string());
        }
        let mut prefix = [0u8; 16];
        let count = file.read(&mut prefix).map_err(|error| error.to_string())?;
        let mime = image_mime(&prefix[..count]);
        let limit = if mime.is_some() {
            IMAGE_LIMIT
        } else {
            TEXT_LIMIT
        };
        let mut bytes = Vec::with_capacity(limit.min(metadata.len() as usize));
        bytes.extend_from_slice(&prefix[..count]);
        file.take((limit + 1 - count) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if let Some(mime) = mime {
            if bytes.len() > IMAGE_LIMIT {
                unavailable("Image preview is limited to 2 MiB.")
            } else if !image_dimensions(&bytes).is_some_and(|(w, h)| {
                w > 0 && h > 0 && u64::from(w) * u64::from(h) <= IMAGE_PIXEL_LIMIT
            }) {
                unavailable("Image dimensions are unsupported or exceed 16 megapixels.")
            } else {
                PreviewContent::Image {
                    data_url: format!("data:{mime};base64,{}", encode_base64(&bytes)),
                }
            }
        } else {
            text_preview(bytes)
        }
    };
    Ok(FilePreview {
        name: entry.name.clone(),
        path: entry.path.clone(),
        folder: entry.folder,
        size: metadata.len(),
        readonly: metadata.permissions().readonly(),
        modified_at: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|time| time.as_secs()),
        content,
    })
}

fn text_preview(mut bytes: Vec<u8>) -> PreviewContent {
    let truncated = bytes.len() > TEXT_LIMIT;
    bytes.truncate(TEXT_LIMIT);
    // A UTF-8 character split by the byte cap is omitted, not replaced.
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).expect("valid prefix")
        }
        Err(_) => {
            return unavailable("Preview supports UTF-8 text and PNG, JPEG, GIF, or WebP images.");
        }
    };
    if text
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return unavailable("Binary file contents are not previewed.");
    }
    PreviewContent::Text {
        text: text.to_owned(),
        truncated,
    }
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let be32 = |start| {
        Some(u32::from_be_bytes(
            bytes.get(start..start + 4)?.try_into().ok()?,
        ))
    };
    let le16 = |start| {
        Some(u32::from(u16::from_le_bytes(
            bytes.get(start..start + 2)?.try_into().ok()?,
        )))
    };
    match image_mime(bytes)? {
        "image/png" => Some((be32(16)?, be32(20)?)),
        "image/gif" => Some((le16(6)?, le16(8)?)),
        "image/webp" => match bytes.get(12..16)? {
            b"VP8X" => {
                let dimension = |start| {
                    let v = bytes.get(start..start + 3)?;
                    Some(1 + u32::from(v[0]) + (u32::from(v[1]) << 8) + (u32::from(v[2]) << 16))
                };
                Some((dimension(24)?, dimension(27)?))
            }
            b"VP8 " if bytes.get(23..26)? == [0x9d, 0x01, 0x2a] => {
                Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff))
            }
            b"VP8L" if *bytes.get(20)? == 0x2f => {
                let v = u32::from_le_bytes(bytes.get(21..25)?.try_into().ok()?);
                Some(((v & 0x3fff) + 1, ((v >> 14) & 0x3fff) + 1))
            }
            _ => None,
        },
        "image/jpeg" => {
            let mut cursor = 2;
            while cursor < bytes.len() {
                if *bytes.get(cursor)? != 0xff {
                    return None;
                }
                while *bytes.get(cursor)? == 0xff {
                    cursor += 1;
                }
                let marker = *bytes.get(cursor)?;
                cursor += 1;
                if marker == 0xd9 || marker == 0xda {
                    return None;
                }
                if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
                    continue;
                }
                let length = usize::from(u16::from_be_bytes(
                    bytes.get(cursor..cursor + 2)?.try_into().ok()?,
                ));
                if length < 2 {
                    return None;
                }
                if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                    if length < 7 {
                        return None;
                    }
                    let h = u16::from_be_bytes(bytes.get(cursor + 3..cursor + 5)?.try_into().ok()?);
                    let w = u16::from_be_bytes(bytes.get(cursor + 5..cursor + 7)?.try_into().ok()?);
                    return Some((u32::from(w), u32::from(h)));
                }
                cursor += length;
            }
            None
        }
        _ => None,
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(a >> 2) as usize] as char);
        output.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn run(command: &mut Command, capability: &str) -> Result<(), String> {
    let status = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("{capability} is unavailable: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{capability} failed ({status})."))
    }
}

fn open_with(entry: &FileEntry, selected: &AppEntry) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        run(
            Command::new("/usr/bin/open")
                .arg("-a")
                .arg(&selected.path)
                .arg("--")
                .arg(&entry.path),
            "Open With",
        )
    }
    #[cfg(target_os = "linux")]
    {
        use gio::prelude::AppInfoExt;
        let app = gio_unix::DesktopAppInfo::from_filename(&selected.path)
            .ok_or("This indexed application cannot open files.")?;
        app.launch(
            &[gio::File::for_path(&entry.path)],
            None::<&gio::AppLaunchContext>,
        )
        .map_err(|error| format!("Open With failed: {error}"))
    }
    #[cfg(target_os = "windows")]
    {
        let _ = (entry, selected);
        Err("Open With using an indexed application is not supported on Windows yet. Use the system file manager's Open With menu.".into())
    }
}

fn open_terminal(entry: &FileEntry) -> Result<(), String> {
    let folder = if entry.folder {
        Path::new(&entry.path)
    } else {
        Path::new(&entry.path)
            .parent()
            .ok_or("This file has no enclosing folder.")?
    };
    #[cfg(target_os = "macos")]
    {
        run(
            Command::new("/usr/bin/open")
                .arg("-a")
                .arg("Terminal")
                .arg("--")
                .arg(folder),
            "Open in Terminal",
        )
    }
    #[cfg(target_os = "windows")]
    {
        run(
            Command::new("wt.exe").arg("-d").arg(folder),
            "Open in Windows Terminal (requires Windows Terminal)",
        )
    }
    #[cfg(target_os = "linux")]
    {
        let mut child = Command::new("x-terminal-emulator")
            .current_dir(folder)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| format!("Open in Terminal requires x-terminal-emulator: {error}"))?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

fn quick_look(entry: &FileEntry) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("/usr/bin/qlmanage")
            .arg("-p")
            .arg(&entry.path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| format!("Quick Look is unavailable: {error}"))?;
        // qlmanage remains alive until its native preview closes. Reap it without
        // holding the IPC request or blocking later file actions.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = entry;
        Err("Native Quick Look is available only on macOS. Use the inline preview instead.".into())
    }
}

fn copy_file(entry: &FileEntry) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // The filename is argv data, never interpolated into AppleScript source.
        run(
            Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg("on run argv\nset the clipboard to (POSIX file (item 1 of argv))\nend run")
                .arg(&entry.path),
            "Copy File",
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = entry;
        Err("Copy File is not supported on this platform yet. Copy Path is available.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::files::{ScanReport, scan};

    fn fixture(root: &Path, name: &str, bytes: &[u8]) -> (SearchManager, FileEntry) {
        std::fs::write(root.join(name), bytes).unwrap();
        let mut search = SearchManager::default();
        search.replace_files(scan(
            vec![root.into()],
            &[],
            100,
            &mut ScanReport::default(),
        ));
        let id = format!("file:{}", root.canonicalize().unwrap().join(name).display());
        let entry = resolve_file(&search, &id).unwrap();
        (search, entry)
    }

    #[test]
    fn only_backend_ids_are_accepted_and_trash_requires_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let (search, _) = fixture(dir.path(), "safe.txt", b"hello");
        assert!(resolve_file(&search, "file:/unindexed.txt").is_err());
        assert!(resolve_app(&search, None).is_err());
        assert!(resolve_app(&search, Some("app:/unindexed")).is_err());
        assert!(require_confirmation(FileAction::Trash, false).is_err());
        assert!(require_confirmation(FileAction::Trash, true).is_ok());
        assert!(dir.path().join("safe.txt").exists());
    }

    #[test]
    fn text_is_bounded_utf8_safe_and_html_is_only_text() {
        let mut bytes = vec![b'a'; TEXT_LIMIT - 1];
        bytes.extend_from_slice("érest".as_bytes());
        let PreviewContent::Text { text, truncated } = text_preview(bytes) else {
            panic!("text");
        };
        assert!(truncated);
        assert_eq!(text.len(), TEXT_LIMIT - 1);
        assert!(matches!(
            text_preview(b"abc\0def".to_vec()),
            PreviewContent::Unavailable { .. }
        ));
        let dir = tempfile::tempdir().unwrap();
        let (_, entry) = fixture(dir.path(), "unsafe.html", b"<script>alert(1)</script>");
        let preview = preview(&entry).unwrap();
        assert!(matches!(preview.content, PreviewContent::Text { .. }));
        assert_eq!(preview.size, 25);
        std::fs::remove_file(&entry.path).unwrap();
        assert!(super::preview(&entry).is_err());
    }

    #[test]
    fn images_use_allowlisted_mime_and_bounded_dimensions() {
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert!(image_mime(b"<svg onload='bad()'>").is_none());
        let dir = tempfile::tempdir().unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&1u32.to_be_bytes());
        png.extend_from_slice(&1u32.to_be_bytes());
        let (_, entry) = fixture(dir.path(), "image.png", &png);
        assert!(
            matches!(preview(&entry).unwrap().content, PreviewContent::Image { data_url } if data_url.starts_with("data:image/png;base64,"))
        );
        png[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        std::fs::write(&entry.path, &png).unwrap();
        assert!(matches!(
            preview(&entry).unwrap().content,
            PreviewContent::Unavailable { .. }
        ));
        png.resize(IMAGE_LIMIT + 1, 0);
        std::fs::write(&entry.path, png).unwrap();
        assert!(matches!(
            preview(&entry).unwrap().content,
            PreviewContent::Unavailable { .. }
        ));
    }
}
