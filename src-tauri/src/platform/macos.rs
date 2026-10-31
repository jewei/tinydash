use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use block2::RcBlock;
use objc2::{
    AllocAnyThread, msg_send,
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_app_kit::{
    NSApplicationActivationOptions, NSBitmapImageFileType, NSBitmapImageRep,
    NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImageInterpolation,
    NSPasteboard, NSPasteboardTypeTIFF, NSRunningApplication, NSWindow, NSWindowCollectionBehavior,
    NSWorkspace,
};
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendarItem, EKEntityType, EKEvent, EKEventStatus, EKEventStore,
    EKParticipantStatus, EKReminder,
};
use objc2_foundation::{
    NSArray, NSBundle, NSCalendar, NSCalendarIdentifierGregorian, NSDate, NSDateComponentUndefined,
    NSDateComponents, NSDictionary, NSNumber, NSPoint, NSRect, NSSize, NSString, NSURL,
    NSURLVolumeAvailableCapacityForImportantUsageKey, NSURLVolumeLocalizedNameKey,
    NSURLVolumeTotalCapacityKey,
};

use super::SECRET_FORMATS;
use crate::{
    error::{Error, Result},
    features::{
        apps::App,
        clipboard::{Content, pixels_fit},
        system::SystemCommand,
        up_next::{self, Agenda, Due, Unreadable, UpNextKind},
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
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|error| {
        Error::msg(format!(
            "Could not open the app: {error}. Check that it is still installed."
        ))
    })
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
    use objc2_foundation::NSError;
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
    let id = format!("tinydash-{}", chrono::Utc::now().timestamp_millis());
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

/// The usage strings macOS needs before it asks for calendars and
/// reminders: the full-access ones from macOS 14, the others before.
const CALENDAR_USAGE_KEYS: [&str; 4] = [
    "NSCalendarsFullAccessUsageDescription",
    "NSCalendarsUsageDescription",
    "NSRemindersFullAccessUsageDescription",
    "NSRemindersUsageDescription",
];
/// Reminders come back on a queue of EventKit's; stop waiting after this.
const REMINDERS_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
/// Set while a request for access waits for the user.
static ASKING_FOR_CALENDARS: AtomicBool = AtomicBool::new(false);
/// The store that reads Up Next. Apple advises keeping one, since a store
/// is slow to make and to release.
static EVENT_STORE: Mutex<Option<KeptStore>> = Mutex::new(None);

struct KeptStore {
    store: Retained<EKEventStore>,
    /// Whether events and reminders could be read when the store was made.
    /// A store made before the user allowed access keeps seeing nothing,
    /// so a new answer needs a new store.
    readable: [bool; 2],
}

// SAFETY: A thread may use a store that another thread made, as Apple's
// guide does to fetch events off the main thread, and the mutex lets one
// read at a time use this one.
unsafe impl Send for KeptStore {}

pub fn read_up_next(from_ms: i64, to_ms: i64) -> Option<Agenda> {
    let own = is_own_responsible_process();
    let events = calendar_access(EKEntityType::Event, own);
    let reminders = calendar_access(EKEntityType::Reminder, own);
    if let (Err(events), Err(reminders)) = (&events, &reminders) {
        return Some(Agenda {
            events: Err(events.clone()),
            reminders: Err(reminders.clone()),
        });
    }
    let readable = [events.is_ok(), reminders.is_ok()];
    autoreleasepool(|_| {
        // Held until the read ends, so one read at a time uses the store.
        let mut kept = EVENT_STORE.lock().unwrap_or_else(|e| e.into_inner());
        let store = match &mut *kept {
            Some(current) if current.readable == readable => current.store.clone(),
            slot => {
                // SAFETY: `new` takes no arguments and makes an empty store.
                let store = unsafe { EKEventStore::new() };
                *slot = Some(KeptStore {
                    store: store.clone(),
                    readable,
                });
                store
            }
        };
        let to = ns_date(to_ms);
        Some(Agenda {
            events: events.map(|()| read_events(&store, &ns_date(from_ms), &to)),
            reminders: reminders.and_then(|()| read_reminders(&store, &to)),
        })
    })
}

pub fn ask_calendar_access(done: impl Fn() + Send + Sync + 'static) {
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    let own = is_own_responsible_process();
    let kinds: Vec<EKEntityType> = [EKEntityType::Event, EKEntityType::Reminder]
        .into_iter()
        .filter(|&kind| calendar_access(kind, own) == Err(Unreadable::NotAsked))
        .collect();
    if kinds.is_empty() || ASKING_FOR_CALENDARS.swap(true, Ordering::AcqRel) {
        return;
    }
    // SAFETY: `new` takes no arguments and makes an empty store.
    let store = unsafe { EKEventStore::new() };
    let waiting = Arc::new(AtomicUsize::new(kinds.len()));
    let done = Arc::new(done);
    for kind in kinds {
        let (waiting, done, kept) = (Arc::clone(&waiting), Arc::clone(&done), store.clone());
        // EventKit calls this once, on a queue of its own. The store only
        // has to live until then; the handler never uses it, and releasing
        // an object is safe on any thread.
        let answered = RcBlock::new(move |_granted: Bool, _error: *mut NSError| {
            let _store = &kept;
            if waiting.fetch_sub(1, Ordering::AcqRel) == 1 {
                ASKING_FOR_CALENDARS.store(false, Ordering::Release);
                done();
            }
        });
        let handler = RcBlock::as_ptr(&answered);
        // SAFETY: `handler` points to a live block of the expected type,
        // which EventKit copies. Each request method exists on the macOS
        // versions it is called on.
        unsafe {
            if objc2::available!(macos = 14.0) {
                if kind == EKEntityType::Reminder {
                    store.requestFullAccessToRemindersWithCompletion(handler);
                } else {
                    store.requestFullAccessToEventsWithCompletion(handler);
                }
            } else {
                #[allow(deprecated)]
                store.requestAccessToEntityType_completion(kind, handler);
            }
        }
    }
}

pub fn open_calendar_app(kind: UpNextKind) -> Result<()> {
    let app = match kind {
        UpNextKind::Event => "com.apple.iCal",
        UpNextKind::Reminder => "com.apple.reminders",
    };
    super::run("/usr/bin/open", &["-b", app])
}

/// Whether TinyDash may read one kind, from the answer macOS keeps. Never
/// asks. `own_responsible_process` comes from `is_own_responsible_process`.
fn calendar_access(
    kind: EKEntityType,
    own_responsible_process: bool,
) -> std::result::Result<(), Unreadable> {
    if !own_responsible_process {
        // The saved answer would be the other app's, so it is not read.
        return Err(Unreadable::OpenedByAnotherApp);
    }
    // SAFETY: A class method that reads the saved answer for a valid kind.
    match unsafe { EKEventStore::authorizationStatusForEntityType(kind) } {
        EKAuthorizationStatus::FullAccess => Ok(()),
        EKAuthorizationStatus::NotDetermined
            if has_calendar_usage_strings(&NSBundle::mainBundle()) =>
        {
            Err(Unreadable::NotAsked)
        }
        EKAuthorizationStatus::NotDetermined => Err(Unreadable::CannotAsk),
        EKAuthorizationStatus::Restricted => Err(Unreadable::Restricted),
        // Denied, or Add Events Only, which cannot read.
        _ => Err(Unreadable::Denied),
    }
}

/// `pid_t responsibility_get_pid_responsible_for_pid(pid_t)`, a private
/// libSystem function: the process that macOS holds responsible for a
/// process, or -1 when it cannot tell.
type ResponsibleFor = extern "C" fn(libc::pid_t) -> libc::pid_t;

/// Whether TinyDash is its own responsible process. macOS checks the access
/// and the usage strings of the responsible process, not of the process
/// that reads or asks, and a program that a terminal or another app starts
/// belongs to that app. An app that launchd starts is its own: from Finder,
/// Launchpad, the Dock, or `open`. So are the login item, a LaunchAgent
/// that runs the binary, and the restart after an update, which Tauri runs
/// through `/usr/bin/open -n`. The answer cannot change while TinyDash
/// runs, so it is looked up once.
fn is_own_responsible_process() -> bool {
    static OWN: OnceLock<bool> = OnceLock::new();
    *OWN.get_or_init(|| {
        let pid = std::process::id() as libc::pid_t;
        responsible_for_itself(find_responsible_for(), pid)
    })
}

/// The private function, where this macOS has it.
fn find_responsible_for() -> Option<ResponsibleFor> {
    // SAFETY: dlsym receives a valid C string, and returns null when no
    // loaded library defines the symbol.
    let symbol = unsafe {
        libc::dlsym(
            libc::RTLD_DEFAULT,
            c"responsibility_get_pid_responsible_for_pid".as_ptr(),
        )
    };
    // SAFETY: libSystem defines the symbol with the C signature of
    // `ResponsibleFor` on every supported macOS. It is safe to call with any
    // process ID, because it only reads what the kernel keeps about it.
    (!symbol.is_null())
        .then(|| unsafe { std::mem::transmute::<*mut libc::c_void, ResponsibleFor>(symbol) })
}

/// Whether `responsible_for` names `pid` as its own responsible process.
/// Without the function, TinyDash cannot tell, so it takes the safe answer.
fn responsible_for_itself(responsible_for: Option<ResponsibleFor>, pid: libc::pid_t) -> bool {
    responsible_for.is_some_and(|responsible_for| responsible_for(pid) == pid)
}

/// Whether the app carries the usage strings. macOS ends its own responsible
/// process when it asks without them. The app bundle has them, and `tauri
/// dev` embeds them in the binary; a build made with `--no-bundle` does not.
fn has_calendar_usage_strings(bundle: &NSBundle) -> bool {
    CALENDAR_USAGE_KEYS.iter().all(|key| {
        bundle
            .objectForInfoDictionaryKey(&NSString::from_str(key))
            .is_some()
    })
}

fn ns_date(unix_ms: i64) -> Retained<NSDate> {
    NSDate::dateWithTimeIntervalSince1970(unix_ms as f64 / 1000.0)
}

fn unix_ms(date: &NSDate) -> i64 {
    // Rounded, as seconds in a double miss some milliseconds by a hair.
    // `as` saturates; real dates fit by far.
    (date.timeIntervalSince1970() * 1000.0).round() as i64
}

fn read_events(store: &EKEventStore, from: &NSDate, to: &NSDate) -> Vec<up_next::Event> {
    // SAFETY: The predicate comes from this store, as the search requires.
    // The search may return nil, though its header says otherwise.
    let found: Option<Retained<NSArray<EKEvent>>> = unsafe {
        let predicate = store.predicateForEventsWithStartDate_endDate_calendars(from, to, None);
        msg_send![store, eventsMatchingPredicate: &*predicate]
    };
    found
        .iter()
        .flat_map(|found| found.iter())
        .take(up_next::MAX_READ)
        .filter_map(|event| read_event(&event))
        .collect()
}

fn read_event(event: &EKEvent) -> Option<up_next::Event> {
    // SAFETY: Getters of an event from the store. The title and the dates
    // may be nil, though the headers do not promise them, so they are read
    // as optional.
    let (title, start, end, all_day) = unsafe {
        let title: Option<Retained<NSString>> = msg_send![event, title];
        let start: Option<Retained<NSDate>> = msg_send![event, startDate];
        let end: Option<Retained<NSDate>> = msg_send![event, endDate];
        (title, start?, end?, event.isAllDay())
    };
    Some(up_next::Event {
        title: title.map(|title| title.to_string()).unwrap_or_default(),
        calendar: calendar_name(event),
        start_ms: unix_ms(&start),
        end_ms: unix_ms(&end),
        all_day,
        declined: canceled_or_declined(event),
    })
}

/// Whether the organizer canceled the event, or the user declined it.
/// EventKit returns both, and Calendar hides them.
fn canceled_or_declined(event: &EKEvent) -> bool {
    // SAFETY: Getters of an event from the store and of its attendees.
    unsafe {
        event.status() == EKEventStatus::Canceled
            || event.attendees().is_some_and(|attendees| {
                attendees.iter().any(|attendee| {
                    attendee.isCurrentUser()
                        && attendee.participantStatus() == EKParticipantStatus::Declined
                })
            })
    }
}

/// Read incomplete reminders due before `to`, overdue ones included.
fn read_reminders(
    store: &EKEventStore,
    to: &NSDate,
) -> std::result::Result<Vec<up_next::Reminder>, Unreadable> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // EventKit calls this once, on a queue of its own.
    let handler = RcBlock::new(move |found: *mut NSArray<EKReminder>| {
        // SAFETY: EventKit passes null or an array that lives for this call.
        let found = unsafe { found.as_ref() };
        let reminders: Vec<up_next::Reminder> = found
            .iter()
            .flat_map(|found| found.iter())
            .take(up_next::MAX_READ)
            .filter_map(|reminder| read_reminder(&reminder))
            .collect();
        // The reader may have stopped waiting.
        send.try_send(reminders).ok();
    });
    // SAFETY: The predicate comes from this store, as the fetch requires,
    // and EventKit copies the handler. The fetch's identifier may be nil,
    // though its header says otherwise.
    let fetch: Option<Retained<AnyObject>> = unsafe {
        let predicate = store.predicateForIncompleteRemindersWithDueDateStarting_ending_calendars(
            None,
            Some(to),
            None,
        );
        msg_send![store, fetchRemindersMatchingPredicate: &*predicate, completion: &*handler]
    };
    receive.recv_timeout(REMINDERS_WAIT).map_err(|_| {
        if let Some(fetch) = fetch {
            // SAFETY: `fetch` is what this store returned for the fetch.
            unsafe { store.cancelFetchRequest(&fetch) };
        }
        Unreadable::Failed("Reminders did not answer in time. Open the launcher again.".into())
    })
}

