use super::*;
use std::{
    ffi::{CString, c_void},
    sync::{Arc, Mutex},
};

type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(allocator: Ref, text: *const i8, encoding: u32) -> Ref;
    fn CFRelease(value: Ref);
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> Ref;
    fn AXUIElementCreateApplication(pid: i32) -> Ref;
    fn AXUIElementSetMessagingTimeout(element: Ref, seconds: f32) -> i32;
    fn CGPreflightPostEventAccess() -> bool;
    fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
    fn AXUIElementSetAttributeValue(element: Ref, attribute: Ref, value: Ref) -> i32;
    fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    fn AXValueGetValue(value: Ref, kind: u32, output: *mut c_void) -> bool;
    fn AXValueCreate(kind: u32, value: *const c_void) -> Ref;
    fn CGEventPost(tap: u32, event: Ref);
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    width: f64,
    height: f64,
}
pub(crate) struct Cf(pub Ref);
// These retained immutable CF/AX references may move between workers. Window mutation is
// serialized by WindowTarget's mutex; no AppKit view objects are retained here.
unsafe impl Send for Cf {}
impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
impl Cf {
    pub(crate) fn string(text: &str) -> UtilityResult<Self> {
        let text = CString::new(text).map_err(|e| e.to_string())?;
        let ptr = unsafe { CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), 0x08000100) };
        if ptr.is_null() {
            Err("Could not create native string".into())
        } else {
            Ok(Self(ptr))
        }
    }
    fn attribute(&self, name: &str) -> UtilityResult<Self> {
        let key = Self::string(name)?;
        let mut result = std::ptr::null();
        let code = unsafe { AXUIElementCopyAttributeValue(self.0, key.0, &mut result) };
        if code != 0 || result.is_null() {
            Err(format!(
                "Accessibility {name} failed ({code}). Allow TinyDash in Privacy & Security > Accessibility; this window may not support positioning."
            ))
        } else {
            Ok(Self(result))
        }
    }
    fn set(&self, name: &str, value: &Cf) -> UtilityResult<()> {
        let key = Self::string(name)?;
        let code = unsafe { AXUIElementSetAttributeValue(self.0, key.0, value.0) };
        if code == 0 {
            Ok(())
        } else {
            Err(format!("Window rejected {name} ({code})"))
        }
    }
}
#[derive(Clone)]
pub struct WindowTarget {
    element: Arc<Mutex<Cf>>,
    identity: super::super::process::ProcessInfo,
    original: Rect,
}
impl WindowTarget {
    pub fn label(&self) -> String {
        format!("{} (PID {})", self.identity.name, self.identity.pid)
    }
}

pub fn capture_window() -> UtilityResult<WindowTarget> {
    effects_allowed()?;
    let system = Cf(unsafe { AXUIElementCreateSystemWide() });
    if system.0.is_null() {
        return Err("Accessibility unavailable".into());
    }
    let app = system.attribute("AXFocusedApplication")?;
    let mut pid = 0;
    if unsafe { AXUIElementGetPid(app.0, &mut pid) } != 0 {
        return Err("Cannot identify focused application".into());
    }
    capture_target(pid as u64)
}

pub fn capture_target(id: u64) -> UtilityResult<WindowTarget> {
    effects_allowed()?;
    let pid = i32::try_from(id).map_err(|_| "Invalid application identity")?;
    let app = Cf(unsafe { AXUIElementCreateApplication(pid) });
    if app.0.is_null() {
        return Err("Accessibility unavailable".into());
    }
    let identity = super::super::process::list()?
        .into_iter()
        .find(|p| p.pid == pid as u32)
        .ok_or("Focus a non-system application other than TinyDash, then capture it")?;
    unsafe {
        AXUIElementSetMessagingTimeout(app.0, 1.0);
    }
    let element = app
        .attribute("AXFocusedWindow")
        .or_else(|_| app.attribute("AXMainWindow"))?;
    unsafe {
        AXUIElementSetMessagingTimeout(element.0, 1.0);
    }
    let position = element.attribute("AXPosition")?;
    let size = element.attribute("AXSize")?;
    let mut point = Point::default();
    let mut dimensions = Size::default();
    if !unsafe { AXValueGetValue(position.0, 1, (&mut point as *mut Point).cast()) }
        || !unsafe { AXValueGetValue(size.0, 2, (&mut dimensions as *mut Size).cast()) }
    {
        return Err("Cannot read window bounds".into());
    }
    Ok(WindowTarget {
        element: Arc::new(Mutex::new(element)),
        identity,
        original: Rect {
            x: point.x as i32,
            y: point.y as i32,
            width: dimensions.width as i32,
            height: dimensions.height as i32,
        },
    })
}

