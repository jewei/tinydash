use std::path::{Path, PathBuf};

use crate::{
    error::{Error, Result},
    providers::apps::AppEntry,
};

pub fn discover_apps() -> Result<Vec<AppEntry>> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Library/CoreServices/Finder.app"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    Ok(scan_roots(&roots))
}

fn scan_roots(roots: &[PathBuf]) -> Vec<AppEntry> {
    let mut apps = Vec::new();
    let mut skipped = 0;
    for root in roots.iter().filter(|path| path.exists()) {
        let mut walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(4)
            .into_iter();
        while let Some(entry) = walker.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.') {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
            {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                if path.is_dir() {
                    if let Some(app) = read_bundle(path) {
                        apps.push(app);
                    }
                }
            }
        }
    }
    if skipped > 0 {
        tracing::warn!(skipped, "Some application directories could not be read");
    }
    apps
}

fn read_bundle(path: &Path) -> Option<AppEntry> {
    let fallback = path.file_stem()?.to_string_lossy();
    let info = plist::Value::from_file(path.join("Contents/Info.plist")).ok();
    let dict = info.as_ref().and_then(plist::Value::as_dictionary);
    let string = |key| {
        dict.and_then(|dict| dict.get(key))
            .and_then(plist::Value::as_string)
    };
    if string("CFBundleIdentifier") == Some("dev.tinydash.launcher") {
        return None;
    }
    let background = dict
        .and_then(|dict| dict.get("LSBackgroundOnly"))
        .is_some_and(|value| {
            value.as_boolean() == Some(true)
                || value.as_signed_integer() == Some(1)
                || value
                    .as_string()
                    .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        });
    if background || string("CFBundlePackageType").is_some_and(|kind| kind != "APPL") {
        return None;
    }
    let name = string("CFBundleDisplayName")
        .or_else(|| string("CFBundleName"))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&fallback)
        .to_owned();
    let aliases = [
        Some(fallback.as_ref()),
        string("CFBundleName"),
        string("CFBundleExecutable"),
    ]
    .into_iter()
    .flatten()
    .filter(|alias| *alias != name)
    .map(str::to_owned)
    .collect();
    // Canonical paths deduplicate symlinked bundles without walking through symlink trees.
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    Some(AppEntry::new(name, path, aliases))
}

pub fn launch(entry: &AppEntry) -> Result<()> {
    tauri_plugin_opener::open_path(&entry.path, None::<&str>)
        .map_err(|error| Error::Launch(error.to_string()))
}

// Called on the clipboard worker. The counter check does not fetch text.
pub fn clipboard_snapshot(previous: Option<u64>) -> anyhow::Result<Option<(u64, Option<String>)>> {
    use crate::providers::clipboard::{MAX_TEXT_BYTES, valid_text};
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    objc2::rc::autoreleasepool(|_| {
        let clipboard = NSPasteboard::generalPasteboard();
        let counter = clipboard.changeCount() as u64;
        if previous == Some(counter) {
            return Ok(None);
        }
        // NSPasteboardTypeString is an immutable AppKit constant.
        let text = clipboard
            .stringForType(unsafe { NSPasteboardTypeString })
            .filter(|value| value.length() <= MAX_TEXT_BYTES)
            .map(|value| value.to_string())
            .filter(|value| valid_text(value));
        if clipboard.changeCount() as u64 != counter {
            anyhow::bail!("Clipboard changed during read");
        }
        Ok(Some((counter, text)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(root: &Path, name: &str, extra: &str) -> PathBuf {
        let path = root.join(format!("{name}.app"));
        std::fs::create_dir_all(path.join("Contents")).expect("create bundle");
        std::fs::write(path.join("Contents/Info.plist"), format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleDisplayName</key><string>{name} Display</string>{extra}</dict></plist>"
        )).expect("write metadata");
        path
    }

    #[test]
    fn finds_bundles_without_descending_into_them_or_hidden_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = bundle(dir.path(), "Editor", "");
        bundle(&app.join("Contents"), "Nested", "");
        bundle(&dir.path().join(".hidden"), "Hidden", "");
        bundle(dir.path(), "Agent", "<key>LSBackgroundOnly</key><true/>");
        let apps = scan_roots(&[dir.path().to_owned(), dir.path().join("missing")]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Editor Display");
        assert!(apps[0].aliases.contains(&"Editor".to_owned()));
    }

    #[test]
    fn handles_broken_metadata_and_symlink_loops() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("Broken.app")).expect("create bundle");
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).expect("symlink");
        let apps = scan_roots(&[dir.path().to_owned()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Broken");
    }

    #[test]
    fn hidden_files_and_bundle_symlinks_do_not_skip_siblings() {
        let dir = tempfile::tempdir().expect("tempdir");
        // A hidden file is not a directory to prune. A bundle symlink is not
        // descended into, so pruning it would incorrectly skip its parent.
        std::fs::write(dir.path().join(".DS_Store"), "").expect("hidden file");
        let target = tempfile::tempdir().expect("target");
        let linked = bundle(target.path(), "Linked", "");
        std::os::unix::fs::symlink(linked, dir.path().join("Linked.app")).expect("symlink");
        bundle(dir.path(), "Sibling", "");
        let apps = scan_roots(&[dir.path().to_owned()]);
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().any(|app| app.name == "Sibling Display"));
        assert!(apps.iter().any(|app| app.name == "Linked Display"));
    }
}
