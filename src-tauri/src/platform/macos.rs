use std::path::{Path, PathBuf};

mod descriptions;
mod focus;
mod icons;

pub use focus::LauncherFocus;

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
                if path.is_dir()
                    && let Some(app) = read_bundle(path)
                {
                    apps.push(app);
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
    // Finder is a launchable application with Apple's special FNDR package type.
    let finder = string("CFBundleIdentifier") == Some("com.apple.finder")
        && string("CFBundlePackageType") == Some("FNDR");
    if background || string("CFBundlePackageType").is_some_and(|kind| kind != "APPL" && !finder) {
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
    let mut app = AppEntry::new(name, path, aliases);
    app.description = descriptions::app_description(
        string("CFBundleIdentifier"),
        string("LSApplicationCategoryType"),
    )
    .to_owned();
    Some(app)
}

pub fn load_app_icons(mut apps: Vec<AppEntry>) -> Vec<AppEntry> {
    for entry in &mut apps {
        entry.icon = icons::application_icon(&entry.path);
    }
    apps
}

pub fn launch(entry: &AppEntry) -> Result<()> {
    tauri_plugin_opener::open_path(&entry.path, None::<&str>)
        .map_err(|error| Error::Launch(error.to_string()))
}

const LOCK_HELPER: &str =
    "/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession";

pub fn system_commands() -> Vec<crate::providers::system::SystemCommand> {
    crate::providers::system::SystemCommand::ALL.to_vec()
}

pub fn run_system_command(command: crate::providers::system::SystemCommand) -> Result<()> {
    use crate::providers::system::SystemCommand;
    match command {
        SystemCommand::Lock if Path::new(LOCK_HELPER).is_file() => {
            let status = std::process::Command::new(LOCK_HELPER)
                .arg("-suspend")
                .status()
                .map_err(|error| {
                    Error::SystemCommand(format!(
                        "Screen lock is unavailable. Use Control+Command+Q. {error}"
                    ))
                })?;
            if !status.success() {
                return Err(Error::SystemCommand(format!(
                    "Screen lock returned {status}. Use Control+Command+Q."
                )));
            }
            Ok(())
        }
        SystemCommand::Lock | SystemCommand::ShowDesktop => send_system_shortcut(command),
        SystemCommand::ToggleAppearance | SystemCommand::EmptyTrash | SystemCommand::ToggleMute => {
            run_script(script(command)?)
        }
        SystemCommand::Settings => {
            let path = [
                "/System/Applications/System Settings.app",
                "/System/Applications/System Preferences.app",
            ]
            .into_iter()
            .find(|path| Path::new(path).is_dir())
            .ok_or_else(|| Error::SystemCommand("System Settings is unavailable.".into()))?;
            tauri_plugin_opener::open_path(path, None::<&str>)
                .map_err(|error| Error::SystemCommand(error.to_string()))
        }
        _ => {
            use objc2_core_services::{AESendMessage, kAENeverInteract, kAENoReply};
            let event = power_event(command)?;
            let mut reply = AppleEventDescriptor::empty();
            // SAFETY: Both descriptors remain owned and valid for the call.
            // NoReply asks loginwindow to begin the normal OS transition; it
            // does not force applications to quit or wait for shutdown.
            let status = unsafe {
                AESendMessage(
                    &event.0,
                    &mut reply.0,
                    (kAENoReply | kAENeverInteract) as i32,
                    300,
                )
            };
            apple_event_status(status)
        }
    }
}

fn run_script(script: &str) -> Result<()> {
    // AppleScript supplies a numeric error independently of its localized text.
    // Catch it inside the script instead of guessing from osascript's stderr.
    let wrapped = format!(
        "try\n{script}\nreturn \"ok\"\non error errorMessage number errorNumber\nreturn (errorNumber as text) & linefeed & errorMessage\nend try"
    );
    let reply = super::system_process::output("/usr/bin/osascript", &["-e", &wrapped])?;
    if reply == "ok" {
        return Ok(());
    }
    let (code, message) = reply
        .split_once('\n')
        .and_then(|(code, message)| code.parse::<i32>().ok().map(|code| (code, message)))
        .ok_or_else(|| {
            Error::SystemCommand("macOS returned an unexpected script response.".into())
        })?;
    Err(Error::SystemCommand(match code {
        -128 => "macOS canceled the command (error -128).".into(),
        -1743 => "macOS denied Automation access. Allow TinyDash in System Settings > Privacy & Security > Automation.".into(),
        _ => format!("{message} (macOS error {code})."),
    }))
}

fn script(command: crate::providers::system::SystemCommand) -> Result<&'static str> {
    use crate::providers::system::SystemCommand;
    match command {
        SystemCommand::ToggleAppearance => Ok(
            "tell application \"System Events\" to tell appearance preferences to set dark mode to not dark mode",
        ),
        SystemCommand::EmptyTrash => Ok(
            "tell application \"Finder\"\nif (count of items of trash) > 0 then empty trash\nend tell",
        ),
        SystemCommand::ToggleMute => {
            Ok("set volume output muted not (output muted of (get volume settings))")
        }
        _ => Err(Error::InvalidAction),
    }
}

