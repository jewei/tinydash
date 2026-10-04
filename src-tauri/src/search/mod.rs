//! Turns a query into ranked results. Everything here is pure: it reads a
//! [`Snapshot`] of the indexes and never touches Tauri, the database, or the OS.

pub mod matcher;
pub mod result;
pub mod usage;

use std::{
    cmp::Reverse,
    sync::{Arc, LazyLock},
};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    actions::Action,
    features::{
        apps::AppIndex, calculator, clipboard::ClipboardHistory, currency::Rates, datetime,
        emoji::EmojiIndex, files, files::FileIndex, library::Library, password, system,
        url_cleaner, web,
    },
    settings::Settings,
};
use matcher::{Matcher, STRONG};
use result::{ResultAction, Scored, SearchResult};
use usage::{Pins, Usage};

/// Results in All. Instant answers and the web fallback count toward it.
pub const ALL_LIMIT: usize = 30;
/// Results in a single category.
pub const CATEGORY_LIMIT: usize = 100;
/// Usage-based suggestions below the pins in an empty All search.
const SUGGESTIONS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Category {
    #[default]
    All,
    Apps,
    Files,
    Clipboard,
    Snippets,
    Emoji,
    System,
}

impl Category {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Apps,
        Self::Files,
        Self::Clipboard,
        Self::Snippets,
        Self::Emoji,
        Self::System,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Apps => "apps",
            Self::Files => "files",
            Self::Clipboard => "clipboard",
            Self::Snippets => "snippets",
            Self::Emoji => "emoji",
            Self::System => "system",
        }
    }

    /// The category that owns a result ID, from its prefix.
    fn of(id: &str) -> Option<Self> {
        let prefix = id.split_once(':')?.0;
        Some(match prefix {
            "app" => Self::Apps,
            "file" => Self::Files,
            "clip" => Self::Clipboard,
            "snippet" | "link" => Self::Snippets,
            "emoji" => Self::Emoji,
            "system" => Self::System,
            _ => return None,
        })
    }
}

/// Everything a search reads, captured at the start of the request.
pub struct Snapshot {
    pub settings: Arc<Settings>,
    pub apps: Arc<AppIndex>,
    pub files: Arc<FileIndex>,
    pub clipboard: Arc<ClipboardHistory>,
    pub library: Arc<Library>,
    pub emoji: Arc<EmojiIndex>,
    pub usage: Arc<Usage>,
    pub pins: Arc<Pins>,
    pub rates: Arc<Option<Rates>>,
    pub now: DateTime<Local>,
}

/// Personal ranking data that every source applies the same way.
pub struct Context<'a> {
    pub usage: &'a Usage,
    pub pins: &'a Pins,
    pub now: i64,
    pub skin_tone: u8,
}

impl Context<'static> {
    /// No usage and no pins, for results that are never ranked or pinned.
    pub fn none() -> Self {
        static USAGE: LazyLock<Usage> = LazyLock::new(Usage::default);
        static PINS: LazyLock<Pins> = LazyLock::new(Pins::default);
        Context {
            usage: &USAGE,
            pins: &PINS,
            now: 0,
            skin_tone: 0,
        }
    }
}

impl Context<'_> {
    pub fn boost(&self, id: &str) -> u32 {
        self.usage.bonus(id, self.now)
    }

    pub fn pinned(&self, id: &str) -> bool {
        self.pins.contains(id)
    }

    pub fn pin_action(&self, id: &str) -> ResultAction {
        if self.pinned(id) {
            ResultAction::new("Unpin", Action::Unpin { id: id.into() })
        } else {
            ResultAction::new("Pin", Action::Pin { id: id.into() })
        }
    }
}

pub fn search(snapshot: &Snapshot, query: &str, category: Category) -> Vec<SearchResult> {
    let context = Context {
        usage: &snapshot.usage,
        pins: &snapshot.pins,
        now: snapshot.now.timestamp(),
        skin_tone: snapshot.settings.emoji_skin_tone,
    };
    let query = query.trim();
    if query.is_empty() {
        return browse(snapshot, &context, category);
    }
    let matcher = &mut Matcher::new(query);
    let ctx = &context;
    let s = snapshot;
    let scored = match category {
        Category::All => return all(snapshot, ctx, query, matcher),
        Category::Apps => s.apps.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Files => s.files.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Clipboard => s.clipboard.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Snippets => s.library.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Emoji => s.emoji.search(matcher, ctx, CATEGORY_LIMIT),
        Category::System => system::search(matcher, ctx),
    };
    by_score(scored, CATEGORY_LIMIT)
}

