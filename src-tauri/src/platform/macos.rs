use std::{
    cell::{OnceCell, RefCell},
    ffi::{c_ulong, c_void},
    path::{Path, PathBuf},
    process::Command,
    ptr::NonNull,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

use objc2::{
    AllocAnyThread,
    rc::{Retained, autoreleasepool},
};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSBitmapImageFileType, NSBitmapImageRep,
    NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImageInterpolation,
    NSPasteboard, NSPasteboardTypeTIFF, NSRunningApplication, NSWindow, NSWindowCollectionBehavior,
    NSWorkspace,
};
use objc2_core_foundation::{CFMachPort, CFRetained, CFRunLoop, kCFRunLoopCommonModes};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
    CGPreflightListenEventAccess, CGRequestListenEventAccess,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString, NSURL,
    NSURLVolumeAvailableCapacityForImportantUsageKey, NSURLVolumeLocalizedNameKey,
    NSURLVolumeTotalCapacityKey,
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, pixels_fit},
        expansion::{self, Key, Piece},
        system::SystemCommand,
    },
    settings::LauncherPosition,
};

pub const FILE_MANAGER: &str = "Finder";
/// Common package types: Finder shows each as one file, and an app bundle
/// alone can hold over 100,000 entries.
pub const PACKAGE_EXTENSIONS: &[&str] = &[
    "app",
    "appex",
    "bundle",
    "framework",
    "plugin",
    "kext",
    "xpc",
    "photoslibrary",
    "musiclibrary",
    "tvlibrary",
    "imovielibrary",
    "fcpbundle",
    "logicx",
    "band",
    "rtfd",
    "sparsebundle",
    "xcarchive",
    "xcodeproj",
    "xcworkspace",
    "playground",
];
pub const SELF_UPDATE: bool = true;
pub const RICH_CLIPBOARD: bool = true;
pub const NATIVE_ICONS: bool = true;
pub const PHYSICAL_PLACEMENT: bool = false;
/// FSEvents watches a whole tree with one stream.
pub const RECURSIVE_WATCH: bool = true;
pub const TEMPLATE_TRAY_ICON: bool = true;

/// Bundles are found up to this many folders deep.
const APP_DEPTH: usize = 4;
const OWN_BUNDLE_ID: &str = "dev.tinydash.launcher";
/// Apple's apps copy passwords without a secret marker; skip their copies.
const PASSWORD_APPS: &[&str] = &["com.apple.Passwords", "com.apple.keychainaccess"];

pub fn app_folders() -> Vec<PathBuf> {
    let mut folders = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        // Keychain Access, Archive Utility, and other Apple utilities.
        PathBuf::from("/System/Library/CoreServices/Applications"),
    ];
    if let Some(home) = std::env::home_dir() {
        folders.push(home.join("Applications"));
    }
    folders
}

pub fn discover_apps() -> Vec<App> {
    let mut apps = Vec::new();
    for folder in app_folders() {
        let mut walker = walkdir::WalkDir::new(folder)
            .max_depth(APP_DEPTH)
            .into_iter();
        while let Some(entry) = walker.next() {
            // An unreadable folder is an error entry; skip it and keep walking.
            let Ok(entry) = entry else {
                continue;
            };
            let name = entry.file_name().to_string_lossy();
            let is_dir = entry.file_type().is_dir();
            if entry.depth() > 0 && name.starts_with('.') {
                if is_dir {
                    walker.skip_current_dir();
                }
                continue;
            }
            if !name.to_ascii_lowercase().ends_with(".app") {
                continue;
            }
            if is_dir {
                walker.skip_current_dir();
                apps.extend(read_bundle(entry.path()));
            } else if entry.path_is_symlink()
                && let Ok(target) = entry.path().canonicalize()
            {
                apps.extend(read_bundle(&target));
            }
        }
    }
    apps.extend(read_bundle(Path::new(
        "/System/Library/CoreServices/Finder.app",
    )));
    apps
}

/// The Finder name (the bundle's file name) is the title; bundle names and
/// the executable are aliases, so `code` finds Visual Studio Code.
fn read_bundle(path: &Path) -> Option<App> {
    let info = plist::Value::from_file(path.join("Contents/Info.plist")).ok();
    let info = info.as_ref().and_then(plist::Value::as_dictionary);
    let string = |key| info?.get(key)?.as_string();
    let background_only = info
        .and_then(|info| info.get("LSBackgroundOnly"))
        .is_some_and(|value| value.as_boolean() == Some(true) || value.as_string() == Some("1"));
    if background_only || string("CFBundleIdentifier") == Some(OWN_BUNDLE_ID) {
        return None;
    }
    let name = path.file_stem()?.to_string_lossy().into_owned();
    let mut aliases: Vec<String> = Vec::new();
    for alias in ["CFBundleDisplayName", "CFBundleName", "CFBundleExecutable"]
        .into_iter()
        .filter_map(string)
    {
        if alias != name && !aliases.iter().any(|known| known == alias) {
            aliases.push(alias.to_owned());
        }
    }
    Some(App {
        name,
        path: path.to_str()?.to_owned(),
        aliases,
        description: describe(
            string("CFBundleIdentifier"),
            string("LSApplicationCategoryType"),
        )
        .map(String::from),
    })
}

