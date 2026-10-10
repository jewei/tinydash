//! Turns a query into ranked results. It reads a [`Snapshot`] of the indexes
//! and never touches Tauri, the database, or the clipboard.

pub mod id;
pub mod matcher;
pub mod result;
pub mod usage;

use std::{
    cmp::Reverse,
    collections::HashSet,
    sync::{Arc, LazyLock},
};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    features::{
        apps::AppIndex, calculator, clip_card, clipboard::ClipboardHistory, currency::Rates,
        datetime, emoji::EmojiIndex, files::FileIndex, library::Library, password, permissions,
        system, url_cleaner, web,
    },
    settings::Settings,
};
use id::Source;
use matcher::{EXACT_ALIAS, Matcher, STRONG};
use result::{Action, ResultAction, ResultKind, Scored, SearchResult};
use usage::{Aliases, Hidden, Pins, Usage};

/// Results in an All search with text. Instant answers and the web
/// fallback, when there is one, count toward it. The empty All view stops
/// at `CATEGORY_LIMIT`, like a category.
pub const ALL_LIMIT: usize = 30;
/// Results in a single category.
pub const CATEGORY_LIMIT: usize = 100;
/// Quicklinks offered after the web search at the end of an All search.
const QUICKLINK_FALLBACKS: usize = 3;
/// Usage-based suggestions below the pins in an empty All search.
const SUGGESTIONS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
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
    /// Every category, in tab order. `cli::parse` searches this list, so a
    /// new variant must be added here.
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
    pub hidden: Arc<Hidden>,
    pub aliases: Arc<Aliases>,
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

/// The results for a query in a category, without the ones the user hid,
/// and with a way to give each of the others an alias or hide it.
pub fn search(snapshot: &Snapshot, query: &str, category: Category) -> Vec<SearchResult> {
    let mut results = find(snapshot, query, category);
    results.retain(|result| !snapshot.hidden.contains(&result.id));
    for result in &mut results {
        let Some((source, _)) = Source::parse(&result.id) else {
            continue;
        };
        if source.aliasable() && !carries_text(result) {
            let alias = snapshot.aliases.get(&result.id).unwrap_or_default();
            let label = if alias.is_empty() {
                "Add Alias…".to_owned()
            } else {
                format!("Change Alias “{alias}”…")
            };
            result.actions.push(ResultAction::new(
                label,
                Action::SetAlias {
                    id: result.id.clone(),
                    alias: alias.to_owned(),
                },
            ));
        }
        if source.hideable() {
            result.actions.push(ResultAction::new(
                "Hide from Results",
                Action::Hide {
                    id: result.id.clone(),
                },
            ));
        }
    }
    results
}

/// A keyword answer or a fallback, which runs its item with typed text,
/// such as `Jira: ABC-12`. An alias names the item, so it is set on the
/// item's own row.
fn carries_text(result: &SearchResult) -> bool {
    result.actions.first().is_some_and(|first| {
        matches!(
            &first.action,
            Action::OpenQuicklink { query, .. } | Action::CopySnippet { query, .. }
                if !query.is_empty()
        )
    })
}

fn find(snapshot: &Snapshot, query: &str, category: Category) -> Vec<SearchResult> {
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
    let mut scored = match category {
        Category::All => return all(snapshot, ctx, query, matcher),
        Category::Apps => s.apps.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Files => s.files.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Clipboard => s.clipboard.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Snippets => s.library.search(matcher, ctx, CATEGORY_LIMIT),
        Category::Emoji => s.emoji.search(matcher, ctx, CATEGORY_LIMIT),
        Category::System => system::search(matcher, ctx),
    };
    scored.extend(alias_hits(s, ctx, matcher, category));
    by_score(scored, CATEGORY_LIMIT)
}

/// Results whose alias starts with the query, in a category or in All.
/// The same result may also match by name; the better score keeps it.
fn alias_hits(
    s: &Snapshot,
    ctx: &Context,
    matcher: &mut Matcher,
    category: Category,
) -> Vec<Scored> {
    s.aliases
        .iter()
        .filter(|(id, _)| {
            Source::parse(id).is_some_and(|(source, _)| match category {
                Category::All => s.settings.in_all(source.category()),
                _ => source.category() == category,
            })
        })
        .filter_map(|(id, alias)| {
            let score = matcher.alias(alias)?;
            Some(Scored {
                score: score + ctx.boost(id),
                result: resolve(s, ctx, id)?,
            })
        })
        .collect()
}

