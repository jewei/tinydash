use url::Url;

pub struct Engine {
    pub name: &'static str,
    aliases: &'static [&'static str],
    endpoint: &'static str,
    parameter: &'static str,
}
pub struct SearchTarget {
    pub engine: String,
    pub text: String,
    pub url: String,
    pub keyword: Option<String>,
}
pub const ENGINES: &[Engine] = &[
    Engine {
        name: "Google",
        aliases: &["google", "g"],
        endpoint: "https://www.google.com/search",
        parameter: "q",
    },
    Engine {
        name: "DuckDuckGo",
        aliases: &["duckduckgo", "ddg"],
        endpoint: "https://duckduckgo.com/",
        parameter: "q",
    },
    Engine {
        name: "Bing",
        aliases: &["bing"],
        endpoint: "https://www.bing.com/search",
        parameter: "q",
    },
    Engine {
        name: "Brave",
        aliases: &["brave"],
        endpoint: "https://search.brave.com/search",
        parameter: "q",
    },
    Engine {
        name: "YouTube",
        aliases: &["youtube", "yt"],
        endpoint: "https://www.youtube.com/results",
        parameter: "search_query",
    },
    Engine {
        name: "GitHub",
        aliases: &["github", "gh"],
        endpoint: "https://github.com/search",
        parameter: "q",
    },
];

pub fn reserved_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "web"
            | "search"
            | "password"
            | "passphrase"
            | "pin"
            | "time"
            | "date"
            | "datetime"
            | "clean"
            | "url"
            | "today"
            | "tomorrow"
            | "yesterday"
            | "now"
            | "next"
            | "in"
    ) || ENGINES
        .iter()
        .any(|engine| engine.aliases.contains(&keyword))
}

pub fn custom_candidate(query: &str, custom: &[crate::settings::WebSearch]) -> bool {
    query
        .split_once(char::is_whitespace)
        .is_some_and(|(first, rest)| {
            !rest.trim().is_empty()
                && custom
                    .iter()
                    .any(|search| search.enabled && search.keyword.eq_ignore_ascii_case(first))
        })
}

pub fn keyword_available(keyword: &str, custom: &[crate::settings::WebSearch]) -> bool {
    ENGINES
        .iter()
        .any(|engine| engine.aliases.contains(&keyword))
        || custom
            .iter()
            .any(|search| search.enabled && search.keyword == keyword)
}

pub fn searches_with_custom(
    query: &str,
    custom: &[crate::settings::WebSearch],
) -> Result<Vec<SearchTarget>, String> {
    let (first, rest) = query.split_once(char::is_whitespace).unwrap_or((query, ""));
    if let Some(search) = custom
        .iter()
        .find(|search| search.enabled && search.keyword.eq_ignore_ascii_case(first))
    {
        if rest.trim().is_empty() {
            return Err(format!("Enter text after {}.", search.keyword));
        }
        let url = search.url(rest.trim()).map_err(|error| error.to_string())?;
        return Ok(vec![SearchTarget {
            engine: search.name.clone(),
            text: rest.trim().into(),
            url,
            keyword: Some(search.keyword.clone()),
        }]);
    }
    let built_in = searches(query)?;
    let mut results: Vec<_> = built_in
        .into_iter()
        .map(|(name, text, url)| SearchTarget {
            engine: name.to_owned(),
            text,
            url,
            keyword: None,
        })
        .collect();
    if !ENGINES.iter().any(|engine| {
        engine
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(first))
    }) {
        let text = if matches!(first.to_ascii_lowercase().as_str(), "web" | "search") {
            rest.trim()
        } else {
            query.trim()
        };
        for search in custom.iter().filter(|search| search.enabled) {
            results.push(SearchTarget {
                engine: search.name.clone(),
                text: text.into(),
                url: search.url(text).map_err(|error| error.to_string())?,
                keyword: Some(search.keyword.clone()),
            });
        }
    }
    Ok(results)
}

pub fn is_candidate(query: &str) -> bool {
    let mut words = query.split_whitespace();
    let first = words.next().unwrap_or("").to_ascii_lowercase();
    matches!(first.as_str(), "web" | "search")
        || (words.next().is_some()
            && ENGINES
                .iter()
                .any(|engine| engine.aliases.contains(&first.as_str())))
}

pub fn searches(query: &str) -> Result<Vec<(&'static str, String, String)>, String> {
    let (first, remainder) = query.split_once(char::is_whitespace).unwrap_or((query, ""));
    let first = first.to_ascii_lowercase();
    let engine = ENGINES
        .iter()
        .find(|engine| engine.aliases.contains(&first.as_str()));
    let text = if engine.is_some() || matches!(first.as_str(), "web" | "search") {
        remainder.trim()
    } else {
        query.trim()
    };
    if text.is_empty() {
        return Err("Enter a search, such as web coffee, google rust, or github tauri.".into());
    }
    Ok(ENGINES
        .iter()
        .filter(|candidate| engine.is_none_or(|chosen| chosen.name == candidate.name))
        .map(|engine| {
            let mut url = Url::parse(engine.endpoint).expect("fixed search endpoint");
            url.query_pairs_mut().append_pair(engine.parameter, text);
            (engine.name, text.into(), url.into())
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn engine_aliases_require_search_text_but_explicit_commands_allow_help() {
        for engine in ENGINES {
            for alias in engine.aliases {
                assert!(!is_candidate(alias), "{alias}");
                assert!(!is_candidate(&format!("  {alias} \t")), "{alias}");
                assert!(is_candidate(&format!("{alias} example")), "{alias}");
            }
        }
        for command in ["web", "search", "web rust", "search rust"] {
            assert!(is_candidate(command), "{command}");
        }
    }

    #[test]
    fn provides_six_engines_and_encodes_search_text_as_one_parameter() {
        let searches = searches("web rust & c++ #東京").unwrap();
        assert_eq!(searches.len(), 6);
        for (_, _, target) in searches {
            let url = Url::parse(&target).unwrap();
            assert_eq!(url.query_pairs().count(), 1);
            assert_eq!(url.query_pairs().next().unwrap().1, "rust & c++ #東京");
            assert!(url.fragment().is_none());
        }
    }
    #[test]
    fn aliases_select_one_engine_and_empty_searches_never_open() {
        for engine in ENGINES {
            for alias in engine.aliases {
                let result = searches(&format!("{alias} example")).unwrap();
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].0, engine.name);
            }
        }
        assert!(searches("google").is_err());
        assert!(searches("web   ").is_err());
    }
}
