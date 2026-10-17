//! Details for the preview pane, loaded when a result is selected.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    actions::offered_file,
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

/// The preview for a result ID. Apps must be indexed and files offered, so
/// the webview cannot probe arbitrary paths.
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
        Source::File if offered_file(state, key) => file(key),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        features::library::{Library, LibraryItem},
        settings::Settings,
    };

    #[test]
    fn previews_only_offered_items() {
        let state = State::for_tests(Settings::default());
        let path = std::env::temp_dir().join(format!("tinydash-preview-{}", std::process::id()));
        std::fs::write(&path, "hello").unwrap();
        let path = path.display().to_string();
        let id = Source::File.id(&path);

        assert!(load(&state, &id).unwrap().is_none());
        state.found.lock().unwrap().remember(&path);
        assert!(matches!(
            load(&state, &id).unwrap(),
            Some(Preview::File {
                size: 5,
                is_dir: false,
                ..
            })
        ));

        state.library.set(Library::new(vec![LibraryItem {
            id: Some(7),
            kind: LibraryKind::Quicklink,
            name: "Docs".into(),
            keyword: String::new(),
            text: "https://example.com".into(),
        }]));
        assert!(load(&state, "snippet:7").unwrap().is_none());
        assert!(load(&state, "clip:abc").unwrap().is_none());
        assert!(load(&state, "nothing:abc").unwrap().is_none());
        std::fs::remove_file(path).unwrap();
    }
}
