use std::{path::Path, ptr};

use objc2::{AllocAnyThread, rc::autoreleasepool};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImageAlphaInfo,
    CGInterpolationQuality,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

/// Resolve the same icon Finder uses, including asset-catalog and custom icons.
/// The icon store calls this outside the search lock, with at most two jobs.
pub fn application_icon(path: &Path, pixels: u16) -> Option<Vec<u8>> {
    if !(16..=256).contains(&pixels) || !path.is_dir() {
        return None;
    }
    let size = usize::from(pixels);
    autoreleasepool(|_| {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path.to_str()?));
        let mut rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(size as f64, size as f64),
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
                size,
                size,
                8,
                size * 4,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
            )
        }?;
        CGContext::set_interpolation_quality(Some(&context), CGInterpolationQuality::High);
        let target = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(size as f64, size as f64),
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
        Some(png.to_vec())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_app_has_no_icon() {
        assert!(
            super::application_icon(std::path::Path::new("/missing/TinyDash-test.app"), 72)
                .is_none()
        );
    }
}
