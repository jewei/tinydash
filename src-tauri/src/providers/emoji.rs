use emojis::{Emoji, Group};
use nucleo_matcher::{
    Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

struct IndexedEmoji {
    emoji: &'static Emoji,
    terms: Vec<(Utf32String, String)>,
    category: &'static str,
    category_match: Utf32String,
}

pub struct EmojiProvider {
    entries: Vec<IndexedEmoji>,
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

impl Default for EmojiProvider {
    fn default() -> Self {
        let entries = emojis::iter()
            .map(|emoji| {
                let category = category(emoji.group());
                // The crate provides names, shortcodes and groups, but no keyword list.
                let synonyms: &[&str] = match emoji.as_str() {
                    "😂" => &["laugh", "lol"],
                    _ => &[],
                };
                let terms = std::iter::once(emoji.name())
                    .chain(emoji.shortcodes())
                    .chain(synonyms.iter().copied())
                    .map(|term| {
                        let normalized = ranking::normalize(&term.replace('_', " "));
                        (normalized.as_str().into(), normalized)
                    })
                    .collect();
                IndexedEmoji {
                    emoji,
                    terms,
                    category,
                    category_match: category.into(),
                }
            })
            .collect();
        Self { entries }
    }
}

impl EmojiProvider {
    pub fn copy_value(id: &str) -> Option<&'static str> {
        emojis::get(id.strip_prefix("emoji:")?).map(Emoji::as_str)
    }

    pub fn search(&self, query: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
        let query = ranking::normalize(&query.replace('_', " "));
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
                } else if query == entry.emoji.as_str() {
                    ranking::name_score(0, &query, &query)
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
                    terms.into_iter().chain(group).max()?
                };
                let shortcode = entry
                    .emoji
                    .shortcode()
                    .map(|code| format!(":{code}: · "))
                    .unwrap_or_default();
                Some(SearchResult {
                    id: format!("emoji:{}", entry.emoji.as_str()),
                    kind: ResultKind::Emoji,
                    path: None,
                    title: entry.emoji.name().to_owned(),
                    subtitle: format!("{shortcode}{}", entry.category),
                    score,
                    icon: Some(entry.emoji.as_str().to_owned()),
                    primary_action: Action::Copy,
                    secondary_actions: vec![],
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
}