// AppKit uses logical points with an upward Y axis; Accessibility uses the
// same points with Y measured down from the primary display's top edge.
// Do not multiply by backingScaleFactor: mixed-Retina layouts need global points.
fn accessibility_rect(frame: objc2_foundation::NSRect, primary_top: f64) -> Rect {
    Rect {
        x: frame.origin.x.round() as i32,
        y: (primary_top - frame.origin.y - frame.size.height).round() as i32,
        width: frame.size.width.round() as i32,
        height: frame.size.height.round() as i32,
    }
}

fn overlap(a: Rect, b: Rect) -> i64 {
    let width = (i64::from(a.x) + i64::from(a.width)).min(i64::from(b.x) + i64::from(b.width))
        - i64::from(a.x.max(b.x));
    let height = (i64::from(a.y) + i64::from(a.height)).min(i64::from(b.y) + i64::from(b.height))
        - i64::from(a.y.max(b.y));
    width.max(0) * height.max(0)
}

fn work_area(app: &tauri::AppHandle, original: Rect) -> UtilityResult<Rect> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = (|| {
            let main =
                objc2::MainThreadMarker::new().ok_or("Display lookup needs the main thread")?;
            let screens = objc2_app_kit::NSScreen::screens(main);
            let primary = screens.iter().next().ok_or("No displays are available")?;
            let frame = primary.frame();
            let primary_top = frame.origin.y + frame.size.height;
            screens
                .iter()
                .map(|screen| {
                    (
                        accessibility_rect(screen.frame(), primary_top),
                        accessibility_rect(screen.visibleFrame(), primary_top),
                    )
                })
                .filter(|(_, area)| area.width > 0 && area.height > 0)
                .max_by_key(|(frame, _)| overlap(original, *frame))
                .map(|(_, area)| area)
                .ok_or_else(|| "No usable display work area is available".into())
        })();
        let _ = send.send(result);
    })
    .map_err(|error| error.to_string())?;
    receive
        .recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "Display work area lookup timed out".to_string())?
}

pub fn window(
    app: &tauri::AppHandle,
    target: &WindowTarget,
    action: WindowAction,
) -> UtilityResult<()> {
    effects_allowed()?;
    super::super::process::validate(&target.identity)?;
    let original = target.original;
    // Restore must still work if a display lookup fails. Other placements use
    // the current visible frame, which excludes the Dock and menu bar.
    let area = if matches!(action, WindowAction::Restore) {
        original
    } else {
        work_area(app, original)?
    };
    let rect = super::placement(original, area, action);
    let point = Point {
        x: rect.x as f64,
        y: rect.y as f64,
    };
    let size = Size {
        width: rect.width as f64,
        height: rect.height as f64,
    };
    let position = Cf(unsafe { AXValueCreate(1, (&point as *const Point).cast()) });
    let dimensions = Cf(unsafe { AXValueCreate(2, (&size as *const Size).cast()) });
    if position.0.is_null() || dimensions.0.is_null() {
        return Err("Cannot create window bounds".into());
    }
    let element = target
        .element
        .lock()
        .map_err(|_| "Window target unavailable")?;
    element.set("AXSize", &dimensions)?;
    element.set("AXPosition", &position)
}