/// What common apps are for, by bundle ID, so a renamed or localized app
/// still gets its words. Bundles rarely describe themselves.
const DESCRIPTIONS: &[(&str, &str)] = &[
    ("com.apple.Safari", "Web browser"),
    ("com.google.Chrome", "Web browser"),
    ("org.mozilla.firefox", "Web browser"),
    ("com.brave.Browser", "Web browser"),
    ("company.thebrowser.Browser", "Web browser"),
    ("com.microsoft.edgemac", "Web browser"),
    ("com.apple.finder", "Files and folders"),
    ("com.apple.Notes", "Notes and checklists"),
    ("com.apple.iCal", "Events and reminders"),
    ("com.apple.reminders", "Tasks and reminders"),
    ("com.apple.mail", "Email"),
    ("com.apple.MobileSMS", "Messages and conversations"),
    ("com.apple.FaceTime", "Voice and video calls"),
    ("com.apple.AddressBook", "Contacts"),
    ("com.apple.Preview", "Images and PDF documents"),
    ("com.apple.Photos", "Photo library"),
    ("com.apple.Music", "Music library and player"),
    ("com.apple.TV", "Movies and TV shows"),
    ("com.apple.podcasts", "Podcasts"),
    ("com.apple.Maps", "Maps and directions"),
    ("com.apple.weather", "Weather forecasts"),
    ("com.apple.calculator", "Calculations and conversions"),
    ("com.apple.TextEdit", "Text documents"),
    ("com.apple.Passwords", "Passwords and passkeys"),
    ("com.apple.systempreferences", "System preferences"),
    ("com.apple.AppStore", "App downloads and updates"),
    ("com.apple.ActivityMonitor", "System activity"),
    ("com.apple.DiskUtility", "Disk management"),
    (
        "com.apple.airport.airportutility",
        "Wi-Fi base station setup",
    ),
    ("com.apple.audio.AudioMIDISetup", "Audio and MIDI devices"),
    (
        "com.apple.BluetoothFileExchange",
        "Send files over Bluetooth",
    ),
    ("com.apple.ColorSyncUtility", "Color profiles"),
    ("com.apple.Console", "System logs"),
    ("com.apple.DigitalColorMeter", "Screen color picker"),
    ("com.apple.MigrateAssistant", "Move data from another Mac"),
    ("com.apple.printcenter", "Printers and print jobs"),
    ("com.apple.ScreenSharing", "Control another Mac"),
    (
        "com.apple.screenshot.launcher",
        "Screenshots and screen recordings",
    ),
    ("com.apple.ScriptEditor2", "AppleScript editor"),
    ("com.apple.SystemProfiler", "Hardware and software details"),
    ("com.apple.VoiceOverUtility", "VoiceOver settings"),
    ("com.apple.Automator", "Workflows"),
    ("com.apple.shortcuts", "Automations"),
    ("com.apple.FontBook", "Fonts"),
    ("com.apple.Image_Capture", "Import photos and scans"),
    ("com.apple.backup.launcher", "Backups"),
    ("com.apple.apps.launcher", "All apps"),
    ("com.apple.siri.launcher", "Voice assistant"),
    ("com.apple.exposelauncher", "Windows and desktops"),
    ("com.apple.VoiceMemos", "Voice recordings"),
    ("com.apple.archiveutility", "Zip archives"),
    ("com.apple.keychainaccess", "Passwords and certificates"),
    ("com.apple.Terminal", "Terminal emulator"),
    ("com.mitchellh.ghostty", "Terminal emulator"),
    ("com.googlecode.iterm2", "Terminal emulator"),
    ("dev.warp.Warp-Stable", "Terminal emulator"),
    ("com.apple.dt.Xcode", "App development"),
    ("com.microsoft.VSCode", "Code editor"),
    ("com.todesktop.230313mzl4w4u92", "Code editor"),
    ("dev.zed.Zed", "Code editor"),
    ("com.google.android.studio", "Android development"),
    ("com.tinyapp.TablePlus", "Database client"),
    ("com.github.GitHubClient", "Git repositories"),
    ("dev.kdrag0n.MacVirt", "Containers and Linux machines"),
    ("com.usebruno.app", "API client"),
    ("com.spotify.client", "Music streaming"),
    ("org.videolan.vlc", "Video player"),
    ("com.apple.QuickTimePlayerX", "Video player"),
    ("com.larksuite.larkApp", "Team chat and collaboration"),
    ("com.tinyspeck.slackmacgap", "Team chat"),
    ("com.hnc.Discord", "Voice and text chat"),
    ("net.whatsapp.WhatsApp", "Messages and calls"),
    ("us.zoom.xos", "Video meetings"),
    ("notion.id", "Notes and docs"),
    ("com.figma.Desktop", "Interface design"),
    ("com.anthropic.claudefordesktop", "AI assistant"),
    ("com.apple.iWork.Pages", "Documents"),
    ("com.apple.iWork.Numbers", "Spreadsheets"),
    ("com.apple.iWork.Keynote", "Presentations"),
];

/// The words for an app: its entry in [`DESCRIPTIONS`], else its App Store
/// category, else none.
fn describe(bundle_id: Option<&str>, category: Option<&str>) -> Option<&'static str> {
    if let Some(&(_, words)) = DESCRIPTIONS.iter().find(|(id, _)| Some(*id) == bundle_id) {
        return Some(words);
    }
    Some(match category?.strip_prefix("public.app-category.")? {
        "business" => "Business",
        "developer-tools" => "Developer tools",
        "education" => "Education",
        "entertainment" => "Entertainment",
        "finance" => "Finance",
        "games" | "action-games" | "adventure-games" | "arcade-games" | "board-games"
        | "card-games" | "casino-games" | "dice-games" | "educational-games" | "family-games"
        | "kids-games" | "music-games" | "puzzle-games" | "racing-games" | "role-playing-games"
        | "simulation-games" | "sports-games" | "strategy-games" | "trivia-games"
        | "word-games" => "Game",
        "graphics-design" => "Graphics and design",
        "healthcare-fitness" => "Health and fitness",
        "lifestyle" => "Lifestyle",
        "medical" => "Medical",
        "music" => "Music",
        "news" => "News",
        "photography" => "Photography",
        "productivity" => "Productivity",
        "reference" => "Reference",
        "social-networking" => "Social networking",
        "sports" => "Sports",
        "travel" => "Travel",
        "utilities" => "Utility",
        "video" => "Video",
        "weather" => "Weather",
        "books" => "Books",
        _ => return None,
    })
}

pub fn prepare_process() {}

pub fn prepare_app(app: &mut tauri::App) {
    // A launcher lives in the menu bar, not the Dock.
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Opening the running app from Finder or Spotlight sends this event; macOS
/// does not start a second process.
pub fn is_reopen(event: &tauri::RunEvent) -> bool {
    matches!(event, tauri::RunEvent::Reopen { .. })
}

/// Finder hides items with the `hidden` flag (`chflags hidden`), such as
/// `~/Library`.
pub const HIDDEN: Option<fn(&std::fs::Metadata) -> bool> = Some(has_hidden_flag);

fn has_hidden_flag(metadata: &std::fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    /// `UF_HIDDEN` in `<sys/stat.h>`.
    const UF_HIDDEN: u32 = 0x8000;
    metadata.st_flags() & UF_HIDDEN != 0
}

pub fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

pub fn launch_app(path: &Path) -> Result<()> {
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|error| Error::msg(error.to_string()))
}

