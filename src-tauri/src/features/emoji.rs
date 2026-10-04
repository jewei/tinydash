//! Emoji search by English name, GitHub shortcode, and optional CLDR keywords.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::{
    CATEGORY_LIMIT, Context,
    id::Source,
    matcher::Matcher,
    result::{Action, Icon, ResultAction, ResultKind, Scored, SearchResult},
    top,
};

/// Extra keyword languages. English names and shortcodes are always on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EmojiLanguage {
    /// Simplified Chinese.
    Zh,
    /// Malay.
    Ms,
    /// Spanish.
    Es,
}

impl EmojiLanguage {
    /// CLDR rows: emoji, short name, then related keywords, tab-separated.
    fn table(self) -> &'static str {
        match self {
            Self::Zh => include_str!("../../data/emoji/zh.tsv"),
            Self::Ms => include_str!("../../data/emoji/ms.tsv"),
            Self::Es => include_str!("../../data/emoji/es.tsv"),
        }
    }
}

/// A localized keyword ranks below an equally good name match.
const KEYWORD_PENALTY: u32 = 300;

struct Entry {
    emoji: &'static emojis::Emoji,
    names: Vec<String>,
    keywords: Vec<String>,
}

pub struct EmojiIndex {
    entries: Vec<Entry>,
}

impl Default for EmojiIndex {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl EmojiIndex {
    pub fn new(languages: &[EmojiLanguage]) -> Self {
        let mut entries: Vec<Entry> = emojis::iter()
            .map(|emoji| Entry {
                emoji,
                names: emoji.shortcodes().map(String::from).collect(),
                keywords: Vec::new(),
            })
            .collect();
        let position: std::collections::HashMap<&str, usize> = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.emoji.as_str(), i))
            .collect();
        for language in languages {
            for row in language.table().lines() {
                let mut fields = row.split('\t');
                let Some(emoji) = fields.next().and_then(emojis::get) else {
                    continue;
                };
                let Some(&i) = position.get(emoji.as_str()) else {
                    continue;
                };
                let entry = &mut entries[i];
                if let Some(name) = fields.next().filter(|name| !name.is_empty()) {
                    entry.names.push(name.to_owned());
                }
                entry.keywords.extend(fields.map(String::from));
            }
        }
        Self { entries }
    }

    pub fn search(&self, matcher: &mut Matcher, ctx: &Context, limit: usize) -> Vec<Scored> {
        let hits = self
            .entries
            .iter()
            .filter_map(|entry| {
                let names =
                    matcher.best(entry.emoji.name(), entry.names.iter().map(String::as_str));
                let keywords = entry
                    .keywords
                    .iter()
                    .filter_map(|keyword| matcher.name(keyword))
                    .max()
                    .map(|score| score.saturating_sub(KEYWORD_PENALTY));
                let score = names.max(keywords)?;
                Some((score + ctx.boost(&id(entry.emoji)), entry.emoji))
            })
            .collect();
        top(hits, limit)
            .into_iter()
            .map(|(score, emoji)| Scored {
                score,
                result: result(emoji, ctx),
            })
            .collect()
    }

    /// Most used first, then catalog order.
    pub fn browse(&self, ctx: &Context) -> Vec<SearchResult> {
        let mut used: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| ctx.boost(&id(entry.emoji)) > 0)
            .collect();
        used.sort_by_key(|entry| std::cmp::Reverse(ctx.boost(&id(entry.emoji))));
        let rest = self
            .entries
            .iter()
            .filter(|entry| ctx.boost(&id(entry.emoji)) == 0);
        used.into_iter()
            .chain(rest)
            .take(CATEGORY_LIMIT)
            .map(|entry| result(entry.emoji, ctx))
            .collect()
    }

    pub fn get(&self, glyph: &str, ctx: &Context) -> Option<SearchResult> {
        emojis::get(glyph).map(|emoji| result(base(emoji), ctx))
    }
}

/// Pins and usage use the default-tone emoji, so they survive a tone change.
fn base(emoji: &'static emojis::Emoji) -> &'static emojis::Emoji {
    emoji
        .with_skin_tone(emojis::SkinTone::Default)
        .unwrap_or(emoji)
}

fn id(emoji: &emojis::Emoji) -> String {
    Source::Emoji.id(emoji.as_str())
}

fn toned(emoji: &'static emojis::Emoji, tone: u8) -> &'static emojis::Emoji {
    use emojis::SkinTone::*;
    let tone = match tone {
        1 => Light,
        2 => MediumLight,
        3 => Medium,
        4 => MediumDark,
        5 => Dark,
        _ => return emoji,
    };
    emoji.with_skin_tone(tone).unwrap_or(emoji)
}

fn result(emoji: &'static emojis::Emoji, ctx: &Context) -> SearchResult {
    let id = id(emoji);
    let shown = toned(emoji, ctx.skin_tone);
    let mut title = shown.name().to_owned();
    if let Some(first) = title.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    SearchResult {
        kind: ResultKind::Emoji,
        title,
        subtitle: emoji
            .shortcode()
            .map_or_else(String::new, |code| format!(":{code}:")),
        icon: Icon::Emoji {
            glyph: shown.as_str().into(),
        },
        actions: vec![
            ResultAction::new(
                "Copy Emoji",
                Action::Copy {
                    text: shown.as_str().into(),
                },
            ),
            ctx.pin_action(&id),
        ],
        pinned: ctx.pinned(&id),
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::usage::{Pins, Usage};

    fn first(index: &EmojiIndex, query: &str, skin_tone: u8) -> SearchResult {
        let (usage, pins) = (Usage::default(), Pins::default());
        let ctx = Context {
            usage: &usage,
            pins: &pins,
            now: 0,
            skin_tone,
        };
        let mut hits = index.search(&mut Matcher::new(query), &ctx, 5);
        hits.remove(0).result
    }

    #[test]
    fn finds_by_name_and_shortcode() {
        let index = EmojiIndex::default();
        assert_eq!(
            first(&index, "rocket", 0).icon,
            Icon::Emoji {
                glyph: "🚀".into()
            }
        );
        assert_eq!(first(&index, "+1", 0).id, "emoji:👍");
    }

    #[test]
    fn localized_keywords_only_when_enabled() {
        assert!(
            EmojiIndex::default()
                .search(&mut Matcher::new("火箭"), &Context::none(), 5)
                .is_empty()
        );
        let index = EmojiIndex::new(&[EmojiLanguage::Zh, EmojiLanguage::Ms, EmojiLanguage::Es]);
        for query in ["火箭", "roket", "cohete"] {
            assert_eq!(first(&index, query, 0).id, "emoji:🚀", "{query}");
        }
    }

    #[test]
    fn skin_tone_changes_the_copy_but_not_the_id() {
        let wave = first(&EmojiIndex::default(), "waving hand", 3);
        assert_eq!(wave.id, "emoji:👋");
        assert_eq!(
            wave.icon,
            Icon::Emoji {
                glyph: "👋🏽".into()
            }
        );
    }
}