fn read_reminder(reminder: &EKReminder) -> Option<up_next::Reminder> {
    // SAFETY: Getters of a reminder from the store. The title may be nil,
    // though the header does not promise it, so it is read as optional.
    let (title, due) = unsafe {
        let title: Option<Retained<NSString>> = msg_send![reminder, title];
        (title, reminder.dueDateComponents()?)
    };
    Some(up_next::Reminder {
        title: title.map(|title| title.to_string()).unwrap_or_default(),
        list: calendar_name(reminder),
        due: due_date(&due)?,
    })
}

/// When a reminder is due. EventKit keeps due dates in the Gregorian
/// calendar, whichever calendar the user chose in the region settings.
fn due_date(due: &NSDateComponents) -> Option<Due> {
    if due.hour() == NSDateComponentUndefined {
        return Some(Due::Day(chrono::NaiveDate::from_ymd_opt(
            i32::try_from(due.year()).ok()?,
            u32::try_from(due.month()).ok()?,
            u32::try_from(due.day()).ok()?,
        )?));
    }
    // SAFETY: A constant that Foundation defines.
    let gregorian = NSCalendar::calendarWithIdentifier(unsafe { NSCalendarIdentifierGregorian })?;
    // The calendar uses the components' own time zone when they have one,
    // and the system's otherwise.
    let date = gregorian.dateFromComponents(due)?;
    Some(Due::At(unix_ms(&date)))
}