/// Draw the Finder icon of `path` into a square bitmap of `pixels` and
/// encode it as PNG. AppKit picks the best representation for that size.
pub fn app_icon(path: &Path, pixels: u32) -> Option<Vec<u8>> {
    let path = NSString::from_str(path.to_str()?);
    let side = isize::try_from(pixels).ok()?;
    autoreleasepool(|_| {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
        // SAFETY: A null planes pointer asks AppKit to allocate the bitmap.
        // Four 8-bit samples with alpha match the packed RGBA layout.
        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                side,
                side,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        context.setImageInterpolation(NSImageInterpolation::High);
        let size = f64::from(pixels);
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size, size)),
            NSRect::ZERO,
            NSCompositingOperation::Copy,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
        // SAFETY: An empty dictionary of the expected type selects default PNG options.
        let png = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        Some(png.to_vec())
    })
}

/// Finder's name and numbers for the volume: free space counts purgeable
/// files, which macOS removes when space runs short.
pub fn disk_space(path: &Path) -> Result<super::Volume> {
    // `fileURLWithPath:` returns null for an empty path, which would panic.
    let path = path
        .to_str()
        .filter(|_| path.is_absolute())
        .ok_or_else(|| Error::msg("TinyDash could not find your home folder."))?;
    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        // SAFETY: Foundation defines these keys as constant strings that live
        // as long as the process.
        let (name_key, total_key, free_key) = unsafe {
            (
                NSURLVolumeLocalizedNameKey,
                NSURLVolumeTotalCapacityKey,
                NSURLVolumeAvailableCapacityForImportantUsageKey,
            )
        };
        let values = url
            .resourceValuesForKeys_error(&NSArray::from_slice(&[name_key, total_key, free_key]))
            .map_err(|error| Error::msg(error.localizedDescription().to_string()))?;
        let number = |key| {
            values
                .objectForKey(key)
                .and_then(|value| value.downcast::<NSNumber>().ok())
                .map(|number| number.unsignedLongLongValue())
                .ok_or_else(|| Error::msg("macOS did not report the disk size."))
        };
        let name = values
            .objectForKey(name_key)
            .and_then(|value| value.downcast::<NSString>().ok())
            .map_or_else(|| "Disk".into(), |name| name.to_string());
        Ok(super::Volume {
            name,
            total_bytes: number(total_key)?,
            free_bytes: number(free_key)?,
        })
    })
}

/// Show a notification through `UNUserNotificationCenter`. The first one
/// asks the user to allow notifications; asking again after an answer shows
/// nothing, so each one asks and is added only when allowed.
pub fn notify(_app: &tauri::AppHandle, title: &str, body: &str) -> Result<()> {
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSError};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNNotificationSound, UNUserNotificationCenter,
    };
    // The notification center raises an exception outside an app bundle,
    // as in a build made with --no-bundle.
    if !NSBundle::mainBundle()
        .bundlePath()
        .to_string()
        .ends_with(".app")
    {
        return Err(Error::msg("Notifications work only in the installed app."));
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    // A new identifier each time, so a notification never replaces the last.
    let id = format!("focus-{}", chrono::Utc::now().timestamp_millis());
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(&id),
        &content,
        None,
    );
    let add = block2::RcBlock::new(move |allowed: Bool, _error: *mut NSError| {
        if allowed.as_bool() {
            UNUserNotificationCenter::currentNotificationCenter()
                .addNotificationRequest_withCompletionHandler(&request, None);
        } else {
            tracing::info!("Notifications for TinyDash are off in System Settings");
        }
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &add,
        );
    Ok(())
}

/// Ask Spotlight for names that contain `name`, with fixed arguments and no
/// shell. Stops after `limit` paths or a second, whichever comes first.
pub fn find_files(name: &str, folder: &Path, limit: usize) -> Vec<PathBuf> {
    // mdfind reads a name that starts with "-" as an unknown option, and it
    // takes no "--", so it gets the name without its leading dashes.
    let asked = name.trim_start_matches('-');
    if asked.is_empty() {
        return Vec::new();
    }
    let mut command = Command::new("/usr/bin/mdfind");
    command.arg("-onlyin").arg(folder).arg("-name").arg(asked);
    let (mut child, output) = match spawn_on_terminal(command) {
        Ok(spawned) => spawned,
        Err(error) => {
            tracing::warn!(%error, "Could not run Spotlight");
            return Vec::new();
        }
    };
    // Without its dashes, the name also matches paths that the full name does
    // not. Drop them before the limit counts, or they can push out the paths
    // that match.
    let full_name = (asked.len() != name.len()).then(|| name.to_owned());
    let keep = move |path: &Path| {
        full_name
            .as_deref()
            .is_none_or(|full| name_contains(path, full))
    };
    let paths = read_paths(output, limit, std::time::Duration::from_secs(1), keep);
    // Spotlight may still be writing after `limit` paths; stop it either way.
    child.kill().ok();
    child.wait().ok();
    paths
}

/// Whether the last part of `path` contains `name`, ignoring case.
fn name_contains(path: &Path, name: &str) -> bool {
    path.file_name().is_some_and(|file| {
        file.to_string_lossy()
            .to_lowercase()
            .contains(&name.to_lowercase())
    })
}

/// Runs `command` with its output on a pseudo-terminal and returns the
/// terminal's reading end. Into a pipe, mdfind writes only in 16 KB blocks
/// or at its end, so a search that takes more than a second gave no paths;
/// onto a terminal, it writes each path as it finds it.
fn spawn_on_terminal(
    mut command: Command,
) -> std::io::Result<(std::process::Child, std::fs::File)> {
    use std::os::fd::{FromRawFd, OwnedFd};
    let (mut reader, mut writer) = (-1, -1);
    // SAFETY: An all-zero termios is a valid value for cfmakeraw to fill.
    let mut raw: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: Every pointer is valid for the call or null, which openpty
    // allows for the name and window size. Raw mode keeps "\n" as it is.
    let opened = unsafe {
        libc::cfmakeraw(&mut raw);
        libc::openpty(
            &mut reader,
            &mut writer,
            std::ptr::null_mut(),
            &mut raw,
            std::ptr::null_mut(),
        )
    };
    if opened != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: openpty succeeded, so both are open descriptors that nothing
    // else owns.
    let (reader, writer) = unsafe { (OwnedFd::from_raw_fd(reader), OwnedFd::from_raw_fd(writer)) };
    for fd in [&reader, &writer] {
        use std::os::fd::AsRawFd;
        // SAFETY: The descriptor is open. Close-on-exec keeps it out of other
        // children, which would hold the terminal open after `command` ends.
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    let child = command
        .stdin(std::process::Stdio::null())
        .stdout(writer)
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // `command` held the writing end; dropping it lets reads end with the child.
    drop(command);
    Ok((child, std::fs::File::from(reader)))
}

/// Up to `limit` paths, one per line, that `output` gives before `timeout`
/// and `keep` accepts. The reader sends each path as it reads it, so a slow
/// search still keeps the paths it found before the timeout.
fn read_paths(
    output: impl std::io::Read + Send + 'static,
    limit: usize,
    timeout: std::time::Duration,
    keep: impl Fn(&Path) -> bool + Send + 'static,
) -> Vec<PathBuf> {
    use std::io::BufRead;
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for path in std::io::BufReader::new(output)
            .lines()
            .map_while(std::io::Result::ok)
            .map(PathBuf::from)
            .filter(|path| keep(path))
            .take(limit)
        {
            if send.send(path).is_err() {
                break;
            }
        }
    });
    let deadline = std::time::Instant::now() + timeout;
    let mut paths = Vec::new();
    while paths.len() < limit {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match receive.recv_timeout(left) {
            Ok(path) => paths.push(path),
            Err(_) => break,
        }
    }
    paths
}

