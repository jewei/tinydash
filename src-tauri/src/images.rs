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
};

use tauri::{
    Manager, UriSchemeContext, UriSchemeResponder, Wry,
    http::{Request, Response, StatusCode, header},
};

use crate::{features::clipboard::Content, platform, state::State};

/// Icons rendered so far, keyed by path and pixel size. Small PNGs; capped.
type IconCache = HashMap<(String, u32), Arc<Vec<u8>>>;
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
        let key = (path, size);
        let cached = icons().get(&key).cloned();
        let png = cached.or_else(|| {
            let png = Arc::new(platform::app_icon(Path::new(&key.0), size)?);
            let mut icons = icons();
            if icons.len() >= ICON_CACHE_LIMIT {
                icons.clear();
            }
            icons.insert(key, png.clone());
            Some(png)
        });
        responder.respond(match png {
            Some(png) => image_response(Vec::clone(&png)),
            None => not_found(),
        });
    });
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

fn icons() -> std::sync::MutexGuard<'static, IconCache> {
    ICONS.lock().unwrap_or_else(|e| e.into_inner())
}

fn decoded_path(request: &Request<Vec<u8>>) -> Option<String> {
    let raw = request.uri().path().strip_prefix('/')?;
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    (!decoded.is_empty()).then(|| Cow::into_owned(decoded))
}

/// An icon: the same for an hour, so the webview may keep it.
fn image_response(png: Vec<u8>) -> Response<Vec<u8>> {
    png_response(png, "max-age=3600")
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
    fn copied_images_are_never_cached_but_icons_are() {
        let cache = |response: Response<Vec<u8>>| {
            response.headers()[header::CACHE_CONTROL]
                .to_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(cache(clip_response(vec![1])), "no-store");
        assert_eq!(cache(image_response(vec![1])), "max-age=3600");
    }
}
