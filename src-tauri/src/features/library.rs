//! Snippets (text templates) and quicklinks (URL or path templates) that the
//! user saves in Settings.
//!
//! Both take the placeholders `{query}`, filled from text typed after the
//! keyword (`jira ABC-12`), `{clipboard}`, `{date}`, `{time}`, and
//! `{datetime}`.

use std::path::PathBuf;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    features::web,
    search::{
        Context,
        id::Source,
        matcher::Matcher,
        result::{Action, Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
        top,
    },
    settings,
};

/// Most snippets and quicklinks together.
pub const MAX_LIBRARY_ITEMS: usize = 500;
const MAX_NAME: usize = 100;
const MAX_KEYWORD: usize = 32;
const MAX_TEXT: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryKind {
    Snippet,
    Quicklink,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryItem {
    /// `None` for an item that is not saved yet.
    pub id: Option<i64>,
    pub kind: LibraryKind,
    pub name: String,
    /// Optional short word that finds the item; quicklinks read text after it.
    pub keyword: String,
    /// Snippet body, or quicklink URL or path template.
    pub text: String,
}

impl LibraryItem {
    fn result_id(&self) -> String {
        let source = match self.kind {
            LibraryKind::Snippet => Source::Snippet,
            LibraryKind::Quicklink => Source::Link,
        };
        source.id(self.id.unwrap_or_default())
    }

    /// Trim fields and reject items that could not work.
    pub fn validated(mut self) -> Result<Self> {
        self.name = self.name.trim().to_owned();
        self.keyword = self.keyword.trim().to_lowercase();
        if self.kind == LibraryKind::Quicklink {
            self.text = self.text.trim().to_owned();
        }
        if self.name.is_empty() || self.name.chars().count() > MAX_NAME {
            return Err(Error::msg(format!(
                "Enter a name of up to {MAX_NAME} characters."
            )));
        }
        if self.keyword.chars().count() > MAX_KEYWORD || self.keyword.contains(char::is_whitespace)
        {
            return Err(Error::msg(format!(
                "A keyword is one word of up to {MAX_KEYWORD} characters."
            )));
        }
        if self.text.trim().is_empty() || self.text.len() > MAX_TEXT {
            return Err(Error::msg("Enter the text, up to 32 KB."));
        }
        if self.kind == LibraryKind::Quicklink {
            quicklink_target(&self.text, "", Local::now(), || None)?;
        }
        Ok(self)
    }

    /// Text typed after its keyword fills it: a quicklink, or a snippet
    /// with `{query}`.
    fn takes_query(&self) -> bool {
        !self.keyword.is_empty()
            && (self.kind == LibraryKind::Quicklink || self.text.contains("{query}"))
    }
}

/// Where a quicklink goes once its `{query}` is filled.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Url(String),
    Path(PathBuf),
}

