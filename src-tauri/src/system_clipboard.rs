//! Reading and writing the OS clipboard through arboard.

use std::{borrow::Cow, io::Cursor, sync::Mutex};

use arboard::{Clipboard, ImageData};

use crate::{
    error::{Error, Result},
    features::clipboard::{Content, MAX_IMAGE_BYTES, MAX_IMAGE_PIXELS},
};

/// One long-lived handle for writes. On X11 the owner must stay alive to
/// serve pasted data, so the handle is never dropped.
static WRITER: Mutex<Option<Clipboard>> = Mutex::new(None);

fn with_writer<T>(write: impl FnOnce(&mut Clipboard) -> Result<T>) -> Result<T> {
    let mut writer = WRITER.lock().unwrap_or_else(|e| e.into_inner());
    if writer.is_none() {
        *writer = Some(Clipboard::new()?);
    }
    write(writer.as_mut().expect("created above"))
}

/// Write text. `secret` adds the markers that tell clipboard managers,
/// including TinyDash, to skip it.
pub fn write_text(text: &str, secret: bool) -> Result<()> {
    with_writer(|clipboard| {
        let set = clipboard.set();
        if !secret {
            return Ok(set.text(text)?);
        }
        #[cfg(target_os = "macos")]
        let set = arboard::SetExtApple::exclude_from_history(set);
        #[cfg(target_os = "windows")]
        let set = arboard::SetExtWindows::exclude_from_cloud(
            arboard::SetExtWindows::exclude_from_history(set),
        );
        #[cfg(target_os = "linux")]
        let set = arboard::SetExtLinux::exclude_from_history(set);
        Ok(set.text(text)?)
    })
}

/// Restore saved content in its original format.
pub fn write(content: &Content) -> Result<()> {
    match content {
        Content::Text(text) => write_text(text, false),
        Content::Files(paths) => {
            if let Some(missing) = paths.iter().find(|path| !path.exists()) {
                return Err(Error::msg(format!(
                    "{} no longer exists.",
                    missing.display()
                )));
            }
            with_writer(|clipboard| Ok(clipboard.set().file_list(paths)?))
        }
        Content::Image { png, .. } => {
            let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
                .map_err(|error| Error::msg(format!("The saved image is damaged: {error}")))?
                .into_rgba8();
            let image = ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: Cow::Owned(image.into_raw()),
            };
            with_writer(|clipboard| Ok(clipboard.set_image(image)?))
        }
    }
}

pub fn read_text() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}

/// Read what the clipboard holds, in the order apps expect: copied files
/// first (Finder also offers their names as text), then text, then images.
/// Kinds the user did not opt into are skipped, not saved as text.
pub fn read(reader: &mut Clipboard, images: bool, files: bool) -> Option<Content> {
    if let Ok(paths) = reader.get().file_list()
        && !paths.is_empty()
    {
        return files.then_some(Content::Files(paths));
    }
    if let Ok(text) = reader.get_text() {
        return Some(Content::Text(text));
    }
    if !images {
        return None;
    }
    let image = reader.get_image().ok()?;
    let (width, height) = (
        u32::try_from(image.width).ok()?,
        u32::try_from(image.height).ok()?,
    );
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return None;
    }
    let buffer = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())?;
    let mut png = Vec::new();
    buffer
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    (png.len() <= MAX_IMAGE_BYTES).then_some(Content::Image { png, width, height })
}