fn system_shortcut(
    command: crate::providers::system::SystemCommand,
) -> Result<(u16, objc2_core_graphics::CGEventFlags)> {
    use crate::providers::system::SystemCommand;
    use objc2_core_graphics::CGEventFlags;
    match command {
        // Apple's documented Control-Command-Q and Fn-F11 shortcuts. F11 is
        // independent of the keyboard's text layout, unlike an injected H.
        SystemCommand::Lock => Ok((12, CGEventFlags::MaskControl | CGEventFlags::MaskCommand)),
        SystemCommand::ShowDesktop => Ok((103, CGEventFlags::MaskSecondaryFn)),
        _ => Err(Error::InvalidAction),
    }
}

fn send_system_shortcut(command: crate::providers::system::SystemCommand) -> Result<()> {
    use objc2_core_graphics::{
        CGEvent, CGEventTapLocation, CGPreflightPostEventAccess, CGRequestPostEventAccess,
    };
    let (key, flags) = system_shortcut(command)?;
    // Ask only when the user runs a shortcut action. Never request access during search.
    if !CGPreflightPostEventAccess() && !CGRequestPostEventAccess() {
        return Err(Error::SystemCommand(
            "Allow TinyDash in System Settings > Privacy & Security > Accessibility, then try again.".into(),
        ));
    }
    let down = CGEvent::new_keyboard_event(None, key, true);
    let up = CGEvent::new_keyboard_event(None, key, false);
    let (Some(down), Some(up)) = (down, up) else {
        return Err(Error::SystemCommand(
            "Could not create the system keyboard shortcut.".into(),
        ));
    };
    for event in [&down, &up] {
        CGEvent::set_flags(Some(event), flags);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event));
    }
    Ok(())
}

struct AppleEventDescriptor(objc2_core_services::AEDesc);

impl AppleEventDescriptor {
    fn empty() -> Self {
        Self(objc2_core_services::AEDesc {
            descriptorType: objc2_core_services::typeNull,
            dataHandle: std::ptr::null_mut(),
        })
    }
}

impl Drop for AppleEventDescriptor {
    fn drop(&mut self) {
        // SAFETY: This descriptor is initialized, uniquely owned, and never
        // copied. AE accepts empty descriptors as well as allocated ones.
        unsafe {
            objc2_core_services::AEDisposeDesc(&mut self.0);
        }
    }
}

fn apple_event_status(status: i32) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(Error::SystemCommand(format!(
            "macOS rejected the Apple event (status {status}). Use the Apple menu if this command is unavailable."
        )))
    }
}