/// Fill a quicklink template. Only web and mail URLs and absolute or `~`
/// paths are allowed, so a quicklink cannot start a program by URL scheme.
/// The clipboard is read only when the template has `{clipboard}`.
pub fn quicklink_target(
    template: &str,
    query: &str,
    now: DateTime<Local>,
    clipboard: impl FnOnce() -> Option<String>,
) -> Result<Target> {
    let lower = template.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        let url = fill(template, query, now, clipboard, |value| {
            Ok(web::encode(value))
        })?;
        url::Url::parse(&url).map_err(|e| Error::msg(format!("Invalid quicklink URL: {e}")))?;
        return Ok(Target::Url(url));
    }
    // A value fills in a name; it must not climb out of the folder.
    let name = |value: &str| {
        if value.contains(['/', '\\']) || value.split_whitespace().any(|part| part == "..") {
            return Err(Error::msg("A path quicklink accepts a name, not a path."));
        }
        Ok(value.to_owned())
    };
    // `~` comes from the template alone, so no value can pick the home folder.
    let home = std::env::home_dir().unwrap_or_default();
    let template = settings::expand_home(template, &home)
        .to_string_lossy()
        .into_owned();
    let filled = fill(&template, query, now, clipboard, name)?;
    // The root (a drive, a UNC server and share, or `/`) comes from the
    // template alone: a value must not pick another disk or server.
    if root(&template) != root(&filled) {
        return Err(Error::msg(
            "A path quicklink starts with a fixed folder, such as ~/ or /.",
        ));
    }
    // A value holds no separator, so the parts of the template and of the
    // filled path line up. A part a value changed must not move up or stay
    // put, as `.{query}` with `.` would; Windows also drops trailing dots
    // and spaces. An empty part also stays put, which matters only before
    // a `..` of the template: that `..` would then climb higher than the
    // user wrote. A `..` the template itself has is the user's own choice.
    let separators = ['/', '\\'];
    let parts: Vec<(&str, &str)> = template
        .split(separators)
        .zip(filled.split(separators))
        .collect();
    let moves = parts.iter().enumerate().any(|(index, (part, done))| {
        let climbs_after = || parts[index + 1..].iter().any(|(later, _)| *later == "..");
        part != done
            && (done.trim_end_matches(['.', ' ']).is_empty() && !done.is_empty()
                || done.is_empty() && climbs_after())
    });
    if moves {
        return Err(Error::msg(
            "A path quicklink accepts a name, not “.” or “..”.",
        ));
    }
    let path = PathBuf::from(filled);
    if !path.is_absolute() {
        return Err(Error::msg(
            "A quicklink opens an http, https, or mailto URL, or an absolute or ~ path.",
        ));
    }
    Ok(Target::Path(path))
}

/// The drive or UNC prefix and the root folder that start a path.
fn root(path: &str) -> Vec<std::path::Component<'_>> {
    std::path::Path::new(path)
        .components()
        .take_while(|part| {
            matches!(
                part,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        })
        .collect()
}

/// Fill snippet placeholders. The clipboard is read only when needed.
pub fn render_snippet(
    text: &str,
    query: &str,
    now: DateTime<Local>,
    clipboard: impl FnOnce() -> Option<String>,
) -> String {
    fill(text, query, now, clipboard, |value| Ok(value.to_owned()))
        .expect("a snippet value is never refused")
}

/// Replace each placeholder in one pass, so that a value never fills in
/// turn: a query of `{clipboard}` stays that text. Unknown names stay as
/// written. `value` encodes or checks each value before it goes in.
fn fill(
    template: &str,
    query: &str,
    now: DateTime<Local>,
    clipboard: impl FnOnce() -> Option<String>,
    mut value: impl FnMut(&str) -> Result<String>,
) -> Result<String> {
    let mut clipboard = Some(clipboard);
    let mut copied = None;
    let mut filled = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        rest = &rest[start..];
        let name = rest[1..].split_once('}').map(|(name, _)| name);
        let text = match name {
            Some("query") => query.to_owned(),
            Some("date") => now.format("%Y-%m-%d").to_string(),
            Some("time") => now.format("%H:%M").to_string(),
            Some("datetime") => now.format("%Y-%m-%d %H:%M").to_string(),
            Some("clipboard") => copied
                .get_or_insert_with(|| clipboard.take().and_then(|read| read()).unwrap_or_default())
                .clone(),
            _ => {
                filled.push('{');
                rest = &rest[1..];
                continue;
            }
        };
        filled.push_str(&value(&text)?);
        // The name and its two braces.
        rest = &rest[name.map_or(0, str::len) + 2..];
    }
    filled.push_str(rest);
    Ok(filled)
}

#[derive(Default)]
pub struct Library {
    items: Vec<LibraryItem>,
}

impl Library {
    pub fn new(mut items: Vec<LibraryItem>) -> Self {
        items.sort_by_cached_key(|item| {
            (
                item.kind == LibraryKind::Quicklink,
                item.name.to_lowercase(),
            )
        });
        Self { items }
    }

