use nucleo_matcher::{
    Config, Matcher,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::result::SearchResult;
use crate::{
    error::{Error, Result},
    providers::apps::{AppEntry, AppProvider},
    ranking,
};

pub const RESULT_LIMIT: usize = 30;

pub struct SearchManager {
    apps: AppProvider,
    matcher: Matcher,
}

impl Default for SearchManager {
    fn default() -> Self {
        Self {
            apps: AppProvider::default(),
            matcher: Matcher::new(Config::DEFAULT),
        }
    }
}

impl SearchManager {
    pub fn replace_apps(&mut self, apps: AppProvider) {
        self.apps = apps;
    }

    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    pub fn app(&self, id: &str) -> Result<AppEntry> {
        self.apps.get(id).cloned().ok_or(Error::AppNotFound)
    }

    pub fn search(&mut self, query: &str) -> Result<Vec<SearchResult>> {
        if query.chars().count() > 256 {
            return Err(Error::QueryTooLong);
        }
        let query = ranking::normalize(query);
        // Literal fuzzy words: punctuation is part of an app name, never query syntax.
        let pattern = Pattern::new(
            &query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let results = self.apps.search(&query, &pattern, &mut self.matcher);
        Ok(ranking::top_results(results, RESULT_LIMIT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> SearchManager {
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(vec![
            AppEntry::new(
                "Visual Studio Code".into(),
                "/apps/vscode.app".into(),
                vec!["code".into()],
            ),
            AppEntry::new("Code".into(), "/apps/Code.app".into(), vec![]),
            AppEntry::new("Xcode".into(), "/apps/Xcode.app".into(), vec![]),
            AppEntry::new("Café".into(), "/apps/cafe.app".into(), vec![]),
            AppEntry::new("Notes!".into(), "/apps/notes.app".into(), vec![]),
        ]));
        manager
    }

    #[test]
    fn exact_name_beats_alias_and_substring() {
        let results = manager().search("CODE").expect("search");
        assert_eq!(results[0].title, "Code");
        assert_eq!(results[1].title, "Visual Studio Code");
    }

    #[test]
    fn supports_fuzzy_words_aliases_accents_and_paths() {
        let mut manager = manager();
        assert_eq!(
            manager.search("vsc").expect("search")[0].title,
            "Visual Studio Code"
        );
        assert_eq!(
            manager.search(" studio   visual ").expect("search")[0].title,
            "Visual Studio Code"
        );
        assert_eq!(manager.search("cafe").expect("search")[0].title, "Café");
        assert_eq!(
            manager.search("/apps/vscode").expect("search")[0].title,
            "Visual Studio Code"
        );
    }

    #[test]
    fn punctuation_is_literal_and_unknown_apps_do_not_match() {
        let mut manager = manager();
        assert_eq!(manager.search("!").expect("search")[0].title, "Notes!");
        assert!(manager.search("zzzzzz").expect("search").is_empty());
        assert!(manager.app("/tmp/arbitrary-executable").is_err());
    }

    #[test]
    fn empty_query_is_deterministic_and_results_are_bounded() {
        let mut manager = manager();
        assert_eq!(manager.search(" \t ").expect("search")[0].title, "Café");
        manager.replace_apps(AppProvider::new(
            (0..100)
                .map(|i| AppEntry::new(format!("App {i:03}"), format!("/app/{i}").into(), vec![]))
                .collect(),
        ));
        assert_eq!(manager.search("").expect("search").len(), RESULT_LIMIT);
        assert!(manager.search(&"a".repeat(257)).is_err());
    }

    #[test]
    fn replaces_index_and_removes_duplicate_ids() {
        let mut manager = manager();
        let entry = AppEntry::new("Only app".into(), "/only".into(), vec![]);
        manager.replace_apps(AppProvider::new(vec![entry.clone(), entry]));
        assert_eq!(manager.app_count(), 1);
        assert!(manager.search("Code").expect("search").is_empty());
    }
}
