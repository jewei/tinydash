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

#[cfg(test)]
mod tests {
    #[test]
    fn only_accepts_app_shortcuts_and_executables() {
        assert!(super::is_app_extension(Some("LNK")));
        assert!(super::is_app_extension(Some("appref-ms")));
        assert!(!super::is_app_extension(Some("url")));
        assert!(!super::is_app_extension(Some("txt")));
        assert!(!super::is_app_extension(None));
    }
}