/// AppKit retains sampler and completion block until click/Escape. Only one session may
/// exist. Waiting is bounded; a timed-out OS sampler must still be dismissed with Escape.
pub fn eyedropper(app: tauri::AppHandle) -> UtilityResult<Option<super::super::color::Color>> {
    use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    static SAMPLING: AtomicBool = AtomicBool::new(false);
    effects_allowed()?;
    if SAMPLING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("A color sampler is already open. Select a pixel or press Escape.".into());
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    if let Err(error) = app.run_on_main_thread(move || {
        let completion = block2::RcBlock::new(move |selected: *mut NSColor| {
            let result = if let Some(selected) = unsafe { selected.as_ref() } {
                selected
                    .colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())
                    .ok_or("Cannot convert sampled color to sRGB".into())
                    .and_then(|rgb| {
                        let channel = |n: f64| (n.clamp(0.0, 1.0) * 255.0).round() as u8;
                        super::super::color::parse(&format!(
                            "#{:02X}{:02X}{:02X}{:02X}",
                            channel(rgb.redComponent()),
                            channel(rgb.greenComponent()),
                            channel(rgb.blueComponent()),
                            channel(rgb.alphaComponent())
                        ))
                        .map(Some)
                    })
            } else {
                Ok(None)
            };
            SAMPLING.store(false, Ordering::Release);
            let _ = sender.send(result);
        });
        let sampler = NSColorSampler::new();
        unsafe {
            sampler.showSamplerWithSelectionHandler(&completion);
        }
    }) {
        SAMPLING.store(false, Ordering::Release);
        return Err(error.to_string());
    }
    receiver
        .recv_timeout(std::time::Duration::from_secs(120))
        .map_err(|_| {
            "Color sampling timed out. Press Escape to dismiss the system sampler.".to_string()
        })?
}

pub fn media(action: MediaAction) -> UtilityResult<()> {
    effects_allowed()?;
    if !unsafe { CGPreflightPostEventAccess() } {
        return Err(
            "Allow TinyDash in Privacy & Security > Accessibility to send media keys".into(),
        );
    }
    let key: i64 = match action {
        MediaAction::PlayPause => 16,
        MediaAction::Next => 17,
        MediaAction::Previous => 18,
        MediaAction::VolumeUp => 0,
        MediaAction::VolumeDown => 1,
        MediaAction::Mute => 7,
    };
    objc2::rc::autoreleasepool(|_| unsafe {
        for state in [0xau64, 0xbu64] {
            let event: *mut objc2::runtime::AnyObject = objc2::msg_send![objc2::class!(NSEvent), otherEventWithType:14usize, location:objc2_foundation::NSPoint::new(0.0,0.0), modifierFlags:0usize, timestamp:0.0f64, windowNumber:0isize, context:std::ptr::null_mut::<objc2::runtime::AnyObject>(), subtype:8i16, data1:((key<<16)|((state as i64)<<8)) as isize, data2:-1isize];
            if event.is_null() {
                return Err("Could not create media key event".into());
            }
            let cg: Ref = objc2::msg_send![event, CGEvent];
            if cg.is_null() {
                return Err("Could not create media key event".into());
            }
            CGEventPost(0, cg);
        }
        Ok(())
    })
}

#[cfg(test)]
mod work_area_tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    #[test]
    fn visible_frame_reserves_dock_and_menu_bar_in_logical_points() {
        let area = accessibility_rect(
            NSRect::new(NSPoint::new(0.0, 70.0), NSSize::new(1440.0, 805.0)),
            900.0,
        );
        assert_eq!(
            (area.x, area.y, area.width, area.height),
            (0, 25, 1440, 805)
        );
        let half = super::super::placement(
            Rect {
                x: 80,
                y: 100,
                width: 600,
                height: 400,
            },
            area,
            WindowAction::Left,
        );
        assert_eq!((half.x, half.y, half.width, half.height), (0, 25, 720, 805));
    }

    #[test]
    fn global_coordinates_handle_displays_above_and_left_without_pixel_scaling() {
        let above = accessibility_rect(
            NSRect::new(NSPoint::new(-1200.0, 900.0), NSSize::new(1200.0, 800.0)),
            900.0,
        );
        assert_eq!((above.x, above.y), (-1200, -800));
        let left = accessibility_rect(
            NSRect::new(NSPoint::new(-1920.0, -180.0), NSSize::new(1920.0, 1080.0)),
            900.0,
        );
        assert_eq!((left.x, left.y, left.width), (-1920, 0, 1920));
        assert_eq!(overlap(above, left), 0);
        assert_eq!(overlap(left, left), 1920 * 1080);
    }
}
