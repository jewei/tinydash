use std::{path::Path, ptr};

use objc2::{AllocAnyThread, rc::autoreleasepool};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImageAlphaInfo,
    CGInterpolationQuality,
};
use objc2_foundation::{
    NSDataBase64EncodingOptions, NSDictionary, NSPoint, NSRect, NSSize, NSString,
};

const ICON_SIZE: usize = 192;

/// Resolve the same icon Finder uses, including asset-catalog and custom icons.
/// Called by the background application scan. Searches reuse the encoded image.
pub fn application_icon(path: &Path) -> Option<String> {
    if !path.is_dir() {
        return None;
    }
    autoreleasepool(|_| {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path.to_str()?));
        let mut rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(ICON_SIZE as f64, ICON_SIZE as f64),
        );
        // SAFETY: The proposed rectangle is valid for this call. There are no hints
        // or borrowed graphics contexts, and the returned image is retained.
        let source = unsafe { image.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
        let space = CGColorSpace::new_device_rgb()?;
        // SAFETY: A null buffer asks CoreGraphics to allocate and own the bitmap.
        // The RGB format has four 8-bit components and a matching row stride.
        let context = unsafe {
            CGBitmapContextCreate(
                ptr::null_mut(),
                ICON_SIZE,
                ICON_SIZE,
                8,
                ICON_SIZE * 4,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }?;
        CGContext::set_interpolation_quality(Some(&context), CGInterpolationQuality::High);
        let target = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(ICON_SIZE as f64, ICON_SIZE as f64),
        );
        CGContext::draw_image(Some(&context), target, Some(&source));
        let scaled = CGBitmapContextCreateImage(Some(&context))?;
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &scaled);
        // SAFETY: An empty, correctly typed dictionary requests default PNG encoding.
        let png = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        let encoded = png.base64EncodedStringWithOptions(NSDataBase64EncodingOptions::empty());
        Some(format!("data:image/png;base64,{encoded}"))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_app_has_no_icon() {
        assert!(
            super::application_icon(std::path::Path::new("/missing/TinyDash-test.app")).is_none()
        );
    }
}
