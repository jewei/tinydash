use emojis::{Emoji, Group, SkinTone};
use nucleo_matcher::{
    Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};
use unicode_normalization::UnicodeNormalization;

use crate::{
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

struct IndexedEmoji {
    emoji: &'static Emoji,
    terms: Vec<(Utf32String, String)>,
    category_match: Utf32String,
}

pub struct EmojiProvider {
    entries: Vec<IndexedEmoji>,
    skin_tone: u8,
}

fn category(group: Group) -> &'static str {
    match group {
        Group::SmileysAndEmotion => "Smileys & emotion",
        Group::PeopleAndBody => "People & body",
        Group::AnimalsAndNature => "Animals & nature",
        Group::FoodAndDrink => "Food & drink",
        Group::TravelAndPlaces => "Travel & places",
        Group::Activities => "Activities",
        Group::Objects => "Objects",
        Group::Symbols => "Symbols",
        Group::Flags => "Flags",
    }
}

fn base(emoji: &'static Emoji) -> &'static Emoji {
    emoji.with_skin_tone(SkinTone::Default).unwrap_or(emoji)
}

/// Pins and usage refer to the base emoji; action IDs retain the displayed variant.
pub fn canonical_id(id: &str) -> Cow<'_, str> {
    if let Some(value) = id.strip_prefix("emoji:")
        && let Some(emoji) = emojis::get(value)
        && base(emoji).as_str() != value
    {
        return Cow::Owned(format!("emoji:{}", base(emoji).as_str()));
    }
    Cow::Borrowed(id)
}