fn power_event(command: crate::providers::system::SystemCommand) -> Result<AppleEventDescriptor> {
    use crate::providers::system::SystemCommand;
    use objc2_core_services::{
        AECreateAppleEvent, AECreateDesc, kAELogOut, kAERestart, kAEShutDown, kAESleep,
        kAnyTransactionID, kAutoGenerateReturnID, typeProcessSerialNumber,
    };
    let event_id = match command {
        SystemCommand::Sleep => kAESleep,
        SystemCommand::Restart => kAERestart,
        SystemCommand::Shutdown => kAEShutDown,
        SystemCommand::Logout => kAELogOut,
        _ => return Err(Error::InvalidAction),
    };
    // Apple's documented system-process address (ProcessSerialNumber {0, 1}).
    let address = [0u32, 1u32];
    let mut target = AppleEventDescriptor::empty();
    let mut event = AppleEventDescriptor::empty();
    // SAFETY: The two u32s have the ProcessSerialNumber layout. AE copies their
    // bytes. All output descriptors are initialized and disposed by their owner.
    unsafe {
        apple_event_status(i32::from(AECreateDesc(
            typeProcessSerialNumber,
            address.as_ptr().cast(),
            size_of_val(&address) as i64,
            &mut target.0,
        )))?;
        apple_event_status(i32::from(AECreateAppleEvent(
            u32::from_be_bytes(*b"aevt"),
            event_id,
            &target.0,
            kAutoGenerateReturnID as i16,
            kAnyTransactionID,
            &mut event.0,
        )))?;
    }
    Ok(event)
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

    #[test]
    fn apple_script_cancellation_does_not_claim_permission_failure() {
        // Execute a harmless script through the production runner. Never empty
        // the developer's Trash or require another application's permission.
        let error = run_script("error \"The operation can't be completed.\" number -128")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("Automation"), "{error}");
        assert!(error.contains("canceled"), "{error}");
        assert!(error.matches("Could not run the system command").count() <= 1);
    }

    #[test]
    fn apple_script_permission_advice_requires_the_permission_error_code() {
        let denied = run_script("error \"Not authorized\" number -1743")
            .unwrap_err()
            .to_string();
        assert!(
            denied.contains("Privacy & Security > Automation"),
            "{denied}"
        );
        assert_eq!(
            denied.matches("Could not run the system command").count(),
            1
        );

        let other = run_script("error \"An item is locked (-1743)\" number -45")
            .unwrap_err()
            .to_string();
        assert!(other.contains("An item is locked"), "{other}");
        assert!(!other.contains("Automation"), "{other}");
        assert!(other.contains("-45"), "{other}");
        assert!(run_script("set fixture to 1").is_ok());
    }

    #[test]
    fn empty_trash_skips_deletion_when_there_are_no_items() {
        use crate::providers::system::SystemCommand;
        // Exercise the command's AppleScript control flow. Replace only its OS
        // reads and deletion with fixtures so no test can delete real items.
        let fixture = script(SystemCommand::EmptyTrash)
            .unwrap()
            .replace("empty trash", "error \"Deletion requested\" number -2700");
        for (count, should_delete) in [(0, false), (1, true), (20, true)] {
            let result =
                run_script(&fixture.replace("count of items of trash", &count.to_string()));
            if should_delete {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("Deletion requested")
                );
            } else {
                assert!(
                    result.is_ok(),
                    "Empty Trash must do nothing when empty: {result:?}"
                );
            }
        }
    }

    #[test]
    fn only_shortcut_commands_can_send_keys() {
        use crate::providers::system::SystemCommand;
        use objc2_core_graphics::CGEventFlags;
        assert_eq!(
            system_shortcut(SystemCommand::Lock).unwrap(),
            (12, CGEventFlags::MaskControl | CGEventFlags::MaskCommand)
        );
        assert_eq!(
            system_shortcut(SystemCommand::ShowDesktop).unwrap(),
            (103, CGEventFlags::MaskSecondaryFn)
        );
        for command in SystemCommand::ALL {
            assert_eq!(
                system_shortcut(command).is_ok(),
                matches!(command, SystemCommand::Lock | SystemCommand::ShowDesktop)
            );
        }
    }

    #[test]
    fn scripts_compile_without_running_system_actions() {
        use crate::providers::system::SystemCommand;
        let directory = tempfile::tempdir().unwrap();
        for command in [
            SystemCommand::ToggleAppearance,
            SystemCommand::EmptyTrash,
            SystemCommand::ToggleMute,
        ] {
            let status = std::process::Command::new("/usr/bin/osacompile")
                .args([
                    "-o",
                    directory.path().join("action.scpt").to_str().unwrap(),
                    "-e",
                    script(command).unwrap(),
                ])
                .status()
                .unwrap();
            assert!(status.success(), "{command:?}");
        }
    }

    #[test]
    fn constructs_system_apple_events_without_sending_them() {
        use crate::providers::system::SystemCommand;
        use objc2_core_services::{AEGetAttributePtr, keyEventClassAttr, keyEventIDAttr, typeType};
        for (command, id) in [
            (SystemCommand::Sleep, *b"slep"),
            (SystemCommand::Restart, *b"rest"),
            (SystemCommand::Shutdown, *b"shut"),
            (SystemCommand::Logout, *b"logo"),
        ] {
            let event = power_event(command).expect("create Apple event");
            for (key, expected) in [(keyEventClassAttr, *b"aevt"), (keyEventIDAttr, id)] {
                let mut value = 0u32;
                let mut actual_type = 0;
                let mut actual_size = 0;
                // Reads the descriptor only. This test cannot send a power event.
                let status = unsafe {
                    AEGetAttributePtr(
                        &event.0,
                        key,
                        typeType,
                        &mut actual_type,
                        (&mut value as *mut u32).cast(),
                        4,
                        &mut actual_size,
                    )
                };
                assert_eq!(status, 0);
                assert_eq!(actual_size, 4);
                assert_eq!(value, u32::from_be_bytes(expected));
            }
        }
        assert!(power_event(SystemCommand::Settings).is_err());
        assert!(system_commands().contains(&SystemCommand::Lock));
    }

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
    fn finds_finder_with_its_special_bundle_type() {
        let directory = tempfile::tempdir().unwrap();
        let finder = bundle(
            &directory.path().join("CoreServices"),
            "Finder",
            "<key>CFBundleIdentifier</key><string>com.apple.finder</string>\
             <key>CFBundlePackageType</key><string>FNDR</string>",
        );
        let applications = directory.path().join("Applications");
        bundle(
            &applications,
            "NotFinder",
            "<key>CFBundleIdentifier</key><string>example.other</string>\
             <key>CFBundlePackageType</key><string>FNDR</string>",
        );
        let apps = scan_roots(&[applications, finder.clone()]);
        assert_eq!(apps.len(), 1, "Finder must be searchable");
        assert_eq!(apps[0].path, finder.canonicalize().unwrap());
        let mut search = crate::launcher::search::SearchManager::default();
        search.replace_apps(crate::providers::apps::AppProvider::new(apps));
        let result = search
            .search("finder", crate::launcher::query::SearchMode::Apps)
            .unwrap()
            .results;
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].primary_action,
            crate::launcher::result::Action::Launch
        );
    }

    #[test]
    fn discovers_browser_pwa_bundles_with_localized_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let pwa = bundle(
            directory.path(),
            "新闻 PWA",
            "<key>CFBundleIdentifier</key><string>com.google.Chrome.app.news</string>\
             <key>CFBundleName</key><string>新闻</string>\
             <key>CFBundlePackageType</key><string>APPL</string>",
        );

        let apps = scan_roots(&[directory.path().to_owned()]);
        let app = apps
            .iter()
            .find(|app| app.path == pwa.canonicalize().unwrap())
            .expect("Chrome PWA bundle should be discovered");
        assert_eq!(app.name, "新闻 PWA Display");
        assert!(app.aliases.contains(&"新闻".to_owned()));
    }

    #[test]
    fn keeps_apps_with_the_same_bundle_id_at_distinct_paths() {
        let directory = tempfile::tempdir().unwrap();
        let first = bundle(
            directory.path(),
            "First copy",
            "<key>CFBundleIdentifier</key><string>com.example.shared</string>",
        );
        let second = bundle(
            directory.path(),
            "Second copy",
            "<key>CFBundleIdentifier</key><string>com.example.shared</string>",
        );

        let apps = scan_roots(&[directory.path().to_owned()]);
        assert_eq!(apps.len(), 2);
        assert_ne!(apps[0].id, apps[1].id);
        let provider = crate::providers::apps::AppProvider::new(apps);
        assert_eq!(provider.len(), 2);
        assert!(
            provider
                .get(&format!("app:{}", first.canonicalize().unwrap().display()))
                .is_some()
        );
        assert!(
            provider
                .get(&format!("app:{}", second.canonicalize().unwrap().display()))
                .is_some()
        );
    }

    #[test]
    fn descriptions_use_bundle_identity_then_category_and_keep_paths_searchable() {
        use crate::{
            launcher::{query::SearchMode, search::SearchManager},
            providers::apps::AppProvider,
        };

        let directory = tempfile::tempdir().unwrap();
        let browser = bundle(
            directory.path(),
            "Renamed browser",
            "<key>CFBundleIdentifier</key><string>com.apple.Safari</string>\
             <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>",
        );
        let other = bundle(
            directory.path(),
            "Safari",
            "<key>CFBundleIdentifier</key><string>example.other</string>\
             <key>LSApplicationCategoryType</key><string>public.app-category.business</string>",
        );
        let unknown = bundle(
            directory.path(),
            "Unknown",
            "<key>LSApplicationCategoryType</key><string>example.unknown</string>",
        );
        let apps = scan_roots(&[directory.path().to_owned()]);
        for (path, expected) in [
            (browser, "Web browser"),
            (other, "Business"),
            (unknown, "Application"),
        ] {
            let path = path.canonicalize().unwrap();
            let app = apps.iter().find(|app| app.path == path).unwrap();
            assert_eq!(app.description, expected);
            let result = app.result(0);
            assert_eq!(result.subtitle, expected);
            assert_eq!(result.path.as_deref(), path.to_str());
        }
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(apps));
        let results = manager
            .search("Renamed browser.app", SearchMode::Apps)
            .unwrap()
            .results;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subtitle, "Web browser");
    }

    #[test]
    fn handles_broken_metadata_and_symlink_loops() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("Broken.app")).expect("create bundle");
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).expect("symlink");
        let apps = scan_roots(&[dir.path().to_owned()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Broken");
        assert_eq!(apps[0].description, "Application");
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
