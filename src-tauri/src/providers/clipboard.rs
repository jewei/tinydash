use nucleo_matcher::{
    Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use serde::Serialize;

use crate::{
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

pub const MAX_TEXT_BYTES: usize = 16_384;
pub const MAX_SELECTION_ENTRIES: usize = 100;
pub const MAX_SELECTION_BYTES: usize = MAX_TEXT_BYTES;

pub fn valid_text(text: &str) -> bool {
    text.len() <= MAX_TEXT_BYTES && !text.trim().is_empty() && !text.contains('\0')
}

/// Formats that a source adds when it copies a password, one-time code, or
/// other secret. History never reads or saves such content. The presence of
/// the format is the signal; its payload varies between sources.
///
/// - macOS: the nspasteboard.org concealed and transient types, and Apple's
///   marker for autofill and browser password fields.
/// - Windows: the documented clipboard monitor exclusion, and the older
///   format that KeePass and similar tools set.
/// - Linux: the KDE password manager hint, also set by KeePassXC.
pub const SECRET_FORMATS: [&str; 6] = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "com.apple.is-sensitive",
    "ExcludeClipboardContentFromMonitorProcessing",
    "Clipboard Viewer Ignore",
    "x-kde-passwordManagerHint",
];

// Windows registers each name instead. Its reader uses the list directly.
#[cfg_attr(windows, allow(dead_code))]
pub fn is_secret_format(format: &str) -> bool {
    SECRET_FORMATS.contains(&format)
}

/// The result of one read after the native clipboard changes.
#[derive(Debug, PartialEq, Eq)]
pub enum Observed {
    /// Text that history can save.
    Text(String),
    /// Content that its source marked as secret. The text is never read.
    Secret,
    /// A source emptied the clipboard. Password managers do this after a copy.
    /// Linux cannot tell a clear from an application exit, so it never reports one.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    Cleared,
    /// Non-text content, or text that history does not accept.
    Other,
}

impl Observed {
    pub fn from_text(text: Option<String>) -> Self {
        text.filter(|text| valid_text(text))
            .map_or(Self::Other, Self::Text)
    }
}

pub fn combine_entries(entries: &[&ClipboardEntry], separator: &str) -> Result<String, String> {
    if entries.is_empty() {
        return Err("Select at least one clipboard entry.".into());
    }
    if entries.len() > MAX_SELECTION_ENTRIES {
        return Err(format!(
            "Select no more than {MAX_SELECTION_ENTRIES} clipboard entries."
        ));
    }
    if separator.contains('\0') {
        return Err("The clipboard separator contains an invalid character.".into());
    }
    if entries.iter().any(|entry| !valid_text(&entry.content)) {
        return Err("A selected clipboard entry is no longer available.".into());
    }
    let bytes = entries
        .iter()
        .map(|entry| entry.content.len())
        .sum::<usize>()
        .saturating_add(
            separator
                .len()
                .saturating_mul(entries.len().saturating_sub(1)),
        );
    if bytes > MAX_SELECTION_BYTES {
        return Err(format!(
            "The selected clipboard text is larger than {MAX_SELECTION_BYTES} bytes."
        ));
    }
    let mut combined = String::with_capacity(bytes);
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            combined.push_str(separator);
        }
        combined.push_str(&entry.content);
    }
    Ok(combined)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

pub fn entry_id(id: &str) -> Option<i64> {
    id.strip_prefix("clipboard:")?
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
}

pub struct IndexedEntry {
    entry: ClipboardEntry,
    text: Utf32String,
    normalized: String,
    title: String,
    subtitle: String,
}

impl From<ClipboardEntry> for IndexedEntry {
    fn from(entry: ClipboardEntry) -> Self {
        let title = entry
            .content
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .chars()
            .take(100)
            .collect();
        Self {
            text: entry.content.as_str().into(),
            normalized: ranking::normalize(&entry.content),
            title,
            subtitle: format!("Text · {} characters", entry.content.chars().count()),
            entry,
        }
    }
}

#[derive(Default)]
pub struct ClipboardProvider {
    // Newest first. Stable result sorting preserves this order for equal matches.
    entries: Vec<IndexedEntry>,
}