pub fn run_system_command(command: SystemCommand) -> Result<()> {
    match command {
        SystemCommand::Lock => lock_screen(),
        SystemCommand::Sleep => super::run("/usr/bin/pmset", &["sleepnow"]),
        SystemCommand::Restart => apple_script(r#"tell application "System Events" to restart"#),
        SystemCommand::ShutDown => apple_script(r#"tell application "System Events" to shut down"#),
        SystemCommand::LogOut => apple_script(r#"tell application "System Events" to log out"#),
        SystemCommand::EmptyTrash => apple_script(
            "tell application \"Finder\"\nif (count of items of trash) > 0 then empty trash\nend tell",
        ),
        SystemCommand::OpenSystemSettings => {
            super::run("/usr/bin/open", &["-b", "com.apple.systempreferences"])
        }
        SystemCommand::SleepDisplays => {
            super::wait_for_key_release();
            super::run("/usr/bin/pmset", &["displaysleepnow"])
        }
        SystemCommand::ToggleDarkMode => apple_script(
            r#"tell application "System Events" to tell appearance preferences to set dark mode to not dark mode"#,
        ),
        // Opening the folder needs no Automation permission, unlike asking Finder.
        SystemCommand::OpenTrash => {
            let trash = std::env::home_dir()
                .ok_or_else(|| Error::msg("Could not find your home folder."))?
                .join(".Trash");
            super::run("/usr/bin/open", &[&trash.to_string_lossy()])
        }
    }
}

fn apple_script(script: &str) -> Result<()> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let error = String::from_utf8_lossy(&output.stderr);
    Err(Error::msg(if error.contains("(-1743)") {
        "macOS blocked the command. Allow TinyDash in System Settings > Privacy & Security > Automation.".to_owned()
    } else if error.contains("(-128)") {
        "The command was canceled.".to_owned()
    } else {
        format!("macOS could not run the command: {}", error.trim())
    }))
}

/// Lock with the login framework call that the system menu uses. It needs
/// no Accessibility or Automation permission.
fn lock_screen() -> Result<()> {
    let unavailable = || Error::msg("Lock Screen is unavailable. Press Control-Command-Q instead.");
    // SAFETY: dlopen and dlsym receive valid C strings. The symbol has the C
    // signature `int SACLockScreenImmediate(void)` on every supported macOS.
    unsafe {
        let framework = libc::dlopen(
            c"/System/Library/PrivateFrameworks/login.framework/Versions/Current/login".as_ptr(),
            libc::RTLD_LAZY,
        );
        if framework.is_null() {
            return Err(unavailable());
        }
        let symbol = libc::dlsym(framework, c"SACLockScreenImmediate".as_ptr());
        if symbol.is_null() {
            return Err(unavailable());
        }
        let lock: extern "C" fn() -> i32 = std::mem::transmute(symbol);
        lock();
    }
    Ok(())
}

pub fn clipboard_change() -> u64 {
    let _one_reader = super::one_clipboard_reader();
    NSPasteboard::generalPasteboard().changeCount() as u64
}

/// The latest change made while a password app was in front, as
/// `note_clipboard_change` saw it, or `u64::MAX` for none.
static PASSWORD_COPY: AtomicU64 = AtomicU64::new(u64::MAX);

fn password_app_in_front() -> bool {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .and_then(|app| app.bundleIdentifier())
            .is_some_and(|id| PASSWORD_APPS.contains(&id.to_string().as_str()))
    })
}

/// The change counter and whether a password app was in front, at one look
/// by `note_clipboard_change`.
type Look = (u64, bool);

/// No look yet: any first change counts as new.
const NO_LOOK: Look = (u64::MAX, false);

static LAST_LOOK: Mutex<Look> = Mutex::new(NO_LOOK);

/// Whether a change seen now may come from a password app, and the look to
/// keep for next time. The monitor looks only twice a second, so a copy made
/// just before the user switched away shows up with another app in front; a
/// password app in front at the look before counts too.
fn look(last: Look, change: u64, password_app_now_in_front: bool) -> (bool, Look) {
    let (last_change, password_app_was_in_front) = last;
    let secret = change != last_change && (password_app_was_in_front || password_app_now_in_front);
    (secret, (change, password_app_now_in_front))
}

/// Remember a change made in a password app, which marks nothing, so its
/// copy stays secret after the user leaves the app. Reads no content.
pub fn note_clipboard_change(change: u64) {
    note_change(change);
}

/// Look at `change` and say whether a password app is in front now.
fn note_change(change: u64) -> bool {
    let now_front = password_app_in_front();
    let mut last = LAST_LOOK.lock().unwrap_or_else(|e| e.into_inner());
    let (secret, next) = look(*last, change, now_front);
    if secret {
        PASSWORD_COPY.store(change, Ordering::Release);
    }
    *last = next;
    now_front
}

/// Call with the reader lock held; it reads the change counter directly.
/// It looks at the counter too: a read can come before the monitor's next
/// tick, just after the user copied in a password app and switched away.
fn clipboard_is_concealed() -> bool {
    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        let marked = pasteboard.types().is_some_and(|types| {
            types
                .iter()
                .any(|kind| SECRET_FORMATS.contains(&kind.to_string().as_str()))
        });
        let change = pasteboard.changeCount() as u64;
        let password_app_now_in_front = note_change(change);
        marked || PASSWORD_COPY.load(Ordering::Acquire) == change || password_app_now_in_front
    })
}