    pub fn items(&self) -> &[LibraryItem] {
        &self.items
    }

    pub fn find(&self, id: i64) -> Option<&LibraryItem> {
        self.items.iter().find(|item| item.id == Some(id))
    }

    pub fn search(&self, matcher: &mut Matcher, ctx: &Context, limit: usize) -> Vec<Scored> {
        let hits = self
            .items
            .iter()
            .filter_map(|item| {
                let keyword = (!item.keyword.is_empty()).then_some(item.keyword.as_str());
                let score = matcher.best(&item.name, keyword)?;
                Some((score + ctx.boost(&item.result_id()), item))
            })
            .collect();
        top(hits, limit)
            .into_iter()
            .map(|(score, item)| Scored {
                score,
                result: result(item, "", ctx),
            })
            .collect()
    }

    pub fn browse(&self, ctx: &Context) -> Vec<SearchResult> {
        self.items
            .iter()
            .map(|item| result(item, "", ctx))
            .collect()
    }

    pub fn get(&self, id: i64, ctx: &Context) -> Option<SearchResult> {
        self.find(id).map(|item| result(item, "", ctx))
    }

    /// Quicklinks with `{query}` that can open the whole query, most used
    /// first, for the end of an All search.
    pub fn fallbacks(&self, query: &str, ctx: &Context, now: DateTime<Local>) -> Vec<SearchResult> {
        let mut links: Vec<&LibraryItem> = self
            .items
            .iter()
            .filter(|item| {
                item.kind == LibraryKind::Quicklink
                    && item.text.contains("{query}")
                    && quicklink_target(&item.text, query, now, || None).is_ok()
            })
            .collect();
        // Stable, so equally used links keep their name order.
        links.sort_by_key(|item| std::cmp::Reverse(ctx.boost(&item.result_id())));
        links.into_iter().map(|item| answer(item, query)).collect()
    }

    /// `keyword text` for a quicklink, or a snippet with `{query}`, whose
    /// keyword is the first word.
    pub fn keyword_answer(&self, query: &str) -> Option<SearchResult> {
        let (keyword, text) = query.split_once(char::is_whitespace)?;
        let keyword = keyword.to_lowercase();
        let item = self
            .items
            .iter()
            .find(|item| item.takes_query() && item.keyword == keyword)?;
        Some(answer(item, text.trim()))
    }
}

/// A result that runs `item` with the typed text. It has no Pin: a pin
/// keeps the item, not the text.
fn answer(item: &LibraryItem, query: &str) -> SearchResult {
    let mut result = result(item, query, &Context::none());
    result
        .actions
        .retain(|action| !matches!(action.action, Action::Pin { .. }));
    result
}