/// The name of the calendar or reminder list that holds `item`.
fn calendar_name(item: &EKCalendarItem) -> String {
    // SAFETY: Getters of an item from the store and of its calendar, whose
    // title is never nil.
    unsafe { item.calendar().map(|calendar| calendar.title().to_string()) }.unwrap_or_default()
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

/// Write the command to a script that only the user can read, and open it
/// with the app for `.command` files: Terminal, or the one the user chose,
/// such as iTerm. No Apple Events, so no permission prompt.
pub fn run_in_terminal(command: &str, data: &Path) -> Result<()> {
    let script = super::terminal::command_file(data, &super::terminal::macos_script(command))
        .map_err(|error| {
            Error::msg(format!(
                "TinyDash could not prepare the command for Terminal: {error}"
            ))
        })?;
    let opened = super::run("/usr/bin/open", &[&script.to_string_lossy()]);
    if opened.is_err() {
        std::fs::remove_file(&script).ok();
    }
    opened
}

fn apple_script(script: &str) -> Result<()> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .map_err(|error| {
            Error::msg(format!(
                "macOS could not run the command: {error}. Try again."
            ))
        })?;
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
    let _one_user = super::one_clipboard_user();
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

/// Call with the clipboard lock held; it reads the change counter directly.
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
    let _one_user = super::one_clipboard_user();
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
    // counter waits for the clipboard lock, so it is read only then.
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_script_runs_the_command_deletes_itself_and_opens_the_login_shell() {
        let data = std::env::temp_dir().join(format!("tinydash-script-{}", std::process::id()));
        std::fs::remove_dir_all(&data).ok();
        let home = data.join("home");
        std::fs::create_dir_all(&home).unwrap();
        // `echo` stands in for the login shell and prints `-l` where it
        // would open. The app for `.command` files can start the script in
        // any folder, such as `/`.
        let run = |command: &str| {
            let script = super::super::terminal::command_file(
                &data,
                &super::super::terminal::macos_script(command),
            )
            .unwrap();
            let output = std::process::Command::new(&script)
                .current_dir("/")
                // zsh reads `.zshenv` in `ZDOTDIR`, else in `HOME`: here, none.
                .env("HOME", &home)
                .env_remove("ZDOTDIR")
                .env("SHELL", "/bin/echo")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(!script.exists(), "{command}");
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(run("echo {clipboard} \"it's\""), "{clipboard} it's\n-l\n");
        // The command starts in the home folder, and can leave it.
        assert_eq!(run("pwd"), format!("{}\n-l\n", home.display()));
        assert_eq!(run("cd / && pwd"), "/\n-l\n");
        // Exit, a syntax error, or a line that runs on still leave a shell.
        assert_eq!(run("exit 3"), "-l\n");
        assert_eq!(run("echo it's"), "-l\n");
        assert_eq!(run("echo a \\"), "a\n-l\n");
        std::fs::remove_dir_all(&data).unwrap();
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
    fn dates_keep_their_milliseconds() {
        for unix_ms in [0, 1_772_000_000_123, -86_400_000] {
            assert_eq!(super::unix_ms(&ns_date(unix_ms)), unix_ms);
        }
    }

    #[test]
    fn asks_for_calendars_only_with_every_usage_string() {
        let folder = std::env::temp_dir().join(format!("tinydash-usage-{}", std::process::id()));
        // TinyDash's own Info.plist, then one that lacks a key. NSBundle
        // keeps a bundle per path, so each case has its own.
        let shipped =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Info.plist"))
                .unwrap();
        let cases = [
            ("Shipped.app", shipped.clone(), true),
            (
                "Partial.app",
                shipped.replace("NSRemindersFullAccessUsageDescription", "Unrelated"),
                false,
            ),
        ];
        for (name, info, can_ask) in cases {
            let contents = folder.join(name).join("Contents");
            std::fs::create_dir_all(&contents).unwrap();
            std::fs::write(contents.join("Info.plist"), info).unwrap();
            let path = NSString::from_str(&folder.join(name).to_string_lossy());
            let bundle = NSBundle::bundleWithPath(&path).expect("a bundle");
            assert_eq!(has_calendar_usage_strings(&bundle), can_ask, "{name}");
        }
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn reads_and_asks_for_calendars_only_as_its_own_responsible_process() {
        extern "C" fn itself(pid: libc::pid_t) -> libc::pid_t {
            pid
        }
        extern "C" fn terminal(_pid: libc::pid_t) -> libc::pid_t {
            4242
        }
        extern "C" fn unknown(_pid: libc::pid_t) -> libc::pid_t {
            -1
        }
        assert!(responsible_for_itself(Some(itself), 1234));
        assert!(!responsible_for_itself(Some(terminal), 1234));
        assert!(!responsible_for_itself(Some(unknown), 1234));
        assert!(!responsible_for_itself(None, 1234));

        // Started by another app, TinyDash does not even read the saved
        // answer, and so never asks either.
        for kind in [EKEntityType::Event, EKEntityType::Reminder] {
            assert_eq!(
                calendar_access(kind, false),
                Err(Unreadable::OpenedByAnotherApp)
            );
        }
    }

    #[test]
    fn finds_the_process_responsible_for_tinydash() {
        let responsible_for = find_responsible_for().expect("the private function");
        let pid = std::process::id() as libc::pid_t;
        // This process, or the app that started the tests.
        assert!(responsible_for(pid) > 0);
    }

    #[test]
    fn reads_due_dates_in_the_gregorian_calendar() {
        let due = NSDateComponents::new();
        due.setYear(2026);
        due.setMonth(10);
        due.setDay(30);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        assert_eq!(due_date(&due), Some(Due::Day(day)));

        // Without a time zone of its own, a due time is local.
        due.setHour(16);
        due.setMinute(30);
        let local = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2026, 10, 30, 16, 30, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(due_date(&due), Some(Due::At(local)));
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