/// The clipboard content for the latest change, or `None` when its source
/// marked it secret.
pub fn read_clipboard(images: bool, files: bool) -> Option<Content> {
    let _one_reader = super::one_clipboard_reader();
    let change = || NSPasteboard::generalPasteboard().changeCount();
    let before = change();
    if clipboard_is_concealed() {
        return None;
    }
    // A copy that replaced the checked one during the read was not checked.
    super::read_with_arboard(images, files).filter(|_| change() == before)
}

/// Reads the TIFF that AppKit offers once, checks its size from the header,
/// then decodes those same bytes. arboard would fetch them a second time.
pub fn clipboard_image() -> Option<image::RgbaImage> {
    autoreleasepool(|_| {
        // SAFETY: AppKit sets the type constant up before any code runs.
        let tiff = unsafe { NSPasteboardTypeTIFF };
        let data = NSPasteboard::generalPasteboard().dataForType(tiff)?;
        // SAFETY: Nothing changes the data while the slice is read.
        decode_tiff(unsafe { data.as_bytes_unchecked() })
    })
}

fn decode_tiff(bytes: &[u8]) -> Option<image::RgbaImage> {
    let reader =
        || image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Tiff);
    let (width, height) = reader().into_dimensions().ok()?;
    if !pixels_fit(width, height) {
        return None;
    }
    Some(reader().decode().ok()?.into_rgba8())
}

pub fn clipboard_text(_app: &tauri::AppHandle) -> Option<String> {
    match read_clipboard(false, false)? {
        Content::Text(text) => Some(text),
        Content::Image { .. } | Content::Files(_) => None,
    }
}

