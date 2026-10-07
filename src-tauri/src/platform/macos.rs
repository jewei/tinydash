use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use objc2::{
    AllocAnyThread,
    rc::{Retained, autoreleasepool},
};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSBitmapImageFileType, NSBitmapImageRep,
    NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImageInterpolation,
    NSPasteboard, NSRunningApplication, NSWindow, NSWindowCollectionBehavior, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString, NSURL,
    NSURLVolumeAvailableCapacityForImportantUsageKey, NSURLVolumeLocalizedNameKey,
    NSURLVolumeTotalCapacityKey,
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{apps::App, clipboard::Content, system::SystemCommand},
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
    })
}

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
    NSPasteboard::generalPasteboard().changeCount() as u64
}

fn clipboard_is_concealed() -> bool {
    autoreleasepool(|_| {
        let marked = NSPasteboard::generalPasteboard()
            .types()
            .is_some_and(|types| {
                types
                    .iter()
                    .any(|kind| SECRET_FORMATS.contains(&kind.to_string().as_str()))
            });
        marked
            || NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .and_then(|app| app.bundleIdentifier())
                .is_some_and(|id| PASSWORD_APPS.contains(&id.to_string().as_str()))
    })
}

/// The clipboard content for the latest change, or `None` when its source
/// marked it secret.
pub fn read_clipboard(images: bool, files: bool) -> Option<Content> {
    if clipboard_is_concealed() {
        return None;
    }
    super::read_with_arboard(images, files)
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

#[cfg(test)]
mod tests {
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

    #[test]
    fn missing_paths_still_get_a_generic_icon_or_none() {
        // AppKit returns a generic icon for unknown paths; either way, no panic.
        let _ = app_icon(Path::new("/nonexistent/TinyDash.app"), 32);
    }
}
