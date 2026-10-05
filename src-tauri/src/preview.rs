//! Details for the preview pane, loaded when a result is selected.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    error::Result,
    features::{clipboard::Content, library::LibraryKind},
    search::id::Source,
    state::State,
};

#[derive(Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Preview {
    Text {
        text: String,
    },
    /// Load the pixels from `clip://localhost/<id>`.
    Image {
        id: i64,
        width: u32,
        height: u32,
    },
    Files {
        paths: Vec<String>,
    },
    File {
        path: String,
        size: u64,
        modified: Option<i64>,
        is_dir: bool,
    },
}

/// The preview for a result ID. Files and apps must be indexed, so the
/// webview cannot probe arbitrary paths.
pub fn load(state: &State, id: &str) -> Result<Option<Preview>> {
    let Some((source, key)) = Source::parse(id) else {
        return Ok(None);
    };
    Ok(match source {
        Source::Clip => {
            let Ok(id) = key.parse() else { return Ok(None) };
            state.clip(id)?.map(|content| match content {
                Content::Text(text) => Preview::Text { text },
                Content::Image { width, height, .. } => Preview::Image { id, width, height },
                Content::Files(paths) => Preview::Files {
                    paths: paths.iter().map(|p| p.display().to_string()).collect(),
                },
            })
        }
        Source::App if state.apps.get().contains(key) => file(key),
        Source::File if state.files.get().contains(key) => file(key),
        Source::Snippet => key
            .parse()
            .ok()
            .and_then(|id| state.library.get().find(id).cloned())
            .filter(|item| item.kind == LibraryKind::Snippet)
            .map(|item| Preview::Text { text: item.text }),
        _ => None,
    })
}

fn file(path: &str) -> Option<Preview> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    Some(Preview::File {
        path: path.into(),
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|age| i64::try_from(age.as_secs()).ok()),
        is_dir: metadata.is_dir(),
    })
}