/// Marks a copy so clipboard managers skip it (`org.nspasteboard.ConcealedType`).
pub fn exclude_from_history(set: arboard::Set<'_>) -> arboard::Set<'_> {
    arboard::SetExtApple::exclude_from_history(set)
}

/// macOS needs no change notifications: `clipboard_change` is a cheap counter.
pub fn watch_clipboard(_capturing: impl Fn() -> bool + 'static) {}

/// Move the launcher to `saved` when the middle of its top edge would be on
/// a screen, so it can still be dragged; else center it on the screen with
/// the pointer. Everything here is in points.
///
/// Tauri reports positions in physical pixels, but on macOS each value uses a
/// different scale: the cursor uses the primary display's, each monitor its
/// own. Converting everything to points makes mixed-DPI setups agree.
pub fn place_launcher(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    saved: Option<LauncherPosition>,
) -> tauri::Result<()> {
    let monitors = app.available_monitors()?;
    let size = window
        .outer_size()?
        .to_logical::<f64>(window.scale_factor()?);
    if let Some(saved) = saved
        && monitor_at(&monitors, saved.x + size.width / 2.0, saved.y + 1.0).is_some()
    {
        return window.set_position(tauri::LogicalPosition::new(saved.x, saved.y));
    }
    let primary_scale = app
        .primary_monitor()?
        .map_or(1.0, |monitor| monitor.scale_factor());
    let cursor = app.cursor_position()?;
    let Some(monitor) = monitor_at(
        &monitors,
        cursor.x / primary_scale,
        cursor.y / primary_scale,
    ) else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let free_width = (f64::from(area.size.width) / scale - size.width).max(0.0);
    let free_height = (f64::from(area.size.height) / scale - size.height).max(0.0);
    // The window has a fixed height and never grows, so it sits in the
    // exact center rather than high up, as launchers that grow do.
    window.set_position(tauri::LogicalPosition::new(
        f64::from(area.position.x) / scale + free_width / 2.0,
        f64::from(area.position.y) / scale + free_height / 2.0,
    ))
}

/// The monitor that holds a point, both in points.
fn monitor_at(monitors: &[tauri::Monitor], x: f64, y: f64) -> Option<&tauri::Monitor> {
    monitors.iter().find(|monitor| {
        let scale = monitor.scale_factor();
        let (position, size) = (monitor.position(), monitor.size());
        let (left, top) = (f64::from(position.x) / scale, f64::from(position.y) / scale);
        let (width, height) = (
            f64::from(size.width) / scale,
            f64::from(size.height) / scale,
        );
        (left..left + width).contains(&x) && (top..top + height).contains(&y)
    })
}

pub fn launcher_position(window: &tauri::WebviewWindow) -> tauri::Result<LauncherPosition> {
    let position = window
        .outer_position()?
        .to_logical::<f64>(window.scale_factor()?);
    Ok(LauncherPosition {
        x: position.x,
        y: position.y,
    })
}

/// Open on the active Space, even over a full-screen app.
pub fn prepare_launcher(window: &tauri::WebviewWindow) {
    let Ok(pointer) = window.ns_window() else {
        return;
    };
    // SAFETY: Tauri returns this window's live NSWindow, and setup runs on
    // the main thread, as AppKit requires.
    let ns_window: &NSWindow = unsafe { &*pointer.cast() };
    ns_window.setCollectionBehavior(
        ns_window.collectionBehavior()
            | NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
}

static PREVIOUS_APP: Mutex<Option<Retained<NSRunningApplication>>> = Mutex::new(None);

pub fn remember_frontmost_app() {
    // A copy made in a password app just before the shortcut may be newer
    // than the clipboard monitor's last look; it must stay secret too. The
    // counter waits for the reader lock, so it is read only then.
    if password_app_in_front() {
        note_clipboard_change(clipboard_change());
    }
    let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return;
    };
    // When TinyDash itself is in front (for example, Settings), Escape should
    // leave focus there, not jump to an app from an earlier session.
    let previous = (app != NSRunningApplication::currentApplication()).then_some(app);
    *PREVIOUS_APP.lock().unwrap_or_else(|e| e.into_inner()) = previous;
}

/// Hand focus back to the app that was in front before the launcher opened.
/// Skipped when the user already switched to another app.
pub fn restore_frontmost_app() {
    let Some(app) = PREVIOUS_APP
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    else {
        return;
    };
    let still_in_front = NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .is_some_and(|front| front == NSRunningApplication::currentApplication());
    if still_in_front && !app.isTerminated() {
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

/// Marks the key events TinyDash types ("TDSH"), so its own tap skips them.
const OWN_EVENTS: i64 = 0x5444_5348;
/// Virtual key codes from `<HIToolbox/Events.h>`: `kVK_Delete` is Backspace.
const BACKSPACE_KEY: u16 = 0x33;
const RETURN_KEY: u16 = 0x24;
/// `CGEventKeyboardSetUnicodeString` types at most 20 UTF-16 units an event.
const MAX_UNITS_PER_EVENT: usize = 20;
/// A pause after each typed key, so a busy app does not drop keys.
const KEY_GAP: Duration = Duration::from_millis(1);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: &'static NSString;
}

pub fn typing_unavailable() -> Option<&'static str> {
    None
}

/// Reading keys needs Input Monitoring, and typing needs Accessibility.
pub fn typing_access(ask: bool) -> Result<()> {
    let accessibility = accessibility_trusted(ask);
    let mut input_monitoring = CGPreflightListenEventAccess();
    if !input_monitoring && ask {
        input_monitoring = CGRequestListenEventAccess();
    }
    missing_access(accessibility, input_monitoring)
        .map_or(Ok(()), |message| Err(Error::msg(message)))
}

fn missing_access(accessibility: bool, input_monitoring: bool) -> Option<String> {
    let missing = match (accessibility, input_monitoring) {
        (true, true) => return None,
        (false, true) => "Accessibility",
        (true, false) => "Input Monitoring",
        (false, false) => "Accessibility and Input Monitoring",
    };
    Some(format!(
        "Allow TinyDash under {missing} in System Settings > Privacy & Security."
    ))
}

/// Whether TinyDash may send keys. `ask` shows the macOS prompt that leads
/// to System Settings when it may not.
fn accessibility_trusted(ask: bool) -> bool {
    autoreleasepool(|_| {
        // SAFETY: The framework defines the key as a constant string that
        // lives for the whole process.
        let prompt = unsafe { kAXTrustedCheckOptionPrompt };
        let options = NSDictionary::from_slices(&[prompt], &[&*NSNumber::new_bool(ask)]);
        // SAFETY: An NSDictionary is a CFDictionary, and it outlives the call.
        unsafe { AXIsProcessTrustedWithOptions(Retained::as_ptr(&options).cast()) }
    })
}

/// Reads keys with an event tap on a thread that runs its own run loop.
pub struct TypingWatch {
    run_loop: TapRunLoop,
    stopped: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// The tap thread's run loop, kept only to stop it.
struct TapRunLoop(CFRetained<CFRunLoop>);

// SAFETY: `CFRunLoopStop` may be called from any thread, and this handle
// is used for nothing else.
unsafe impl Send for TapRunLoop {}

impl TypingWatch {
    /// False once the thread has ended, also on its own when macOS turned
    /// the tap off for good, as it does when access is taken away.
    pub fn is_running(&self) -> bool {
        !self.thread.is_finished()
    }

    pub fn stop(self) {
        self.stopped.store(true, Ordering::Release);
        // A stop that comes before the loop runs is lost, so repeat it
        // until the thread ends.
        while !self.thread.is_finished() {
            self.run_loop.0.stop();
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.thread.join().is_err() {
            tracing::warn!("The key watch ended with a panic");
        }
    }
}

/// What the tap callback works with, on the tap thread only.
struct Tap {
    /// The tap itself, to turn it on again after macOS turns it off.
    port: OnceCell<CFRetained<CFMachPort>>,
    on_key: RefCell<Box<dyn FnMut(Key)>>,
    /// The app that got the latest key; a key in another app starts afresh.
    front_app: RefCell<Option<Retained<NSRunningApplication>>>,
}

pub fn watch_typing(on_key: impl FnMut(Key) + Send + 'static) -> Result<TypingWatch> {
    let (ready, started) = mpsc::channel();
    let stopped = Arc::new(AtomicBool::new(false));
    let thread = std::thread::Builder::new()
        .name("key-watch".into())
        .spawn({
            let stopped = Arc::clone(&stopped);
            move || run_tap(Box::new(on_key), &stopped, &ready)
        })?;
    match started.recv() {
        Ok(Ok(run_loop)) => Ok(TypingWatch {
            run_loop,
            stopped,
            thread,
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err(Error::msg("Could not start reading keys."))
        }
    }
}

fn run_tap(
    on_key: Box<dyn FnMut(Key)>,
    stopped: &AtomicBool,
    ready: &mpsc::Sender<Result<TapRunLoop>>,
) {
    let tap = Box::new(Tap {
        port: OnceCell::new(),
        on_key: RefCell::new(on_key),
        front_app: RefCell::new(None),
    });
    let mask = [
        CGEventType::KeyDown,
        CGEventType::LeftMouseDown,
        CGEventType::RightMouseDown,
        CGEventType::OtherMouseDown,
    ]
    .iter()
    .fold(0, |mask, kind| mask | 1 << kind.0);
    // SAFETY: `on_tap_event` has the callback's type, and `tap` outlives
    // the port, which is invalidated before `tap` drops.
    let port = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            mask,
            Some(on_tap_event),
            std::ptr::from_ref::<Tap>(&tap).cast_mut().cast(),
        )
    };
    let Some(port) = port else {
        let _ = ready.send(Err(Error::msg(
            "macOS did not let TinyDash read keys. Allow TinyDash under Input Monitoring in System Settings > Privacy & Security.",
        )));
        return;
    };
    let source = CFMachPort::new_run_loop_source(None, Some(&port), 0);
    let (Some(source), Some(run_loop)) = (source, CFRunLoop::current()) else {
        port.invalidate();
        let _ = ready.send(Err(Error::msg("Could not start reading keys.")));
        return;
    };
    // SAFETY: CoreFoundation sets the mode constant up before any code runs.
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
    CGEvent::tap_enable(&port, true);
    let port = tap.port.get_or_init(|| port);
    if ready.send(Ok(TapRunLoop(run_loop.clone()))).is_ok() {
        // macOS invalidates the tap when access is taken away; the loop
        // would then return at once, so the thread ends instead.
        while !stopped.load(Ordering::Acquire) && port.is_valid() && source.is_valid() {
            CFRunLoop::run();
        }
        if !stopped.load(Ordering::Acquire) {
            tracing::warn!(
                "Snippets stopped expanding as you type: macOS turned off the key watch, as it does when access is taken away"
            );
        }
    }
    CGEvent::tap_enable(port, false);
    port.invalidate();
    source.invalidate();
}

/// The tap callback. A listen-only tap returns the event unchanged.
unsafe extern "C-unwind" fn on_tap_event(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: NonNull<CGEvent>,
    tap: *mut c_void,
) -> *mut CGEvent {
    // SAFETY: `tap` is the `Tap` that `run_tap` passed; it lives until the
    // port is invalidated, and only the tap thread uses it.
    let tap = unsafe { &*tap.cast::<Tap>() };
    // SAFETY: macOS passes an event that is valid during the call.
    read_tap_event(tap, kind, unsafe { event.as_ref() });
    event.as_ptr()
}

fn read_tap_event(tap: &Tap, kind: CGEventType, event: &CGEvent) {
    let send = |key| {
        if let Ok(mut on_key) = tap.on_key.try_borrow_mut() {
            on_key(key);
        }
    };
    match kind {
        CGEventType::KeyDown => {
            let field = |field| CGEvent::integer_value_field(Some(event), field);
            if field(CGEventField::EventSourceUserData) == OWN_EVENTS {
                return;
            }
            let front_app = frontmost_app();
            if tap.front_app.replace(front_app.clone()) != front_app {
                send(Key::Reset);
            }
            let mut units = [0; 8];
            let mut length: c_ulong = 0;
            // SAFETY: Both pointers are valid, and the buffer holds the
            // length passed.
            unsafe {
                CGEvent::keyboard_get_unicode_string(
                    Some(event),
                    units.len() as c_ulong,
                    &mut length,
                    units.as_mut_ptr(),
                );
            }
            let typed = &units[..units.len().min(length as usize)];
            let keycode = field(CGEventField::KeyboardEventKeycode);
            for key in keys_for(keycode, CGEvent::flags(Some(event)), typed) {
                send(key);
            }
        }
        // macOS turns off a tap that took too long, and may turn it off
        // after some user input; keys missed meanwhile are unknown. A tap
        // that lost its access stays off, and its thread ends.
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            if let Some(port) = tap.port.get() {
                CGEvent::tap_enable(port, true);
            }
            send(Key::Reset);
        }
        // A click.
        _ => send(Key::Reset),
    }
}

fn frontmost_app() -> Option<Retained<NSRunningApplication>> {
    autoreleasepool(|_| NSWorkspace::sharedWorkspace().frontmostApplication())
}

/// The keys for a key press: a reset for a shortcut (Command, Control, or
/// Option held), Backspace, or what the key typed.
fn keys_for(keycode: i64, flags: CGEventFlags, typed: &[u16]) -> Vec<Key> {
    let shortcut =
        CGEventFlags::MaskCommand | CGEventFlags::MaskControl | CGEventFlags::MaskAlternate;
    if flags.intersects(shortcut) {
        vec![Key::Reset]
    } else if keycode == i64::from(BACKSPACE_KEY) {
        vec![Key::Backspace]
    } else {
        expansion::keys_for(typed)
    }
}

/// Types with key events posted at the system level, which reach the app in
/// front. Each carries [`OWN_EVENTS`], so the tap skips it. Typing stops soon
/// after another app comes to the front, so little of the rest lands there.
pub fn type_text(erase: usize, text: &str) -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::Private)
        .ok_or_else(|| Error::msg("Could not type the snippet."))?;
    CGEventSource::set_user_data(Some(&source), OWN_EVENTS);
    // Keys the user presses meanwhile still arrive.
    CGEventSource::set_local_events_suppression_interval(Some(&source), 0.0);
    let front_app = frontmost_app();
    // The key code does not matter once the event carries text.
    let presses = std::iter::repeat_n((BACKSPACE_KEY, Vec::new()), erase).chain(
        expansion::pieces(text, MAX_UNITS_PER_EVENT)
            .into_iter()
            .map(|piece| match piece {
                Piece::Return => (RETURN_KEY, Vec::new()),
                Piece::Text(units) => (0, units),
            }),
    );
    for (keycode, units) in presses {
        if frontmost_app() != front_app {
            return Err(Error::msg(
                "Stopped typing the snippet: another app came to the front.",
            ));
        }
        press(&source, keycode, &units)?;
    }
    Ok(())
}