fn result(item: &LibraryItem, query: &str, ctx: &Context) -> SearchResult {
    let id = item.result_id();
    let item_id = item.id.unwrap_or_default();
    let (kind, symbol, subtitle, primary) = match item.kind {
        LibraryKind::Snippet => (
            ResultKind::Snippet,
            Symbol::Snippet,
            item.text
                .lines()
                .next()
                .unwrap_or_default()
                .replace("{query}", if query.is_empty() { "{query}" } else { query })
                .chars()
                .take(80)
                .collect(),
            ResultAction::new(
                "Copy Snippet",
                Action::CopySnippet {
                    id: item_id,
                    query: query.into(),
                },
            ),
        ),
        LibraryKind::Quicklink => (
            ResultKind::Quicklink,
            Symbol::Link,
            if query.is_empty() {
                item.text.clone()
            } else {
                item.text.replace("{query}", query)
            },
            ResultAction::new(
                "Open Quicklink",
                Action::OpenQuicklink {
                    id: item_id,
                    query: query.into(),
                },
            ),
        ),
    };
    let title = if query.is_empty() {
        item.name.clone()
    } else {
        format!("{}: {query}", item.name)
    };
    SearchResult {
        kind,
        title,
        subtitle,
        icon: Icon::Symbol { name: symbol },
        actions: vec![primary, ctx.pin_action(&id)],
        pinned: ctx.pinned(&id),
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn item(kind: LibraryKind, name: &str, keyword: &str, text: &str) -> LibraryItem {
        LibraryItem {
            id: Some(1),
            kind,
            name: name.into(),
            keyword: keyword.into(),
            text: text.into(),
        }
    }

    #[test]
    fn validation_trims_and_rejects_unsafe_quicklinks() {
        let ok = item(
            LibraryKind::Quicklink,
            " Jira ",
            " JIRA ",
            " https://jira.test/browse/{query} ",
        )
        .validated()
        .unwrap();
        assert_eq!((ok.name.as_str(), ok.keyword.as_str()), ("Jira", "jira"));
        assert!(item(LibraryKind::Snippet, "", "", "x").validated().is_err());
        assert!(
            item(LibraryKind::Snippet, "a", "two words", "x")
                .validated()
                .is_err()
        );
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ms-settings:",
            "relative/path",
        ] {
            assert!(
                item(LibraryKind::Quicklink, "a", "", bad)
                    .validated()
                    .is_err(),
                "{bad}"
            );
        }
        assert!(
            item(LibraryKind::Quicklink, "a", "", "~/Projects")
                .validated()
                .is_ok()
        );
        let now = Local.with_ymd_and_hms(2026, 10, 4, 9, 5, 0).unwrap();
        let target = |template: &str, query: &str| quicklink_target(template, query, now, || None);
        assert!(target("~/Projects/{query}", "../../Downloads/x.app").is_err());
        assert!(target("~/Projects/{query}", "notes").is_ok());
        let clipboard = || Some("../secret".to_owned());
        assert!(quicklink_target("~/Projects/{clipboard}", "", now, clipboard).is_err());
        let dot = || Some(".".to_owned());
        assert!(quicklink_target("~/Projects/.{clipboard}", "", now, dot).is_err());
        assert!(target("~/Projects/{query}{query}", ".").is_err());
        assert!(target("~/Projects/{query}", ".notes").is_ok());
        assert!(target("~/Projects/../Downloads/{query}", "x").is_ok());
        // Values cannot cancel the template's own `..` to climb higher.
        let template = "~/root/{clipboard}{clipboard}/x/{query}..";
        assert!(quicklink_target(template, "q", now, dot).is_err());
        // Nor can a value pick the root, such as the home folder or a
        // network server, or stay put with dots Windows drops.
        assert!(target("{query}/x", "~").is_err());
        assert!(target(r"\\{query}\share", "attacker").is_err());
        assert!(target("~/Notes/{query}", "...").is_err());
        assert!(target("~/Notes/{query}", ". ").is_err());
        assert!(target("~/Notes/{query}", "").is_ok());
        assert!(target("~/x/{query}/y", "").is_ok());
        // An empty value before the template's own `..` would climb higher.
        assert!(target("~/a/b/{query}/../c", "").is_err());
        assert!(target("~/a/b/{query}/../c", "n").is_ok());
        assert!(quicklink_target("~/a/b/{clipboard}/../../c", "", now, || None).is_err());
        assert!(
            item(LibraryKind::Quicklink, "a", "", "{query}/x")
                .validated()
                .is_err()
        );
    }

    #[test]
    fn fills_quicklinks_and_snippets() {
        let now = Local.with_ymd_and_hms(2026, 10, 4, 9, 5, 0).unwrap();
        assert_eq!(
            quicklink_target("https://x.test/?q={query}&d={date}", "a b", now, || None).unwrap(),
            Target::Url("https://x.test/?q=a%20b&d=2026-10-04".into())
        );
        assert_eq!(
            quicklink_target("https://x.test/{clipboard}", "", now, || Some("a/b".into())).unwrap(),
            Target::Url("https://x.test/a%2Fb".into())
        );
        let clip = || Some("clip".into());
        assert_eq!(
            render_snippet("{date} {time} {clipboard} {query}", "Sam", now, clip),
            "2026-10-04 09:05 clip Sam"
        );
        let mut read = false;
        render_snippet("no placeholders", "", now, || {
            read = true;
            None
        });
        assert!(!read);
    }

    #[test]
    fn values_never_fill_in_turn() {
        let now = Local.with_ymd_and_hms(2026, 10, 4, 9, 5, 0).unwrap();
        let mut reads = 0;
        let text = render_snippet(
            "{query} {clipboard}{clipboard} {x} {",
            "{clipboard}",
            now,
            || {
                reads += 1;
                Some("{date}".into())
            },
        );
        assert_eq!(text, "{clipboard} {date}{date} {x} {");
        assert_eq!(reads, 1);
        assert_eq!(render_snippet("{{query}}", "a", now, || None), "{a}");
    }

    #[test]
    fn keywords_take_the_rest_of_the_query() {
        let greeting = LibraryItem {
            id: Some(2),
            ..item(LibraryKind::Snippet, "Greeting", "hi", "Hello {query},")
        };
        let signature = LibraryItem {
            id: Some(3),
            ..item(LibraryKind::Snippet, "Sig", "sig", "Regards")
        };
        let library = Library::new(vec![
            item(
                LibraryKind::Quicklink,
                "Jira",
                "jira",
                "https://jira.test/browse/{query}",
            ),
            greeting,
            signature,
        ]);
        let answer = library.keyword_answer("jira ABC-12").unwrap();
        assert_eq!(answer.title, "Jira: ABC-12");
        assert_eq!(
            answer.actions[0].action,
            Action::OpenQuicklink {
                id: 1,
                query: "ABC-12".into()
            }
        );
        assert!(library.keyword_answer("jira").is_none());
        assert!(library.keyword_answer("jiraa x").is_none());
        let hello = library.keyword_answer("HI Sam").unwrap();
        assert_eq!(
            (hello.title.as_str(), hello.subtitle.as_str()),
            ("Greeting: Sam", "Hello Sam,")
        );
        assert_eq!(
            hello.actions[0].action,
            Action::CopySnippet {
                id: 2,
                query: "Sam".into()
            }
        );
        assert_eq!(hello.actions.len(), 1);
        // Its own row shows where the text goes.
        let own = library.get(2, &Context::none()).unwrap();
        assert_eq!(own.subtitle, "Hello {query},");
        // Without {query}, a snippet keyword takes no text.
        assert!(library.keyword_answer("sig Sam").is_none());
    }

    #[test]
    fn fallbacks_are_quicklinks_that_can_open_the_query() {
        let link = |id, name: &str, text: &str| LibraryItem {
            id: Some(id),
            ..item(LibraryKind::Quicklink, name, "", text)
        };
        let library = Library::new(vec![
            link(1, "Jira", "https://jira.test/browse/{query}"),
            link(2, "Docs", "https://docs.test/"),
            link(3, "Notes", "~/Notes/{query}"),
            item(LibraryKind::Snippet, "Hi", "", "Hello {query}"),
        ]);
        let now = Local::now();
        let titles = |query: &str| -> Vec<String> {
            library
                .fallbacks(query, &Context::none(), now)
                .into_iter()
                .map(|result| result.title)
                .collect()
        };
        assert_eq!(titles("ABC-12"), ["Jira: ABC-12", "Notes: ABC-12"]);
        // A path quicklink takes a name, not a path.
        assert_eq!(titles("a/b"), ["Jira: a/b"]);
        let jira = &library.fallbacks("x", &Context::none(), now)[0];
        assert!(
            !jira
                .actions
                .iter()
                .any(|a| matches!(a.action, Action::Pin { .. }))
        );
    }
}