/// An empty query: pins first, then the category's own order.
fn browse(s: &Snapshot, ctx: &Context, category: Category) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = s
        .pins
        .ids()
        .iter()
        .filter(|id| category == Category::All || Category::of(id) == Some(category))
        .filter_map(|id| resolve(s, ctx, id))
        .collect();
    let rest = match category {
        Category::All => s
            .usage
            .ranked(ctx.now)
            .into_iter()
            .filter(|id| !ctx.pinned(id) && suggestible(id))
            .filter_map(|id| resolve(s, ctx, id))
            .take(SUGGESTIONS)
            .collect(),
        Category::Apps => s.apps.browse(ctx),
        Category::Files => s
            .usage
            .ranked(ctx.now)
            .into_iter()
            .filter(|id| id.starts_with("file:"))
            .filter_map(|id| resolve(s, ctx, id))
            .collect(),
        Category::Clipboard => s.clipboard.browse(ctx),
        Category::Snippets => s.library.browse(ctx),
        Category::Emoji => s.emoji.browse(ctx),
        Category::System => system::browse(ctx),
    };
    results.extend(rest.into_iter().filter(|result| !result.pinned));
    results.truncate(CATEGORY_LIMIT);
    results
}

/// Suggestions skip clipboard text and system commands: neither should be
/// one keystroke away just because it was used before.
fn suggestible(id: &str) -> bool {
    !id.starts_with("clip:") && !id.starts_with("system:")
}

/// Rebuild the current result for a pinned or used ID.
fn resolve(s: &Snapshot, ctx: &Context, id: &str) -> Option<SearchResult> {
    let (prefix, key) = id.split_once(':')?;
    match prefix {
        "app" => s.apps.get(key, ctx),
        "file" => files::result_for_path(key, ctx),
        "clip" => s.clipboard.get(key.parse().ok()?, ctx),
        "snippet" | "link" => s.library.get(id, ctx),
        "emoji" => s.emoji.get(key, ctx),
        "system" => system::get(id, ctx),
        _ => None,
    }
}

/// All: instant answers, then name matches across sources, then fuzzy
/// matches, then a web search. Inside each tier, sources keep a fixed order
/// so that, for example, an app named like the query beats an emoji code.
fn all(s: &Snapshot, ctx: &Context, query: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
    let mut results = answers(s, query);
    let keyword_search = results.iter().any(|r| r.id.starts_with("web:"));

    let sources = [
        s.apps.search(matcher, ctx, ALL_LIMIT),
        system::search(matcher, ctx),
        s.library.search(matcher, ctx, ALL_LIMIT),
        s.files.search(matcher, ctx, ALL_LIMIT),
        s.clipboard.search(matcher, ctx, ALL_LIMIT),
        s.emoji.search(matcher, ctx, ALL_LIMIT),
    ];
    let mut ranked: Vec<(usize, Scored)> = sources
        .into_iter()
        .enumerate()
        .flat_map(|(order, scored)| scored.into_iter().map(move |hit| (order, hit)))
        .collect();
    ranked.sort_by_key(|(order, hit)| (hit.score < STRONG, *order, Reverse(hit.score)));

    let room = ALL_LIMIT.saturating_sub(results.len() + 1);
    results.extend(ranked.into_iter().take(room).map(|(_, hit)| hit.result));
    if !keyword_search {
        results.push(web::fallback(query, s.settings.search_engine));
    }
    results
}

/// Results computed from the query itself rather than looked up in an index.
fn answers(s: &Snapshot, query: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();
    results.extend(calculator::answer(query, Option::as_ref(&s.rates)));
    results.extend(datetime::answers(query, s.now));
    results.extend(password::answers(query));
    results.extend(url_cleaner::answer(query));
    results.extend(web::answer(query));
    results.extend(s.library.quicklink_answer(query));
    results
}

fn by_score(mut scored: Vec<Scored>, limit: usize) -> Vec<SearchResult> {
    scored.sort_by_key(|hit| Reverse(hit.score));
    scored
        .into_iter()
        .take(limit)
        .map(|hit| hit.result)
        .collect()
}

/// Keep the best `limit` hits by score, without sorting every match.
pub fn top<T>(mut hits: Vec<(u32, T)>, limit: usize) -> Vec<(u32, T)> {
    if hits.len() > limit {
        hits.select_nth_unstable_by_key(limit, |(score, _)| Reverse(*score));
        hits.truncate(limit);
    }
    hits.sort_by_key(|(score, _)| Reverse(*score));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_keeps_the_best_hits_in_order() {
        let hits = vec![(1, 'a'), (5, 'b'), (3, 'c'), (4, 'd')];
        assert_eq!(top(hits, 2), [(5, 'b'), (4, 'd')]);
        assert_eq!(top(vec![(1, 'a')], 5), [(1, 'a')]);
    }

    #[test]
    fn ids_map_to_their_category() {
        assert_eq!(Category::of("app:/A.app"), Some(Category::Apps));
        assert_eq!(Category::of("link:3"), Some(Category::Snippets));
        assert_eq!(Category::of("calc:1+1"), None);
    }
}
