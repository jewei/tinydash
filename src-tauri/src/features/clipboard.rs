//! Clipboard history: what is kept and how it is found. Capture lives in
//! `monitor.rs`; storage lives in `store.rs`.

use std::path::PathBuf;

use crate::search::{
    Context,
    id::Source,
    matcher::{Matcher, normalize},
    result::{Action, Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
};

/// Longer text is not saved. Very large copies are usually data, not snippets.
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
/// Larger images are not saved.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;
/// Most file references in one entry.
pub const MAX_FILES: usize = 64;
/// Most unpinned images kept, whatever the history limit.
pub const MAX_IMAGES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipKind {
    Text,
    Image,
    Files,
}

/// What the clipboard held at capture time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    Text(String),
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// References to existing files. Their contents are never copied.
    Files(Vec<PathBuf>),
}

impl Content {
    pub fn kind(&self) -> ClipKind {
        match self {
            Self::Text(_) => ClipKind::Text,
            Self::Image { .. } => ClipKind::Image,
            Self::Files(_) => ClipKind::Files,
        }
    }

    /// Whether this content may be saved at all.
    pub fn fits(&self) -> bool {
        match self {
            Self::Text(text) => {
                !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES && !text.contains('\0')
            }
            Self::Image { png, width, height } => {
                png.len() <= MAX_IMAGE_BYTES
                    && u64::from(*width) * u64::from(*height) <= MAX_IMAGE_PIXELS
            }
            Self::Files(paths) => {
                !paths.is_empty()
                    && paths.len() <= MAX_FILES
                    && paths.iter().all(|p| p.is_absolute())
            }
        }
    }

    /// Stable 64-bit FNV-1a hash of the content, used to find repeats.
    pub fn hash(&self) -> i64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for byte in bytes {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        match self {
            Self::Text(text) => {
                feed(b"text");
                feed(text.as_bytes());
            }
            Self::Image { png, .. } => {
                feed(b"image");
                feed(png);
            }
            Self::Files(paths) => {
                feed(b"files");
                for path in paths {
                    feed(path.to_string_lossy().as_bytes());
                    feed(b"\0");
                }
            }
        }
        hash as i64
    }

    /// One line that names the entry in the list.
    pub fn title(&self) -> String {
        match self {
            Self::Text(text) => {
                let line = text
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("");
                line.trim().chars().take(200).collect()
            }
            Self::Image { width, height, .. } => format!("Image {width} × {height}"),
            Self::Files(paths) => {
                let name = |path: &PathBuf| {
                    path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into(),
                    )
                };
                match paths.as_slice() {
                    [one] => name(one),
                    [first, rest @ ..] => format!("{} and {} more", name(first), rest.len()),
                    [] => String::new(),
                }
            }
        }
    }
}

/// One saved entry, without image bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub kind: ClipKind,
    pub title: String,
    /// Title and content (text, or file paths) in the form matching compares.
    haystack: String,
    /// Unix seconds of the last copy.
    pub copied_at: i64,
}

impl Entry {
    /// `searchable` is the text or the file paths, not the stored form.
    pub fn new(id: i64, kind: ClipKind, title: String, searchable: &str, copied_at: i64) -> Self {
        Self {
            haystack: normalize(&format!("{title}\n{searchable}")),
            id,
            kind,
            title,
            copied_at,
        }
    }
}

/// Saved entries, newest first.
#[derive(Default)]
pub struct ClipboardHistory {
    entries: Vec<Entry>,
}

impl ClipboardHistory {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    /// Every word of the query must appear in the entry. Newer entries rank
    /// higher; clipboard text never outranks a strong name match elsewhere.
    pub fn search(&self, matcher: &mut Matcher, ctx: &Context, limit: usize) -> Vec<Scored> {
        let words: Vec<&str> = matcher.query().split(' ').collect();
        self.entries
            .iter()
            .filter(|entry| words.iter().all(|word| entry.haystack.contains(word)))
            .take(limit)
            .enumerate()
            .map(|(rank, entry)| Scored {
                score: 1_000_u32.saturating_sub(rank as u32),
                result: result(entry, ctx),
            })
            .collect()
    }

    pub fn browse(&self, ctx: &Context) -> Vec<SearchResult> {
        self.entries
            .iter()
            .map(|entry| result(entry, ctx))
            .collect()
    }

    pub fn get(&self, id: i64, ctx: &Context) -> Option<SearchResult> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| result(entry, ctx))
    }
}

