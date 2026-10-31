//! `?` lists the words that start something in an All search: the built-in
//! ones, the web keywords, and the keywords of the user's snippets and
//! quicklinks. `? words` lists those whose row holds every word. Enter
//! types the keyword into the search field, ready for its text.

use crate::{
    features::{
        library::{Library, LibraryKind},
        web,
    },
    search::{
        matcher::normalize,
        result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol},
    },
};

/// Rows in the list. Up to 500 snippets and quicklinks can have keywords,
/// so `? words` finds the ones past it.
const ROWS: usize = 100;

/// The longest text a row types: a keyword of up to 32 characters and a
/// space, with room to spare. `actions.rs` refuses longer text.
pub const MAX_TYPED: usize = 64;

/// A built-in way to start a query. `search/mod.rs` tests that each
/// `example` gets an answer, so this list cannot drift from the features.
struct BuiltIn {
    keywords: &'static str,
    what: &'static str,
    example: &'static str,
    /// What Enter types: the keyword, with a space when text follows it.
    typed: &'static str,
    symbol: Symbol,
}

const BUILT_IN: &[BuiltIn] = &[
    BuiltIn {
        keywords: "=",
        what: "Calculations you copied",
        example: "= cm",
        typed: "=",
        symbol: Symbol::Calculator,
    },
    BuiltIn {
        keywords: "now, time in",
        what: "The time here or in a city",
        example: "time in tokyo",
        typed: "time in ",
        symbol: Symbol::Clock,
    },
    BuiltIn {
        keywords: "today, in",
        what: "Count days from a date",
        example: "today + 3 days",
        typed: "today + ",
        symbol: Symbol::Clock,
    },
    BuiltIn {
        keywords: "days until, days since, days between",
        what: "Count the days to or from a date",
        example: "days until dec 25",
        typed: "days until ",
        symbol: Symbol::Clock,
    },
    BuiltIn {
        keywords: "password, pw",
        what: "A password, a passphrase, and a PIN",
        example: "pw 32",
        typed: "pw ",
        symbol: Symbol::Key,
    },
    BuiltIn {
        keywords: "passphrase",
        what: "A passphrase of random words",
        example: "passphrase 8",
        typed: "passphrase ",
        symbol: Symbol::Key,
    },
    BuiltIn {
        keywords: "pin",
        what: "A PIN of random digits",
        example: "pin 4",
        typed: "pin ",
        symbol: Symbol::Key,
    },
    BuiltIn {
        keywords: "chmod",
        what: "File permissions, both ways",
        example: "chmod 755",
        typed: "chmod ",
        symbol: Symbol::Lock,
    },
];

/// The examples of the built-in rows, for the test that each one answers.
#[cfg(test)]
pub fn examples() -> impl Iterator<Item = &'static str> {
    BUILT_IN.iter().map(|entry| entry.example)
}

/// `?` or `? words` in All; `None` for any other query.
pub fn answer(query: &str, library: &Library) -> Option<Vec<SearchResult>> {
    let filter = query.trim().strip_prefix('?')?;
    // `?x` is not this list; a space must follow, as for `=`.
    if !(filter.is_empty() || filter.starts_with(char::is_whitespace)) {
        return None;
    }
    let words: Vec<String> = normalize(filter)
        .split_whitespace()
        .map(String::from)
        .collect();
    let built_in = BUILT_IN.iter().map(|entry| {
        row(
            format!("keyword:{}", entry.keywords),
            entry.keywords,
            format!("{} · {}", entry.what, entry.example),
            entry.typed,
            entry.symbol,
        )
    });
    let engines = web::engines().map(|(name, keywords)| {
        let first = keywords[0];
        row(
            format!("keyword:{name}"),
            &keywords.join(", "),
            format!("Search {name} · {first} rust"),
            &format!("{first} "),
            Symbol::Globe,
        )
    });
    let mut own: Vec<_> = library
        .items()
        .iter()
        .filter(|item| !item.keyword.is_empty())
        .collect();
    own.sort_by(|a, b| a.keyword.cmp(&b.keyword));
    let own = own.into_iter().map(|item| {
        let symbol = match item.kind {
            LibraryKind::Snippet => Symbol::Snippet,
            LibraryKind::Quicklink => Symbol::Link,
        };
        let (what, typed) = if item.takes_query() {
            (
                format!("{} with the text after it", item.name),
                format!("{} ", item.keyword),
            )
        } else {
            (item.name.clone(), item.keyword.clone())
        };
        // Its own ID: a keyword may match a built-in one, such as `pin`.
        let id = format!("keyword:item:{}", item.id.unwrap_or_default());
        row(id, &item.keyword, what, &typed, symbol)
    });
    Some(
        built_in
            .chain(engines)
            .chain(own)
            .filter(|result| {
                let text = format!("{} {}", result.title, result.subtitle).to_lowercase();
                // Each word starts a word of the row, so `git` finds GitHub
                // but not "digits".
                let row: Vec<&str> = text
                    .split(|c: char| c.is_whitespace() || matches!(c, ',' | '·'))
                    .collect();
                words
                    .iter()
                    .all(|word| row.iter().any(|part| part.starts_with(word.as_str())))
            })
            .take(ROWS)
            .collect(),
    )
}

