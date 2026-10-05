//! Snippets (text templates) and quicklinks (URL or path templates) that the
//! user saves in Settings.
//!
//! - Snippet placeholders: `{date}`, `{time}`, `{datetime}`, `{clipboard}`.
//! - Quicklink placeholder: `{query}`, filled from text typed after the
//!   keyword, for example `jira ABC-12`.

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
            quicklink_target(&self.text, "")?;
        }
        Ok(self)
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
pub fn quicklink_target(template: &str, query: &str) -> Result<Target> {
    let lower = template.to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        let url = web::fill(template, query);
        url::Url::parse(&url).map_err(|e| Error::msg(format!("Invalid quicklink URL: {e}")))?;
        return Ok(Target::Url(url));
    }
    // The query fills in a name; it must not climb out of the folder.
    if query.contains(['/', '\\']) || query.split_whitespace().any(|part| part == "..") {
        return Err(Error::msg("A path quicklink accepts a name, not a path."));
    }
    let path = template.replace("{query}", query);
    let path = match path.strip_prefix('~') {
        Some(rest) => std::env::home_dir()
            .ok_or_else(|| Error::msg("The home folder is unknown."))?
            .join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(path),
    };
    if !path.is_absolute() {
        return Err(Error::msg(
            "A quicklink opens an http, https, or mailto URL, or an absolute or ~ path.",
        ));
    }
    Ok(Target::Path(path))
}

/// Fill snippet placeholders. The clipboard is read only when needed.
pub fn render_snippet(
    text: &str,
    now: DateTime<Local>,
    clipboard: impl FnOnce() -> Option<String>,
) -> String {
    let mut text = text
        .replace("{datetime}", &now.format("%Y-%m-%d %H:%M").to_string())
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H:%M").to_string());
    if text.contains("{clipboard}") {
        text = text.replace("{clipboard}", &clipboard().unwrap_or_default());
    }
    text
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

    /// `keyword text` for a quicklink whose keyword is the first word.
    pub fn quicklink_answer(&self, query: &str) -> Option<SearchResult> {
        let (keyword, text) = query.split_once(char::is_whitespace)?;
        let keyword = keyword.to_lowercase();
        let item = self.items.iter().find(|item| {
            item.kind == LibraryKind::Quicklink
                && !item.keyword.is_empty()
                && item.keyword == keyword
        })?;
        let mut result = result(item, text.trim(), &Context::none());
        result
            .actions
            .retain(|action| !matches!(action.action, Action::Pin { .. }));
        Some(result)
    }
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
                .chars()
                .take(80)
                .collect(),
            ResultAction::new("Copy Snippet", Action::CopySnippet { id: item_id }),
        ),
        LibraryKind::Quicklink => (
            ResultKind::Quicklink,
            Symbol::Link,
            item.text.replace("{query}", query),
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
        assert!(quicklink_target("~/Projects/{query}", "../../Downloads/x.app").is_err());
        assert!(quicklink_target("~/Projects/{query}", "notes").is_ok());
    }

    #[test]
    fn fills_quicklinks_and_snippets() {
        assert_eq!(
            quicklink_target("https://x.test/?q={query}", "a b").unwrap(),
            Target::Url("https://x.test/?q=a%20b".into())
        );
        let now = Local.with_ymd_and_hms(2026, 10, 4, 9, 5, 0).unwrap();
        assert_eq!(
            render_snippet("{date} {time} {clipboard}", now, || Some("clip".into())),
            "2026-10-04 09:05 clip"
        );
        let mut read = false;
        render_snippet("no placeholders", now, || {
            read = true;
            None
        });
        assert!(!read);
    }

    #[test]
    fn quicklink_keywords_take_the_rest_of_the_query() {
        let library = Library::new(vec![item(
            LibraryKind::Quicklink,
            "Jira",
            "jira",
            "https://jira.test/browse/{query}",
        )]);
        let answer = library.quicklink_answer("jira ABC-12").unwrap();
        assert_eq!(answer.title, "Jira: ABC-12");
        assert_eq!(
            answer.actions[0].action,
            Action::OpenQuicklink {
                id: 1,
                query: "ABC-12".into()
            }
        );
        assert!(library.quicklink_answer("jira").is_none());
        assert!(library.quicklink_answer("jiraa x").is_none());
    }
}