fn result(entry: &Entry, ctx: &Context) -> SearchResult {
    let id = Source::Clip.id(entry.id);
    let (label, symbol) = match entry.kind {
        ClipKind::Text => ("Text", Symbol::Text),
        ClipKind::Image => ("Image", Symbol::Image),
        ClipKind::Files => ("Files", Symbol::Files),
    };
    SearchResult {
        kind: ResultKind::Clipboard,
        title: entry.title.clone(),
        subtitle: format!("{label} · {}", ago(ctx.now - entry.copied_at)),
        icon: Icon::Symbol { name: symbol },
        // Mod+Enter runs the second action and may wait for newer results,
        // so Delete, which cannot be undone, never takes that place.
        actions: [
            Some(ResultAction::new("Copy", Action::CopyClip { id: entry.id })),
            Some(ctx.pin_action(&id)),
            // Text is saved plain already.
            (entry.kind == ClipKind::Files).then(|| {
                ResultAction::new("Copy as Plain Text", Action::CopyClipText { id: entry.id })
            }),
            Some(ResultAction::new(
                "Delete",
                Action::DeleteClip { id: entry.id },
            )),
        ]
        .into_iter()
        .flatten()
        .collect(),
        pinned: ctx.pinned(&id),
        id,
    }
}

fn ago(seconds: i64) -> String {
    let plural = |n: i64, unit: &str| format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match seconds.max(0) {
        0..60 => "Just now".into(),
        s @ 60..3_600 => plural(s / 60, "minute"),
        s @ 3_600..86_400 => plural(s / 3_600, "hour"),
        s => plural(s / 86_400, "day"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, text: &str) -> Entry {
        let content = Content::Text(text.into());
        Entry::new(id, content.kind(), content.title(), text, 0)
    }

    #[test]
    fn decides_what_fits() {
        assert!(Content::Text("hi".into()).fits());
        assert!(!Content::Text(" \n".into()).fits());
        assert!(!Content::Text("a\0b".into()).fits());
        assert!(!Content::Text("x".repeat(MAX_TEXT_BYTES + 1)).fits());
        assert!(!Content::Files(vec!["relative".into()]).fits());
        let image = |width, height| Content::Image {
            png: vec![],
            width,
            height,
        };
        assert!(image(4000, 4000).fits());
        assert!(!image(5000, 5000).fits());
    }

    #[test]
    fn titles_and_hashes() {
        assert_eq!(
            Content::Text("\n  first line \nsecond".into()).title(),
            "first line"
        );
        let files = Content::Files(vec!["/a/one.txt".into(), "/a/two.txt".into()]);
        assert_eq!(files.title(), "one.txt and 1 more");
        assert_ne!(
            Content::Text("a".into()).hash(),
            Content::Text("b".into()).hash()
        );
        assert_eq!(
            Content::Text("a".into()).hash(),
            Content::Text("a".into()).hash()
        );
    }

    #[test]
    fn search_needs_every_word_and_prefers_newer_entries() {
        let history = ClipboardHistory::new(vec![
            entry(2, "Meeting notes for Monday"),
            entry(1, "monday standup notes"),
        ]);
        let hits = history.search(&mut Matcher::new("notes MONDAY"), &Context::none(), 10);
        let ids: Vec<_> = hits.iter().map(|hit| hit.result.id.as_str()).collect();
        assert_eq!(ids, ["clip:2", "clip:1"]);
        assert!(hits[0].score > hits[1].score);
        let actions: Vec<_> = hits[0].result.actions.iter().map(|a| &a.action).collect();
        assert!(matches!(
            actions[..],
            [
                Action::CopyClip { .. },
                Action::Pin { .. },
                Action::DeleteClip { .. }
            ]
        ));
        assert!(
            history
                .search(&mut Matcher::new("friday"), &Context::none(), 10)
                .is_empty()
        );
    }

    #[test]
    fn copied_files_also_copy_as_plain_text() {
        let content = Content::Files(vec!["/a/one.txt".into()]);
        let files = Entry::new(3, content.kind(), content.title(), "/a/one.txt", 0);
        let history = ClipboardHistory::new(vec![files]);
        let result = history.get(3, &Context::none()).unwrap();
        let labels: Vec<_> = result.actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["Copy", "Pin", "Copy as Plain Text", "Delete"]);
        assert_eq!(result.actions[2].action, Action::CopyClipText { id: 3 });
    }

    #[test]
    fn describes_age() {
        assert_eq!(ago(5), "Just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(7_200), "2 hours ago");
        assert_eq!(ago(3 * 86_400), "3 days ago");
    }
}