impl ClipboardProvider {
    pub fn new(entries: Vec<ClipboardEntry>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .filter(|entry| valid_text(&entry.content))
                .map(Into::into)
                .collect(),
        }
    }

    pub fn update(&mut self, entry: IndexedEntry, removed: &[i64]) {
        self.entries
            .retain(|old| old.entry.id != entry.entry.id && !removed.contains(&old.entry.id));
        self.entries.insert(0, entry);
    }

    pub fn remove(&mut self, id: i64) {
        self.entries.retain(|entry| entry.entry.id != id);
    }

    pub fn remove_many(&mut self, ids: &[i64]) {
        self.entries.retain(|entry| !ids.contains(&entry.entry.id));
    }

    pub fn newest_id(&self) -> Option<String> {
        self.entries
            .first()
            .map(|entry| format!("clipboard:{}", entry.entry.id))
    }

    pub fn entries_for_ids(&self, ids: &[i64]) -> Option<Vec<&ClipboardEntry>> {
        ids.iter()
            .map(|id| {
                self.entries
                    .iter()
                    .find(|entry| entry.entry.id == *id)
                    .map(|entry| &entry.entry)
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&ClipboardEntry> {
        let id = entry_id(id)?;
        self.entries
            .iter()
            .find(|entry| entry.entry.id == id)
            .map(|entry| &entry.entry)
    }

    pub fn search(&self, input: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
        #[cfg(test)]
        super::search_work::record(super::search_work::Provider::Clipboard, self.entries.len());
        let query = ranking::normalize(input);
        let pattern = Pattern::new(
            &query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        self.entries
            .iter()
            .filter_map(|entry| {
                let score = if query.is_empty() {
                    0
                } else {
                    ranking::name_score(
                        pattern.score(entry.text.slice(..), matcher)?,
                        &entry.normalized,
                        &query,
                    )
                };
                Some(SearchResult {
                    id: format!("clipboard:{}", entry.entry.id),
                    kind: ResultKind::Clipboard,
                    path: None,
                    title: entry.title.clone(),
                    subtitle: entry.subtitle.clone(),
                    score,
                    icon: None,
                    primary_action: Action::Copy,
                    secondary_actions: vec![Action::Delete],
                    pin: None,
                    confirmation: None,
                    detail: None,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn secret_formats_cover_each_platform_convention() {
        for format in [
            "org.nspasteboard.ConcealedType",
            "org.nspasteboard.TransientType",
            "com.apple.is-sensitive",
            "ExcludeClipboardContentFromMonitorProcessing",
            "Clipboard Viewer Ignore",
            "x-kde-passwordManagerHint",
        ] {
            assert!(is_secret_format(format), "{format}");
        }
        for format in [
            "public.utf8-plain-text",
            "UTF8_STRING",
            "CF_UNICODETEXT",
            "",
        ] {
            assert!(!is_secret_format(format), "{format}");
        }
        assert_eq!(
            Observed::from_text(Some("A".into())),
            Observed::Text("A".into())
        );
        assert_eq!(Observed::from_text(Some(" \n".into())), Observed::Other);
        assert_eq!(Observed::from_text(None), Observed::Other);
    }

    use super::*;

    fn entry(id: i64, content: &str) -> ClipboardEntry {
        ClipboardEntry {
            id,
            content: content.into(),
            created_at: 1,
            last_used_at: None,
        }
    }

    #[test]
    fn filters_text_without_modifying_copy_payload() {
        for invalid in ["", " \n\t", "a\0b", &"x".repeat(MAX_TEXT_BYTES + 1)] {
            assert!(!valid_text(invalid));
        }
        assert!(valid_text(&"🚀".repeat(MAX_TEXT_BYTES / 4)));
        let text = "  Café notes\n  Launch 🚀 tomorrow.  \n";
        let provider = ClipboardProvider::new(vec![entry(7, text)]);
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let results = provider.search("cafe launch", &mut matcher);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Café notes");
        assert_eq!(provider.get("clipboard:7").expect("entry").content, text);
        assert!(provider.search("zzzz", &mut matcher).is_empty());
        for id in [
            "7",
            "clipboard:-1",
            "clipboard:0",
            "clipboard:8",
            "clipboard:999999999999999999999",
        ] {
            assert!(provider.get(id).is_none());
        }
    }

    #[test]
    fn combines_entries_in_input_order_without_changing_text() {
        let first = entry(1, "  Café 🚀\n");
        let second = entry(2, "second");
        assert_eq!(
            combine_entries(&[&first, &second], "\n---\n").expect("combine"),
            "  Café 🚀\n\n---\nsecond"
        );
        assert_eq!(
            combine_entries(&[&second, &first], "").expect("combine"),
            "second  Café 🚀\n"
        );
        assert!(combine_entries(&[], "\n").is_err());
        assert!(combine_entries(&[&entry(1, &"x".repeat(MAX_SELECTION_BYTES))], "y").is_ok());
        assert!(
            combine_entries(
                &[&entry(1, &"x".repeat(MAX_SELECTION_BYTES)), &entry(2, "z")],
                "y"
            )
            .is_err()
        );
    }

    #[test]
    fn reports_newest_entry_and_resolves_ordered_ids() {
        let provider = ClipboardProvider::new(vec![entry(2, "new"), entry(1, "old")]);
        assert_eq!(provider.newest_id().as_deref(), Some("clipboard:2"));
        assert_eq!(
            provider
                .entries_for_ids(&[1, 2, 1])
                .expect("entries")
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            [1, 2, 1]
        );
        assert!(provider.entries_for_ids(&[2, 999]).is_none());
    }

    #[test]
    fn rejects_oversized_selection_without_copying_selected_payloads() {
        let provider = ClipboardProvider::new(
            (1..=MAX_SELECTION_ENTRIES as i64)
                .map(|id| entry(id, &"x".repeat(MAX_TEXT_BYTES)))
                .collect(),
        );
        let ids: Vec<_> = (1..=MAX_SELECTION_ENTRIES as i64).rev().collect();
        let selected = provider.entries_for_ids(&ids).expect("entries");
        for (entry, id) in selected.iter().zip(ids) {
            let original = provider.get(&format!("clipboard:{id}")).expect("entry");
            assert!(std::ptr::eq(*entry, original));
        }
        assert!(combine_entries(&selected, "\n").is_err());
    }
}