/// Press and release a key, typing `text` (at most 20 UTF-16 units) when
/// there is any.
fn press(source: &CGEventSource, keycode: u16, text: &[u16]) -> Result<()> {
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(Some(source), keycode, down)
            .ok_or_else(|| Error::msg("Could not type the snippet."))?;
        CGEvent::set_integer_value_field(
            Some(&event),
            CGEventField::EventSourceUserData,
            OWN_EVENTS,
        );
        // Without the modifiers the user may still hold, such as Shift.
        CGEvent::set_flags(Some(&event), CGEventFlags::empty());
        if !text.is_empty() {
            // SAFETY: The pointer and length describe `text`.
            unsafe {
                CGEvent::keyboard_set_unicode_string(
                    Some(&event),
                    text.len() as c_ulong,
                    text.as_ptr(),
                );
            }
        }
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }
    std::thread::sleep(KEY_GAP);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn says_which_access_is_missing() {
        use super::missing_access;
        assert_eq!(missing_access(true, true), None);
        let message = |accessibility, input| missing_access(accessibility, input).unwrap();
        assert_eq!(
            message(false, false),
            "Allow TinyDash under Accessibility and Input Monitoring in System Settings > Privacy & Security."
        );
        assert!(message(false, true).contains("under Accessibility in"));
        assert!(message(true, false).contains("under Input Monitoring in"));
    }

    #[test]
    fn reads_key_presses() {
        use super::{BACKSPACE_KEY, keys_for};
        use crate::features::expansion::Key;
        use objc2_core_graphics::CGEventFlags;
        let none = CGEventFlags::empty();
        let utf16 = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
        let backspace = i64::from(BACKSPACE_KEY);
        // kVK_ANSI_S, without and with Shift.
        assert_eq!(keys_for(1, none, &utf16("s")), [Key::Char('s')]);
        assert_eq!(
            keys_for(1, CGEventFlags::MaskShift, &utf16("S")),
            [Key::Char('S')]
        );
        assert_eq!(keys_for(backspace, none, &utf16("\u{8}")), [Key::Backspace]);
        // Shortcuts, and Option+Backspace, which deletes a word.
        for flags in [
            CGEventFlags::MaskCommand,
            CGEventFlags::MaskControl,
            CGEventFlags::MaskAlternate,
        ] {
            assert_eq!(keys_for(1, flags, &utf16("s")), [Key::Reset]);
            assert_eq!(keys_for(backspace, flags, &[]), [Key::Reset]);
        }
        // Arrows carry the Fn flag on Mac keyboards, and still reset.
        assert_eq!(
            keys_for(0x7B, CGEventFlags::MaskSecondaryFn, &utf16("\u{F702}")),
            [Key::Reset]
        );
        // Return and Escape.
        assert_eq!(keys_for(0x24, none, &utf16("\r")), [Key::Reset]);
        assert_eq!(keys_for(0x35, none, &utf16("\u{1b}")), [Key::Reset]);
    }

    #[test]
    fn decodes_a_tiff_that_fits() {
        let mut tiff = Vec::new();
        image::RgbaImage::new(3, 2)
            .write_to(
                &mut std::io::Cursor::new(&mut tiff),
                image::ImageFormat::Tiff,
            )
            .unwrap();
        let image = super::decode_tiff(&tiff).unwrap();
        assert_eq!(image.dimensions(), (3, 2));
        assert!(super::decode_tiff(&tiff[..4]).is_none());
        assert!(super::decode_tiff(b"not an image").is_none());
    }

    #[test]
    fn describes_known_apps_then_by_category() {
        use super::describe;
        assert_eq!(
            describe(
                Some("com.apple.Safari"),
                Some("public.app-category.productivity")
            ),
            Some("Web browser")
        );
        assert_eq!(
            describe(
                Some("eu.exelban.Stats"),
                Some("public.app-category.utilities")
            ),
            Some("Utility")
        );
        assert_eq!(
            describe(
                Some("com.apple.Chess"),
                Some("public.app-category.board-games")
            ),
            Some("Game")
        );
        assert_eq!(
            describe(Some("com.example"), Some("public.app-category.new-kind")),
            None
        );
        assert_eq!(describe(None, None), None);
    }

    use super::*;

    #[test]
    fn finds_system_apps_with_icons() {
        let apps = discover_apps();
        let finder = apps
            .iter()
            .find(|app| app.name == "Finder")
            .expect("Finder");
        let png = app_icon(Path::new(&finder.path), 64).expect("icon");
        assert_eq!(&png[1..4], b"PNG");
        assert!(!apps.iter().any(|app| app.path.contains("/Contents/")));
    }

    /// Gives `text`, then blocks like a search that is still running, until
    /// the test drops the sender of `release`.
    struct SlowOutput {
        text: std::io::Cursor<Vec<u8>>,
        release: std::sync::mpsc::Receiver<()>,
    }

    impl std::io::Read for SlowOutput {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let read = self.text.read(buffer)?;
            if read == 0 {
                self.release.recv().ok();
            }
            Ok(read)
        }
    }

    #[test]
    fn keeps_paths_read_before_the_timeout() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let output = SlowOutput {
            text: std::io::Cursor::new(b"/a\n/b\n".to_vec()),
            release: wait,
        };
        let paths = read_paths(output, 10, std::time::Duration::from_millis(200), |_| true);
        drop(release);
        assert_eq!(paths, [PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn spawned_output_is_a_terminal_with_plain_lines() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("test -t 1 && printf '/a\\n/b\\n'");
        let (mut child, output) = spawn_on_terminal(command).unwrap();
        let paths = read_paths(output, 10, std::time::Duration::from_secs(5), |_| true);
        child.wait().ok();
        assert_eq!(paths, [PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn a_name_with_leading_dashes_keeps_them() {
        assert!(name_contains(Path::new("/home/-Dash.txt"), "-dash"));
        assert!(name_contains(Path::new("/home/x--dash.txt"), "--dash"));
        assert!(!name_contains(Path::new("/home/dash.txt"), "-dash"));
        assert!(!name_contains(Path::new("/-dash/notes.txt"), "-dash"));
    }

    /// Runs Spotlight for real: `TINYDASH_LIVE_SPOTLIGHT=1 cargo test live_spotlight`.
    #[test]
    fn live_spotlight_takes_a_name_that_starts_with_a_dash() {
        if std::env::var_os("TINYDASH_LIVE_SPOTLIGHT").is_none() {
            return;
        }
        let home = std::env::home_dir().expect("a home folder");
        let folder = home.join(format!("Downloads/tinydash-dash-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("-tinydash-dash.txt");
        std::fs::write(&file, "").unwrap();
        let mut found = Vec::new();
        for _ in 0..30 {
            found = find_files("-tinydash-dash", &folder, 10);
            if !found.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(found, [file]);
        assert!(find_files("--", &home, 10).is_empty());
    }

    #[test]
    fn reads_at_most_the_limit() {
        let output = std::io::Cursor::new(b"/a\n/b\n/c\n".to_vec());
        let paths = read_paths(output, 2, std::time::Duration::from_secs(1), |_| true);
        assert_eq!(paths, [PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn the_limit_counts_only_the_paths_kept() {
        let output = std::io::Cursor::new(b"/draft.txt\n/Drafts\n/-draft.txt\n".to_vec());
        let keep = |path: &Path| name_contains(path, "-draft");
        let paths = read_paths(output, 2, std::time::Duration::from_secs(1), keep);
        assert_eq!(paths, [PathBuf::from("/-draft.txt")]);
    }

    #[test]
    fn finds_apps_in_core_services() {
        let apps = discover_apps();
        assert!(apps.iter().any(|app| app.name == "Archive Utility"));
    }

    #[test]
    fn a_copy_seen_after_leaving_a_password_app_is_secret() {
        // Passwords in front, no new copy yet.
        let (secret, last) = look((7, false), 7, true);
        assert!(!secret);
        // The user copied, then switched to Safari before the next look.
        let (secret, last) = look(last, 8, false);
        assert!(secret);
        // Safari stays in front; nothing new.
        let (secret, last) = look(last, 8, false);
        assert!(!secret);
        // A copy made in Safari is not secret.
        let (secret, _) = look(last, 9, false);
        assert!(!secret);
    }

    #[test]
    fn a_copy_made_in_a_password_app_is_secret() {
        let (secret, last) = look((7, false), 8, true);
        assert!(secret);
        // The same copy seen again is not new.
        assert!(!look(last, 8, true).0);
    }

    #[test]
    fn the_first_look_judges_only_the_app_in_front_now() {
        assert!(!look(NO_LOOK, 3, false).0);
        assert!(look(NO_LOOK, 3, true).0);
    }

    #[test]
    fn missing_paths_still_get_a_generic_icon_or_none() {
        // AppKit returns a generic icon for unknown paths; either way, no panic.
        let _ = app_icon(Path::new("/nonexistent/TinyDash.app"), 32);
    }
}
