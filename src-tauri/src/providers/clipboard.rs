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

pub fn valid_text(text: &str) -> bool {
    text.len() <= MAX_TEXT_BYTES && !text.trim().is_empty() && !text.contains('\0')
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

    pub fn get(&self, id: &str) -> Option<&ClipboardEntry> {
        let id = entry_id(id)?;
        self.entries
            .iter()
            .find(|entry| entry.entry.id == id)
            .map(|entry| &entry.entry)
    }

    pub fn search(&self, input: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
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
                    title: entry.title.clone(),
                    subtitle: entry.subtitle.clone(),
                    score,
                    icon: None,
                    primary_action: Action::Copy,
                    secondary_actions: vec![Action::Delete],
                    confirmation: None,
                    detail: None,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_text_without_modifying_copy_payload() {
        for invalid in ["", " \n\t", "a\0b", &"x".repeat(MAX_TEXT_BYTES + 1)] {
            assert!(!valid_text(invalid));
        }
        assert!(valid_text(&"🚀".repeat(MAX_TEXT_BYTES / 4)));
        let text = "  Café notes\n  Launch 🚀 tomorrow.  \n";
        let provider = ClipboardProvider::new(vec![ClipboardEntry {
            id: 7,
            content: text.into(),
            created_at: 1,
            last_used_at: None,
        }]);
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
}