fn row(id: String, keywords: &str, subtitle: String, typed: &str, symbol: Symbol) -> SearchResult {
    SearchResult {
        id,
        kind: ResultKind::Keyword,
        title: keywords.into(),
        subtitle,
        icon: Icon::Symbol { name: symbol },
        actions: vec![ResultAction::new(
            "Use Keyword",
            Action::TypeQuery { text: typed.into() },
        )],
        pinned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::library::LibraryItem;

    fn item(id: i64, kind: LibraryKind, name: &str, keyword: &str, text: &str) -> LibraryItem {
        LibraryItem {
            id: Some(id),
            kind,
            name: name.into(),
            keyword: keyword.into(),
            text: text.into(),
        }
    }

    fn typed(result: &SearchResult) -> &str {
        match &result.actions[0].action {
            Action::TypeQuery { text } => text,
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn a_question_mark_lists_built_in_web_and_own_keywords() {
        let library = Library::new(vec![
            item(
                1,
                LibraryKind::Quicklink,
                "Jira",
                "jira",
                "https://jira.test/{query}",
            ),
            item(2, LibraryKind::Snippet, "Address", "addr", "1 Main St"),
            item(3, LibraryKind::Snippet, "No keyword", "", "text"),
            item(4, LibraryKind::Snippet, "Bank PIN", "pin", "1234"),
        ]);
        let rows = answer("?", &library).unwrap();
        let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles[0], "=");
        assert!(titles.contains(&"g, google"));
        // The user's own keywords come last, by keyword.
        assert_eq!(&titles[titles.len() - 3..], ["addr", "jira", "pin"]);
        let jira = &rows[rows.len() - 2];
        assert_eq!(jira.subtitle, "Jira with the text after it");
        assert_eq!(typed(jira), "jira ");
        // A keyword that only finds its item is typed without a space.
        assert_eq!(typed(&rows[rows.len() - 3]), "addr");
        // Every ID is different, also for `pin`, built in and the user's own,
        // so the list can keep its selection.
        let mut ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), rows.len());
    }

    #[test]
    fn words_after_the_question_mark_filter_the_list() {
        let library = Library::new(vec![item(
            1,
            LibraryKind::Quicklink,
            "Jira",
            "jira",
            "https://jira.test/{query}",
        )]);
        let rows = answer("?  JIRA ", &library).unwrap();
        assert_eq!(rows.len(), 1);
        let rows = answer("? search git", &library).unwrap();
        assert_eq!(rows[0].title, "gh, github");
        // A word matches the start of a word: "digits" in the PIN row has
        // "git" inside it, not at its start.
        let titles: Vec<String> = answer("? git", &library)
            .unwrap()
            .into_iter()
            .map(|row| row.title)
            .collect();
        assert_eq!(titles, ["gh, github"]);
        assert_eq!(answer("? =", &library).unwrap()[0].title, "=");
        assert_eq!(typed(&rows[0]), "gh ");
        assert!(answer("? nothing-like-this", &library).unwrap().is_empty());
    }

    #[test]
    fn words_match_in_either_unicode_form() {
        let library = Library::new(vec![item(
            1,
            LibraryKind::Snippet,
            "Caf\u{e9} menu",
            "menu",
            "Soup",
        )]);
        // Pasted text can hold decomposed accents.
        let rows = answer("? cafe\u{301}", &library).unwrap();
        let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles, ["menu"]);
    }

    #[test]
    fn each_built_in_row_types_the_start_of_its_example() {
        for entry in BUILT_IN {
            assert!(
                entry.example.starts_with(entry.typed.trim_end()),
                "{}",
                entry.example
            );
            assert!(entry.typed.chars().count() <= MAX_TYPED);
        }
    }

    #[test]
    fn other_queries_are_not_the_list() {
        let library = Library::new(Vec::new());
        for query in ["", "?x", "what?"] {
            assert!(answer(query, &library).is_none(), "{query}");
        }
    }
}
