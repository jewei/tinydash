//! Image protocols for the webview, so images load like normal URLs and
//! never pass through IPC as base64.
//!
//! - `icon://localhost/<encoded path>?size=<px>`: a file's system icon.
//! - `clip://localhost/<id>`: a saved clipboard image.
//!
//! The frontend builds these with `convertFileSrc`; Windows uses the
//! `http://<scheme>.localhost` form of the same URLs.

use std::{
    borrow::Cow,
    collections::HashMap,
    path::Path,
    sync::{Arc, LazyLock, Mutex},
    time::SystemTime,
};

use tauri::{
    Manager, UriSchemeContext, UriSchemeResponder, Wry,
    http::{Request, Response, StatusCode, header},
};

use crate::{features::clipboard::Content, platform, state::State};

/// Icons rendered so far, keyed by path, pixel size, and the path's
/// modification time. An update that replaces the whole app bundle changes
/// that time, so the new icon renders. An update that writes inside the
/// bundle does not, but AppKit keeps the old icon for it anyway, even
/// after a restart or a LaunchServices registration. Small PNGs; capped.
type IconCache = HashMap<(String, u32, Option<SystemTime>), Arc<Vec<u8>>>;
static ICONS: LazyLock<Mutex<IconCache>> = LazyLock::new(Mutex::default);
const ICON_CACHE_LIMIT: usize = 512;

pub fn serve_icon(
    _: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some(path) = decoded_path(&request) else {
        return responder.respond(not_found());
    };
    let size = request
        .uri()
        .query()
        .and_then(|query| query.strip_prefix("size="))
        .and_then(|size| size.parse::<u32>().ok())
        .unwrap_or(64)
        .clamp(16, 256);
    tauri::async_runtime::spawn_blocking(move || {
        let png = cached_icon(&ICONS, path, size, platform::app_icon);
        responder.respond(match png {
            Some(png) => image_response(Vec::clone(&png)),
            None => not_found(),
        });
    });
}

/// The icon of `path` from `cache`, rendered with `render` when the cache
/// has none for the file as it is now.
fn cached_icon(
    cache: &Mutex<IconCache>,
    path: String,
    size: u32,
    render: impl FnOnce(&Path, u32) -> Option<Vec<u8>>,
) -> Option<Arc<Vec<u8>>> {
    let modified = std::fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .ok();
    let key = (path, size, modified);
    let cached = lock(cache).get(&key).cloned();
    cached.or_else(|| {
        let png = Arc::new(render(Path::new(&key.0), size)?);
        let mut icons = lock(cache);
        if icons.len() >= ICON_CACHE_LIMIT {
            icons.clear();
        }
        icons.insert(key, png.clone());
        Some(png)
    })
}

pub fn serve_clipboard_image(
    context: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some(id) = decoded_path(&request).and_then(|id| id.parse::<i64>().ok()) else {
        return responder.respond(not_found());
    };
    let app = context.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = match app.state::<State>().clip(id) {
            Ok(Some(Content::Image { png, .. })) => clip_response(png),
            _ => not_found(),
        };
        responder.respond(response);
    });
}

fn lock(cache: &Mutex<IconCache>) -> std::sync::MutexGuard<'_, IconCache> {
    cache.lock().unwrap_or_else(|e| e.into_inner())
}

fn decoded_path(request: &Request<Vec<u8>>) -> Option<String> {
    let raw = request.uri().path().strip_prefix('/')?;
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    (!decoded.is_empty()).then(|| Cow::into_owned(decoded))
}

/// An icon: the webview asks again each time, because the URL stays the
/// same when an app update changes the icon. The icon cache makes that cheap.
fn image_response(png: Vec<u8>) -> Response<Vec<u8>> {
    png_response(png, "no-cache")
}

/// A copied image, which may be private: the webview must not keep it on
/// disk, where it would outlive a deleted entry or cleared history.
fn clip_response(png: Vec<u8>) -> Response<Vec<u8>> {
    png_response(png, "no-store")
}

fn png_response(png: Vec<u8>, cache: &'static str) -> Response<Vec<u8>> {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, cache)
        .body(png)
        .expect("static headers are valid")
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("static headers are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_images_are_never_kept_and_icons_are_checked_again() {
        let cache = |response: Response<Vec<u8>>| {
            response.headers()[header::CACHE_CONTROL]
                .to_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(cache(clip_response(vec![1])), "no-store");
        assert_eq!(cache(image_response(vec![1])), "no-cache");
    }

    #[test]
    fn an_icon_renders_again_after_its_file_changes() {
        let path = std::env::temp_dir().join(format!("tinydash-icon-{}", std::process::id()));
        std::fs::write(&path, "old").unwrap();
        let cache = Mutex::default();
        let renders = std::cell::Cell::new(0u8);
        let icon = || {
            cached_icon(&cache, path.to_string_lossy().into_owned(), 64, |_, _| {
                renders.set(renders.get() + 1);
                Some(vec![renders.get()])
            })
            .unwrap()
        };

        assert_eq!(*icon(), vec![1]);
        assert_eq!(*icon(), vec![1], "the second request comes from the cache");

        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() + std::time::Duration::from_secs(60))
            .unwrap();
        assert_eq!(*icon(), vec![2], "the changed file renders again");

        std::fs::remove_file(&path).unwrap();
    }
}
