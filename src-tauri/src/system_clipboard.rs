//! Reading and writing the OS clipboard through arboard.

use std::{borrow::Cow, sync::Mutex};

use arboard::{Clipboard, ImageData};

use crate::{
    error::{Error, Result},
    features::clipboard::Content,
    platform,
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
        let set = if secret {
            platform::exclude_from_history(set)
        } else {
            set
        };
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
