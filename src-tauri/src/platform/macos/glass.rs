use objc2::{MainThreadMarker, MainThreadOnly, runtime::AnyClass};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSAutoresizingMaskOptions, NSGlassEffectView, NSGlassEffectViewStyle, NSView, NSWindow,
};

use crate::launcher::window::LauncherAppearance;

pub fn set_launcher_appearance(
    webview: tauri::webview::PlatformWebview,
    appearance: LauncherAppearance,
) -> Result<bool, String> {
    let mtm = MainThreadMarker::new().ok_or("Window appearance requires the main thread.")?;
    // The app still supports macOS 12. Resolve the class before using any API
    // introduced in macOS 26, and keep the opaque frontend on older systems.
    if AnyClass::get(c"NSGlassEffectView").is_none() {
        return Ok(false);
    }
    // Tauri supplies live AppKit objects and runs this callback on the main thread.
    let (window, view) = unsafe {
        (
            &*webview.ns_window().cast::<NSWindow>(),
            &*webview.inner().cast::<NSView>(),
        )
    };
    let content = window
        .contentView()
        .ok_or("Launcher content is unavailable.")?;
    // Clip every native child to the same outline as the web launcher. The
    // glass corner radius alone leaves native drawing outside that outline.
    content.setWantsLayer(true);
    let layer = content.layer().ok_or("Launcher layer is unavailable.")?;
    layer.setCornerRadius(20.0);
    layer.setMasksToBounds(true);
    let glass = content
        .subviews()
        .iter()
        .find_map(|child| child.downcast::<NSGlassEffectView>().ok())
        .unwrap_or_else(|| {
            let glass =
                NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(mtm), view.frame());
            glass.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            // Match --radius-lg. Keep the window and Tauri's event view intact.
            glass.setCornerRadius(20.0);
            glass.setStyle(NSGlassEffectViewStyle::Regular);
            // Use contentView, not a sibling overlay: AppKit then places the
            // web content above the glass and preserves its input handling.
            glass.setContentView(Some(view));
            content.addSubview(&glass);
            glass
        });
    let name = match appearance {
        LauncherAppearance::Dark => unsafe { NSAppearanceNameDarkAqua },
        LauncherAppearance::Light
        | LauncherAppearance::Sage
        | LauncherAppearance::Rose
        | LauncherAppearance::Ink => unsafe { NSAppearanceNameAqua },
    };
    let appearance =
        NSAppearance::appearanceNamed(name).ok_or("Window appearance is unavailable.")?;
    glass.setAppearance(Some(&appearance));
    // Leave tint and opacity to AppKit so the system Liquid Glass control and
    // accessibility settings remain authoritative.
    Ok(true)
}