/// An empty query: pins first, then the category's own order.
fn browse(s: &Snapshot, ctx: &Context, category: Category) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = s
        .pins
        .ids()
        .iter()
        .filter(|id| {
            Source::parse(id).is_some_and(|(source, _)| match category {
                // A pinned clip is kept, not a favorite to launch: it heads
                // the Clipboard tab, and a search still finds it, but the
                // start screen leaves its rows to apps and other favorites.
                Category::All => source != Source::Clip,
                _ => source.category() == category,
            })
        })
        .filter_map(|id| resolve(s, ctx, id))
        .collect();
    let rest = match category {
        Category::All => s
            .usage
            .ranked(ctx.now)
            .into_iter()
            .filter(|id| {
                !ctx.pinned(id)
                    && Source::parse(id).is_some_and(|(source, _)| {
                        source.suggestible() && s.settings.in_all(source.category())
                    })
            })
            .filter_map(|id| resolve(s, ctx, id))
            .take(SUGGESTIONS)
            .collect(),
        Category::Apps => s.apps.browse(ctx),
        Category::Files => s
            .usage
            .ranked(ctx.now)
            .into_iter()
            .filter(|id| Source::parse(id).is_some_and(|(source, _)| source == Source::File))
            .filter_map(|id| resolve(s, ctx, id))
            .take(CATEGORY_LIMIT)
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

/// Rebuild the current result for a pinned or used ID, or `None` when the
/// item is gone.
pub fn resolve(s: &Snapshot, ctx: &Context, id: &str) -> Option<SearchResult> {
    let (source, key) = Source::parse(id)?;
    match source {
        Source::App => s.apps.get(key, ctx),
        Source::File => s.files.get(key, ctx),
        Source::Clip => s.clipboard.get(key.parse().ok()?, ctx),
        Source::Snippet => s
            .library
            .get(key.parse().ok()?, ctx)
            .filter(|r| r.kind == ResultKind::Snippet),
        Source::Link => s
            .library
            .get(key.parse().ok()?, ctx)
            .filter(|r| r.kind == ResultKind::Quicklink),
        Source::Emoji => s.emoji.get(key, ctx),
        Source::System => system::get(key, ctx),
    }
}

/// All: instant answers, then name matches across sources, then fuzzy
/// matches, then a web search unless a web keyword already answered. Inside
/// each tier, sources keep a fixed order so that, for example, an app named
/// like the query beats an emoji code.
fn all(s: &Snapshot, ctx: &Context, query: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
    let mut results = answers(s, query);
    let keyword_search = results.iter().any(|r| r.kind == ResultKind::WebSearch);

    // A whole alias goes first; an alias the query only starts ranks with
    // its result's source, as in that source's tab.
    let (exact, mut partial): (Vec<Scored>, Vec<Scored>) =
        alias_hits(s, ctx, matcher, Category::All)
            .into_iter()
            .partition(|hit| hit.score >= EXACT_ALIAS);
    // A source the user keeps out of All is not searched at all.
    let mut source = |category, search: &mut dyn FnMut() -> Vec<Scored>| {
        s.settings.in_all(category).then(|| {
            let mut hits = search();
            hits.extend(partial.extract_if(.., |hit| {
                Source::parse(&hit.result.id).is_some_and(|(of, _)| of.category() == category)
            }));
            hits
        })
    };
    let sources = [
        Some(exact),
        source(Category::Apps, &mut || {
            s.apps.search(matcher, ctx, ALL_LIMIT)
        }),
        source(Category::System, &mut || system::search(matcher, ctx)),
        source(Category::Snippets, &mut || {
            s.library.search(matcher, ctx, ALL_LIMIT)
        }),
        source(Category::Files, &mut || {
            s.files.search(matcher, ctx, ALL_LIMIT)
        }),
        source(Category::Clipboard, &mut || {
            s.clipboard.search(matcher, ctx, ALL_LIMIT)
        }),
        source(Category::Emoji, &mut || {
            s.emoji.search(matcher, ctx, ALL_LIMIT)
        }),
    ];
    let mut ranked: Vec<(usize, Scored)> = sources
        .into_iter()
        .enumerate()
        .flat_map(|(order, scored)| scored.into_iter().flatten().map(move |hit| (order, hit)))
        .collect();
    ranked.sort_by_key(|(order, hit)| (hit.score < STRONG, *order, Reverse(hit.score)));

    // A keyword answer has its snippet's or quicklink's own ID, and a result
    // may match by name and by alias; keep only the first of each ID, so no
    // two rows share one. A keyword answer carries the typed text.
    let answered: HashSet<String> = results.iter().map(|r| r.id.clone()).collect();
    let mut seen = answered.clone();
    let ranked: Vec<SearchResult> = ranked
        .into_iter()
        .map(|(_, hit)| hit.result)
        .filter(|result| seen.insert(result.id.clone()))
        .collect();
    // Quicklinks that can open the query, unless the user already chose a
    // keyword, such as `g` or a quicklink's own, or keeps snippets out of
    // All. A quicklink that shows as a name match is not offered again.
    let chose = results.iter().any(|r| {
        matches!(
            r.kind,
            ResultKind::WebSearch | ResultKind::Quicklink | ResultKind::Snippet
        )
    });
    let candidates: Vec<SearchResult> = if chose || !s.settings.in_all(Category::Snippets) {
        Vec::new()
    } else {
        s.library
            .fallbacks(query, ctx, s.now)
            .into_iter()
            .filter(|link| !s.hidden.contains(&link.id) && !answered.contains(&link.id))
            .collect()
    };
    let fallback = usize::from(!keyword_search);
    let room = |links: usize| ALL_LIMIT.saturating_sub(results.len() + fallback + links);
    // The links take room from the name matches, and the name matches that
    // show decide which links are left; fewer links leave more room, so
    // this settles within a few steps.
    let mut links = QUICKLINK_FALLBACKS.min(candidates.len());
    let (room, links) = loop {
        let room = room(links);
        let shown: HashSet<&str> = ranked.iter().take(room).map(|r| r.id.as_str()).collect();
        let offered: Vec<&SearchResult> = candidates
            .iter()
            .filter(|link| !shown.contains(link.id.as_str()))
            .take(QUICKLINK_FALLBACKS)
            .collect();
        if offered.len() == links {
            break (room, offered.into_iter().cloned().collect::<Vec<_>>());
        }
        links = offered.len();
    };
    results.extend(ranked.into_iter().take(room));
    if !keyword_search {
        results.push(web::fallback(query, s.settings.search_engine));
    }
    results.extend(links);
    results
}

/// Results computed from the query itself rather than looked up in an index.
fn answers(s: &Snapshot, query: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();
    results.extend(calculator::answer(query, Option::as_ref(&s.rates)));
    if !s.settings.currency_rates_enabled {
        results.extend(calculator::rates_off_answer(query));
    }
    results.extend(datetime::answers(query, s.now));
    results.extend(password::answers(query));
    results.extend(url_cleaner::answer(query));
    results.extend(clip_card::answer(query, s.now));
    results.extend(permissions::answer(query));
    results.extend(web::answer(query));
    results.extend(s.library.keyword_answer(query));
    results
}

/// The best `limit` results, each ID once, at its best score.
fn by_score(mut scored: Vec<Scored>, limit: usize) -> Vec<SearchResult> {
    scored.sort_by_key(|hit| Reverse(hit.score));
    let mut seen = HashSet::new();
    scored
        .into_iter()
        .map(|hit| hit.result)
        .filter(|result| seen.insert(result.id.clone()))
        .take(limit)
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
    fn a_currency_query_asks_for_rates_while_they_are_off() {
        let state = crate::state::State::for_tests(Settings::default());
        let results = search(&state.snapshot(), "100 usd to eur", Category::All);
        assert_eq!(results[0].title, "Currency rates are off");
        assert_eq!(results[0].actions[0].action, Action::TurnOnCurrencyRates);
        let math = search(&state.snapshot(), "12 * 8", Category::All);
        assert!(
            math.iter()
                .all(|result| result.title != "Currency rates are off")
        );
    }

    #[test]
    fn colors_unix_times_and_modes_are_answers_in_all() {
        let state = crate::state::State::for_tests(Settings::default());
        let answers = |query: &str| {
            search(&state.snapshot(), query, Category::All)
                .into_iter()
                .filter(|result| result.kind != ResultKind::WebSearch)
                .map(|result| (result.kind, result.title))
                .collect::<Vec<_>>()
        };
        assert_eq!(answers("#2f6f5e")[0], (ResultKind::Color, "#2F6F5E".into()));
        assert_eq!(
            answers("chmod 755"),
            [(ResultKind::Permissions, "rwxr-xr-x".into())]
        );
        let time = answers("1791354301");
        assert_eq!(time.len(), 1, "{time:?}");
        assert_eq!(time[0].0, ResultKind::DateTime);
        assert!(
            answers("755")
                .iter()
                .all(|(kind, _)| *kind != ResultKind::Permissions)
        );
    }

    #[test]
    fn a_source_kept_out_of_all_stays_in_its_tab() {
        use usage::Use;

        let mut settings = Settings::default();
        for tab in &mut settings.tabs {
            tab.in_all = tab.category != Category::Emoji;
        }
        let state = crate::state::State::for_tests(settings);
        let rocket = "emoji:🚀".to_owned();
        let ids = |query: &str, category| -> Vec<String> {
            search(&state.snapshot(), query, category)
                .into_iter()
                .map(|result| result.id)
                .collect()
        };

        assert!(
            !ids("rocket", Category::All)
                .iter()
                .any(|id| id.starts_with("emoji:"))
        );
        assert!(ids("rocket", Category::Emoji).contains(&rocket));
        // Not suggested either, but a pin is the user's own choice.
        let used = Use {
            count: 5,
            last_used: 0,
        };
        state.usage.set(Usage::new([(rocket.clone(), used)]));
        assert!(!ids("", Category::All).contains(&rocket));
        state.pins.set(Pins::new(vec![rocket.clone()]));
        assert_eq!(ids("", Category::All).first(), Some(&rocket));
    }

    #[test]
    fn the_start_screen_skips_pinned_clips() {
        use crate::features::clipboard::{ClipKind, Entry};

        let settings = Settings {
            clipboard_history_enabled: true,
            ..Settings::default()
        };
        let state = crate::state::State::for_tests(settings);
        let clip = Entry::new(
            7,
            ClipKind::Text,
            "meeting notes".into(),
            "meeting notes",
            0,
        );
        state.clipboard.set(ClipboardHistory::new(vec![clip]));
        let command = search(&state.snapshot(), "", Category::System)[0]
            .id
            .clone();
        state
            .pins
            .set(Pins::new(vec!["clip:7".into(), command.clone()]));
        let ids = |query: &str, category| -> Vec<String> {
            search(&state.snapshot(), query, category)
                .into_iter()
                .map(|result| result.id)
                .collect()
        };

        let start = ids("", Category::All);
        assert_eq!(start.first(), Some(&command), "{start:?}");
        assert!(!start.contains(&"clip:7".to_owned()));
        assert_eq!(
            ids("", Category::Clipboard).first().map(String::as_str),
            Some("clip:7")
        );
        assert!(ids("meeting", Category::All).contains(&"clip:7".to_owned()));
    }

    #[test]
    fn a_keyword_search_leaves_no_slot_for_the_fallback() {
        let state = crate::state::State::for_tests(Settings::default());
        let snapshot = state.snapshot();
        let keyword = search(&snapshot, "g a", Category::All);
        assert_eq!(keyword.len(), ALL_LIMIT);
        assert_eq!(keyword[0].kind, ResultKind::WebSearch);
        let plain = search(&snapshot, "a", Category::All);
        assert_eq!(plain.len(), ALL_LIMIT);
        assert_eq!(plain[ALL_LIMIT - 1].kind, ResultKind::WebSearch);
    }

    #[test]
    fn a_quicklink_shows_once_when_its_keyword_answers() {
        use crate::features::library::{LibraryItem, LibraryKind};
        let state = crate::state::State::for_tests(Settings::default());
        let item = LibraryItem {
            id: None,
            kind: LibraryKind::Quicklink,
            name: "Jira".into(),
            keyword: "jira".into(),
            text: "https://jira.test/browse/{query}".into(),
        };
        state.store.save_library_item(&item).unwrap();
        state.reload_library().unwrap();
        let results = search(&state.snapshot(), "jira a", Category::All);
        assert_eq!(results[0].title, "Jira: a");
        let mut ids: Vec<_> = results.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), results.len());
    }

    #[test]
    fn an_alias_lifts_its_result_once_in_all_and_in_its_tab() {
        let state = crate::state::State::for_tests(Settings::default());
        state
            .aliases
            .set(Aliases::new([("system:sleep".into(), "lo".into())]));
        let ids = |query: &str, category| -> Vec<String> {
            search(&state.snapshot(), query, category)
                .into_iter()
                .map(|result| result.id)
                .collect()
        };
        // "lo" is the alias of Sleep, and Lock Screen and Log Out start with it.
        for category in [Category::All, Category::System] {
            let found = ids("lo", category);
            assert_eq!(found[0], "system:sleep", "{found:?}");
            let sleeps = found.iter().filter(|id| *id == "system:sleep").count();
            assert_eq!(sleeps, 1);
        }
        assert!(!ids("lo", Category::Apps).contains(&"system:sleep".to_owned()));
        let sleep = search(&state.snapshot(), "sleep", Category::System);
        let labels: Vec<_> = sleep[0].actions.iter().map(|a| a.label.as_str()).collect();
        assert!(labels.contains(&"Change Alias “lo”…"), "{labels:?}");
        state.hidden.set(Hidden::new(["system:sleep".to_owned()]));
        assert!(!ids("lo", Category::All).contains(&"system:sleep".to_owned()));
    }

    #[test]
    fn quicklinks_that_take_the_query_follow_the_web_search() {
        use crate::features::library::{LibraryItem, LibraryKind};
        let state = crate::state::State::for_tests(Settings::default());
        for (name, keyword) in [("Jira", "jira"), ("Maps", ""), ("Wiki", ""), ("Docs", "")] {
            let item = LibraryItem {
                id: None,
                kind: LibraryKind::Quicklink,
                name: name.into(),
                keyword: keyword.into(),
                text: format!("https://{name}.test/{{query}}"),
            };
            state.store.save_library_item(&item).unwrap();
        }
        state.reload_library().unwrap();
        let titles = |query: &str| -> Vec<String> {
            search(&state.snapshot(), query, Category::All)
                .into_iter()
                .map(|result| result.title)
                .collect()
        };
        let found = titles("rust");
        assert_eq!(
            found[found.len() - 4..],
            [
                "Search Google for “rust”",
                "Docs: rust",
                "Jira: rust",
                "Maps: rust"
            ]
        );
        // A keyword chose where the text goes.
        assert!(
            !titles("g rust")
                .iter()
                .any(|title| title.ends_with(": rust"))
        );
        assert!(
            !titles("jira rust")
                .iter()
                .any(|title| title == "Maps: jira rust")
        );
        // A link that matched by name shows once, as itself.
        let wiki = titles("wiki");
        assert!(wiki.contains(&"Wiki".to_owned()));
        assert!(!wiki.contains(&"Wiki: wiki".to_owned()));
        // Rows with typed text leave aliases to the item's own row.
        let jira = search(&state.snapshot(), "rust", Category::All)
            .into_iter()
            .find(|result| result.title == "Jira: rust")
            .unwrap();
        let alias_action = |a: &ResultAction| matches!(a.action, Action::SetAlias { .. });
        assert!(!jira.actions.iter().any(alias_action));
        let docs = search(&state.snapshot(), "", Category::Snippets)[0]
            .id
            .clone();
        state.hidden.set(Hidden::new([docs]));
        assert!(!titles("rust").contains(&"Docs: rust".to_owned()));
        assert!(titles("rust").contains(&"Wiki: rust".to_owned()));
    }

    #[test]
    fn a_full_list_keeps_room_for_every_quicklink_it_offers() {
        use crate::features::library::{LibraryItem, LibraryKind};
        let state = crate::state::State::for_tests(Settings::default());
        // "a" matches more than a full list of emoji. "A Wiki" shows as a
        // name match; "Maps" matches by name too, below the cut, so it is
        // offered as a fallback instead.
        for name in ["A Wiki", "Docs", "Maps", "Notes"] {
            let item = LibraryItem {
                id: None,
                kind: LibraryKind::Quicklink,
                name: name.into(),
                keyword: String::new(),
                text: "https://x.test/{query}".into(),
            };
            state.store.save_library_item(&item).unwrap();
        }
        state.reload_library().unwrap();
        let results = search(&state.snapshot(), "a", Category::All);
        assert_eq!(results.len(), ALL_LIMIT);
        let tail: Vec<&str> = results[ALL_LIMIT - 3..]
            .iter()
            .map(|result| result.title.as_str())
            .collect();
        assert_eq!(tail, ["Docs: a", "Maps: a", "Notes: a"]);
        let titles: Vec<&str> = results.iter().map(|r| r.title.as_str()).collect();
        assert!(titles.contains(&"A Wiki"), "{titles:?}");
        assert!(!titles.contains(&"A Wiki: a"));
    }

    #[test]
    fn top_keeps_the_best_hits_in_order() {
        let hits = vec![(1, 'a'), (5, 'b'), (3, 'c'), (4, 'd')];
        assert_eq!(top(hits, 2), [(5, 'b'), (4, 'd')]);
        assert_eq!(top(vec![(1, 'a')], 5), [(1, 'a')]);
    }
}
