//! Web searches: `g rust`, `yt lo-fi`, `gh tauri`, and a fallback search at
//! the end of an All search that has no such keyword search. Nothing is
//! sent until the user opens it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol};

/// The engine for the fallback search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchEngine {
    #[default]
    Google,
    DuckDuckGo,
    Bing,
    Brave,
}

struct Engine {
    name: &'static str,
    keywords: &'static [&'static str],
    /// `{query}` is replaced by the encoded search text.
    template: &'static str,
}

const ENGINES: &[Engine] = &[
    Engine {
        name: "Google",
        keywords: &["g", "google"],
        template: "https://www.google.com/search?q={query}",
    },
    Engine {
        name: "DuckDuckGo",
        keywords: &["ddg", "duckduckgo"],
        template: "https://duckduckgo.com/?q={query}",
    },
    Engine {
        name: "Bing",
        keywords: &["bing"],
        template: "https://www.bing.com/search?q={query}",
    },
    Engine {
        name: "Brave",
        keywords: &["brave"],
        template: "https://search.brave.com/search?q={query}",
    },
    Engine {
        name: "YouTube",
        keywords: &["yt", "youtube"],
        template: "https://www.youtube.com/results?search_query={query}",
    },
    Engine {
        name: "GitHub",
        keywords: &["gh", "github"],
        template: "https://github.com/search?q={query}",
    },
    Engine {
        name: "Wikipedia",
        keywords: &["w", "wiki", "wikipedia"],
        template: "https://en.wikipedia.org/w/index.php?search={query}",
    },
];

impl SearchEngine {
    fn engine(self) -> &'static Engine {
        let name = match self {
            Self::Google => "Google",
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Bing => "Bing",
            Self::Brave => "Brave",
        };
        ENGINES
            .iter()
            .find(|engine| engine.name == name)
            .expect("every SearchEngine has an entry")
    }
}

/// An engine keyword followed by search text. A bare keyword is not a search,
/// so `gh` can still find an app.
pub fn answer(query: &str) -> Option<SearchResult> {
    let (keyword, text) = query.trim().split_once(char::is_whitespace)?;
    let text = text.trim();
    let keyword = keyword.to_lowercase();
    let engine = ENGINES
        .iter()
        .find(|engine| engine.keywords.contains(&keyword.as_str()))?;
    (!text.is_empty()).then(|| result(engine, text))
}

pub fn fallback(query: &str, engine: SearchEngine) -> SearchResult {
    result(engine.engine(), query.trim())
}

fn result(engine: &Engine, text: &str) -> SearchResult {
    let url = fill(engine.template, text);
    SearchResult {
        id: format!("web:{}:{text}", engine.name),
        kind: ResultKind::WebSearch,
        title: format!("Search {} for “{text}”", engine.name),
        subtitle: url.clone(),
        icon: Icon::Symbol {
            name: Symbol::Globe,
        },
        actions: vec![
            ResultAction::new("Open in Browser", Action::OpenUrl { url: url.clone() }),
            ResultAction::new("Copy URL", Action::Copy { text: url }),
        ],
        pinned: false,
    }
}

/// Replace `{query}` with the text, encoded with [`encode`].
fn fill(template: &str, text: &str) -> String {
    template.replace("{query}", &encode(text))
}

/// Percent-encode text so it stays one value in a URL path or query string
/// (a space becomes `%20`, never `+`).
pub fn encode(text: &str) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const VALUE: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    utf8_percent_encode(text, VALUE).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(result: &SearchResult) -> &str {
        match &result.actions[0].action {
            Action::OpenUrl { url } => url,
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn keywords_need_search_text() {
        assert_eq!(answer("gh"), None);
        assert_eq!(answer("gh  "), None);
        assert_eq!(
            opened(&answer("GH tauri app").unwrap()),
            "https://github.com/search?q=tauri%20app"
        );
        assert_eq!(answer("ghostty settings"), None);
    }

    #[test]
    fn encodes_the_text_as_one_value() {
        assert_eq!(
            fill("https://x.test/?q={query}", "a&b=c 東京"),
            "https://x.test/?q=a%26b%3Dc%20%E6%9D%B1%E4%BA%AC"
        );
        assert!(
            opened(&fallback("rust", SearchEngine::DuckDuckGo))
                .starts_with("https://duckduckgo.com/")
        );
    }
}