fn preferred(emoji: &'static Emoji, tone: u8) -> &'static Emoji {
    let tone = match tone {
        1 => SkinTone::Light,
        2 => SkinTone::MediumLight,
        3 => SkinTone::Medium,
        4 => SkinTone::MediumDark,
        5 => SkinTone::Dark,
        _ => SkinTone::Default,
    };
    emoji.with_skin_tone(tone).unwrap_or(emoji)
}

fn result(emoji: &'static Emoji, score: u32) -> SearchResult {
    let shortcode = base(emoji)
        .shortcode()
        .map(|code| format!(":{code}: · "))
        .unwrap_or_default();
    SearchResult {
        id: format!("emoji:{}", emoji.as_str()),
        kind: ResultKind::Emoji,
        path: None,
        title: emoji.name().to_owned(),
        subtitle: format!("{shortcode}{}", category(emoji.group())),
        score,
        icon: Some(emoji.as_str().to_owned()),
        primary_action: Action::Copy,
        secondary_actions: vec![],
        pin: None,
        confirmation: None,
        detail: None,
    }
}

fn keywords(languages: &[String]) -> HashMap<&'static str, Vec<&'static str>> {
    let mut keywords: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    for language in languages {
        let data = match language.as_str() {
            "zh" => include_str!("../../data/emoji/zh.tsv"),
            "ms" => include_str!("../../data/emoji/ms.tsv"),
            "es" => include_str!("../../data/emoji/es.tsv"),
            _ => continue,
        };
        for line in data.lines() {
            let mut fields = line.split('\t');
            if let Some(emoji) = fields.next().and_then(emojis::get) {
                keywords
                    .entry(base(emoji).as_str())
                    .or_default()
                    .extend(fields);
            }
        }
    }
    keywords
}

impl Default for EmojiProvider {
    fn default() -> Self {
        Self::new(0, &[])
    }
}

impl EmojiProvider {
    pub fn new(skin_tone: u8, languages: &[String]) -> Self {
        let keywords = keywords(languages);
        let entries = emojis::iter()
            .map(|emoji| {
                let synonyms: &[&str] = match emoji.as_str() {
                    "😂" => &["laugh", "lol"],
                    _ => &[],
                };
                let mut seen = HashSet::new();
                let terms = std::iter::once(emoji.name())
                    .chain(emoji.shortcodes())
                    .chain(synonyms.iter().copied())
                    .chain(keywords.get(emoji.as_str()).into_iter().flatten().copied())
                    .map(|term| {
                        ranking::normalize(&term.replace('_', " ").nfc().collect::<String>())
                    })
                    .filter(|term| seen.insert(term.clone()))
                    .map(|term| (term.as_str().into(), term))
                    .collect();
                IndexedEmoji {
                    emoji,
                    terms,
                    category_match: category(emoji.group()).into(),
                }
            })
            .collect();
        Self { entries, skin_tone }
    }

    pub fn set_skin_tone(&mut self, tone: u8) {
        self.skin_tone = tone;
    }

    pub fn copy_value(id: &str) -> Option<&'static str> {
        emojis::get(id.strip_prefix("emoji:")?).map(Emoji::as_str)
    }

    pub fn pinned_result(id: &str, tone: u8) -> Option<SearchResult> {
        let emoji = emojis::get(id.strip_prefix("emoji:")?)?;
        Some(result(preferred(base(emoji), tone), 0))
    }

    #[cfg(test)]
    pub fn search(&self, query: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
        self.search_interruptible(query, matcher, || false)
    }

    pub fn search_interruptible(
        &self,
        query: &str,
        matcher: &mut Matcher,
        mut cancelled: impl FnMut() -> bool,
    ) -> Vec<SearchResult> {
        #[cfg(test)]
        super::search_work::record(super::search_work::Provider::Emoji, self.entries.len());
        if cancelled() {
            return Vec::new();
        }
        // An explicitly typed emoji keeps its exact tone, including mixed tones.
        if let Some(emoji) = emojis::get(query.trim()) {
            return vec![result(
                emoji,
                ranking::name_score(0, emoji.as_str(), emoji.as_str()),
            )];
        }
        let query = ranking::normalize(&query.replace('_', " ").nfc().collect::<String>());
        let pattern = Pattern::new(
            &query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut results = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            if index % 64 == 0 && cancelled() {
                return Vec::new();
            }
            let score = if query.is_empty() {
                0
            } else {
                let terms = entry
                    .terms
                    .iter()
                    .filter_map(|(term, normalized)| {
                        pattern
                            .score(term.slice(..), matcher)
                            .map(|score| ranking::name_score(score, normalized, &query))
                    })
                    .max();
                let group = pattern
                    .score(entry.category_match.slice(..), matcher)
                    .map(|score| score / 4);
                let Some(score) = terms.into_iter().chain(group).max() else {
                    continue;
                };
                score
            };
            results.push(result(preferred(entry.emoji, self.skin_tone), score));
        }
        results
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nucleo_matcher::Config;

    #[test]
    fn searches_names_shortcodes_synonyms_and_categories_locally() {
        let provider = EmojiProvider::default();
        let mut matcher = Matcher::new(Config::DEFAULT);
        for (query, expected) in [
            ("rocket", "🚀"),
            ("COFFEE", "☕"),
            ("laugh", "😂"),
            ("heart", "❤️"),
            ("party_popper", "🎉"),
        ] {
            let results = ranking::top_results(provider.search(query, &mut matcher), 30);
            assert_eq!(results[0].icon.as_deref(), Some(expected), "{query}");
            assert_eq!(EmojiProvider::copy_value(&results[0].id), Some(expected));
        }
        assert!(!provider.search("food", &mut matcher).is_empty());
        assert!(provider.search("zzzzzzzzzz", &mut matcher).is_empty());
        assert_eq!(EmojiProvider::copy_value("emoji:arbitrary text"), None);
    }
    #[test]
    fn applies_all_tones_and_preserves_exact_action_values() {
        let mut matcher = Matcher::new(Config::DEFAULT);
        for (tone, expected) in ["👍", "👍🏻", "👍🏼", "👍🏽", "👍🏾", "👍🏿"].iter().enumerate()
        {
            let provider = EmojiProvider::new(tone as u8, &[]);
            let results = ranking::top_results(provider.search("thumbs up", &mut matcher), 30);
            assert_eq!(results[0].icon.as_deref(), Some(*expected));
            assert_eq!(EmojiProvider::copy_value(&results[0].id), Some(*expected));
            assert_eq!(canonical_id(&results[0].id), "emoji:👍");
            let rocket = provider.search("🚀", &mut matcher);
            assert_eq!(rocket[0].icon.as_deref(), Some("🚀"));
            // Changing a preference cannot change the bytes of a displayed action ID.
            let mut changed = provider;
            changed.set_skin_tone(5);
            assert_eq!(EmojiProvider::copy_value(&results[0].id), Some(*expected));
        }
        let provider = EmojiProvider::new(1, &[]);
        for glyph in ["👍", "👍🏿", "👩🏽‍💻", "🫱🏻‍🫲🏿"] {
            let results = provider.search(glyph, &mut matcher);
            assert_eq!(results.len(), 1);
            assert_eq!(EmojiProvider::copy_value(&results[0].id), Some(glyph));
        }
    }

    #[test]
    fn selected_cldr_languages_add_keywords_without_replacing_english() {
        let mut matcher = Matcher::new(Config::DEFAULT);
        for (language, query) in [("zh", "火箭"), ("ms", "roket"), ("es", "cohete")] {
            let provider = EmojiProvider::new(0, &[language.into()]);
            for query in [query, "rocket"] {
                let results = ranking::top_results(provider.search(query, &mut matcher), 30);
                assert_eq!(
                    results[0].icon.as_deref(),
                    Some("🚀"),
                    "{language}: {query}"
                );
            }
        }
        let provider = EmojiProvider::new(3, &["zh".into(), "ms".into(), "es".into()]);
        let results = ranking::top_results(provider.search("拇指向上", &mut matcher), 30);
        assert_eq!(results[0].icon.as_deref(), Some("👍🏽"));
        let composed = provider.search("café", &mut matcher);
        let decomposed = provider.search("cafe\u{301}", &mut matcher);
        assert_eq!(
            ranking::top_results(composed, 1)[0].id,
            ranking::top_results(decomposed, 1)[0].id
        );
        assert!(
            EmojiProvider::default()
                .search("火箭", &mut matcher)
                .is_empty()
        );
    }

    #[test]
    fn emoji_keyword_search_stops_without_partial_results() {
        let provider = EmojiProvider::new(0, &["zh".into(), "ms".into(), "es".into()]);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut checkpoints = 0;
        let results = provider.search_interruptible("", &mut matcher, || {
            checkpoints += 1;
            checkpoints >= 3
        });
        assert!(results.is_empty());
        assert_eq!(checkpoints, 3);
    }
}
