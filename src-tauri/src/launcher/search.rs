use std::collections::HashMap;

use nucleo_matcher::{
    Config, Matcher,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::{
    actions::ResolvedAction,
    query::{Query, SearchMode},
    result::{Action, SearchResult},
};
use crate::{
    error::{Error, Result},
    providers::{
        apps::{AppEntry, AppProvider},
        calculator::CalculatorProvider,
        clipboard::{ClipboardEntry, ClipboardProvider},
        emoji::EmojiProvider,
    },
    ranking,
};

pub const RESULT_LIMIT: usize = 30;

pub struct SearchManager {
    apps: AppProvider,
    matcher: Matcher,
    emoji: Option<EmojiProvider>,
    calculator: CalculatorProvider,
    pub clipboard: ClipboardProvider,
    usage: HashMap<String, ranking::Usage>,
}

pub struct SearchOutcome {
    pub results: Vec<SearchResult>,
    pub notice: Option<String>,
}

impl Default for SearchManager {
    fn default() -> Self {
        Self {
            apps: AppProvider::default(),
            matcher: Matcher::new(Config::DEFAULT),
            emoji: None,
            calculator: CalculatorProvider::default(),
            clipboard: ClipboardProvider::default(),
            usage: HashMap::new(),
        }
    }
}

impl SearchManager {
    pub fn set_usage(&mut self, usage: HashMap<String, ranking::Usage>) {
        self.usage = usage;
    }

    pub fn record_usage(&mut self, id: &str, now: i64) -> ranking::Usage {
        let usage = self.usage.entry(id.to_owned()).or_default();
        usage.count = usage.count.saturating_add(1);
        usage.last_used_at = now.max(0);
        *usage
    }

    pub fn replace_apps(&mut self, apps: AppProvider) {
        self.apps = apps;
    }

    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    pub fn app(&self, id: &str) -> Result<AppEntry> {
        self.apps.get(id).cloned().ok_or(Error::AppNotFound)
    }

    pub fn clipboard_entry(&self, id: &str) -> Result<ClipboardEntry> {
        self.clipboard.get(id).cloned().ok_or(Error::ResultExpired)
    }

    pub fn resolve_action(&self, id: &str, action: Action) -> Result<ResolvedAction> {
        match action {
            Action::Launch if id.starts_with("app:") => Ok(ResolvedAction::Launch(self.app(id)?)),
            Action::Reveal if id.starts_with("app:") => {
                Ok(ResolvedAction::Reveal(self.app(id)?.path))
            }
            Action::Copy if id.starts_with("emoji:") => EmojiProvider::copy_value(id)
                .map(|value| ResolvedAction::Copy(value.to_owned()))
                .ok_or(Error::ResultExpired),
            Action::Copy if id.starts_with("calculation:") => self
                .calculator
                .copy_value(id)
                .map(|value| ResolvedAction::Copy(value.to_owned()))
                .ok_or(Error::ResultExpired),
            Action::Copy if id.starts_with("clipboard:") => {
                Ok(ResolvedAction::Copy(self.clipboard_entry(id)?.content))
            }
            Action::Delete if id.starts_with("clipboard:") => {
                Ok(ResolvedAction::Delete(self.clipboard_entry(id)?.id))
            }
            _ => Err(Error::InvalidAction),
        }
    }

    pub fn search(&mut self, input: &str, mode: SearchMode) -> Result<SearchOutcome> {
        let query = Query::parse(input, mode)?;
        let mut results = Vec::new();
        let mut notice = None;
        if query.mode == SearchMode::Clipboard
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
            results.extend(self.clipboard.search(query.text, &mut self.matcher));
        }
        if matches!(query.mode, SearchMode::All | SearchMode::Apps) {
            let normalized = ranking::normalize(query.text);
            // App punctuation remains literal. Calculator input retains its case.
            let pattern = Pattern::new(
                &normalized,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            results.extend(self.apps.search(&normalized, &pattern, &mut self.matcher));
        }
        if query.mode == SearchMode::Emoji
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
            results.extend(
                self.emoji
                    .get_or_insert_with(EmojiProvider::default)
                    .search(query.text, &mut self.matcher),
            );
        }
        if !query.text.is_empty()
            && (query.mode == SearchMode::Calculator
                || (query.mode == SearchMode::All && CalculatorProvider::is_candidate(query.text)))
        {
            match self.calculator.search(query.text) {
                Ok(result) => results.push(result),
                Err(error) if query.mode == SearchMode::Calculator => {
                    notice = Some(error.to_string())
                }
                Err(_) => {} // Ordinary app names and partial input are not calculator errors.
            }
        }
        ranking::apply_usage(&mut results, &self.usage, ranking::now());
        Ok(SearchOutcome {
            results: ranking::top_results(results, RESULT_LIMIT),
            notice,
        })
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
        let results = manager()
            .search("CODE", SearchMode::All)
            .expect("search")
            .results;
        assert_eq!(results[0].title, "Code");
        assert_eq!(results[1].title, "Visual Studio Code");
    }

    #[test]
    fn supports_fuzzy_words_aliases_accents_and_paths() {
        let mut manager = manager();
        assert_eq!(
            manager
                .search("vsc", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Visual Studio Code"
        );
        assert_eq!(
            manager
                .search(" studio   visual ", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Visual Studio Code"
        );
        assert_eq!(
            manager
                .search("cafe", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Café"
        );
        assert_eq!(
            manager
                .search("/apps/vscode", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Visual Studio Code"
        );
    }

    #[test]
    fn punctuation_is_literal_and_unknown_apps_do_not_match() {
        let mut manager = manager();
        assert_eq!(
            manager
                .search("!", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Notes!"
        );
        assert!(
            manager
                .search("zzzzzz", SearchMode::All)
                .expect("search")
                .results
                .is_empty()
        );
        assert!(manager.app("/tmp/arbitrary-executable").is_err());
    }

    #[test]
    fn empty_query_is_deterministic_and_results_are_bounded() {
        let mut manager = manager();
        assert_eq!(
            manager
                .search(" \t ", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Café"
        );
        manager.replace_apps(AppProvider::new(
            (0..100)
                .map(|i| AppEntry::new(format!("App {i:03}"), format!("/app/{i}").into(), vec![]))
                .collect(),
        ));
        assert_eq!(
            manager
                .search("", SearchMode::All)
                .expect("search")
                .results
                .len(),
            RESULT_LIMIT
        );
        assert!(manager.search(&"a".repeat(257), SearchMode::All).is_err());
    }

    #[test]
    fn replaces_index_and_removes_duplicate_ids() {
        let mut manager = manager();
        let entry = AppEntry::new("Only app".into(), "/only".into(), vec![]);
        manager.replace_apps(AppProvider::new(vec![entry.clone(), entry]));
        assert_eq!(manager.app_count(), 1);
        assert!(
            manager
                .search("Code", SearchMode::Apps)
                .expect("search")
                .results
                .is_empty()
        );
    }

    #[test]
    fn clipboard_is_searchable_in_all_and_its_own_mode_and_validates_actions() {
        let mut manager = manager();
        manager.clipboard = ClipboardProvider::new(vec![ClipboardEntry {
            id: 3,
            content: "Meeting notes\nKeep these spaces.  🚀\n".into(),
            created_at: 100,
            last_used_at: None,
        }]);
        let history = manager
            .search("", SearchMode::Clipboard)
            .expect("history")
            .results;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, "clipboard:3");
        assert_eq!(
            manager
                .search("meeting", SearchMode::All)
                .expect("all")
                .results[0]
                .id,
            "clipboard:3"
        );
        assert!(
            manager
                .search("meeting", SearchMode::Apps)
                .expect("apps")
                .results
                .is_empty()
        );
        assert!(
            !manager
                .search("", SearchMode::All)
                .expect("home")
                .results
                .iter()
                .any(|result| result.id.starts_with("clipboard:"))
        );
        assert!(
            matches!(manager.resolve_action("clipboard:3", Action::Copy), Ok(ResolvedAction::Copy(text)) if text.ends_with("  🚀\n"))
        );
        assert!(matches!(
            manager.resolve_action("clipboard:3", Action::Delete),
            Ok(ResolvedAction::Delete(3))
        ));
        assert!(
            manager
                .resolve_action("clipboard:3", Action::Launch)
                .is_err()
        );
        assert!(manager.resolve_action("emoji:🚀", Action::Delete).is_err());
        manager.clipboard.remove(3);
        assert!(manager.resolve_action("clipboard:3", Action::Copy).is_err());
        assert!(manager.clipboard_entry("clipboard:3").is_err());
    }

    #[test]
    fn combines_providers_and_respects_explicit_modes() {
        let mut manager = manager();
        assert!(manager.emoji.is_none());
        manager.search("", SearchMode::All).expect("search");
        assert!(manager.emoji.is_none(), "empty startup does not load emoji");
        let calculation = manager.search("12 * 8", SearchMode::All).expect("search");
        assert_eq!(calculation.results[0].title, "96");
        assert_eq!(calculation.results[0].primary_action, Action::Copy);
        assert!(
            manager
                .search("12 * 8", SearchMode::Apps)
                .expect("search")
                .results
                .is_empty()
        );
        for mode in [SearchMode::All, SearchMode::Emoji] {
            assert_eq!(
                manager.search(":rocket:", mode).expect("search").results[0]
                    .icon
                    .as_deref(),
                Some("🚀")
            );
        }
        assert!(
            manager
                .search("Code", SearchMode::Calculator)
                .expect("search")
                .results
                .is_empty()
        );
        assert_eq!(
            manager
                .search("", SearchMode::Emoji)
                .expect("search")
                .results
                .len(),
            RESULT_LIMIT
        );
    }

    #[test]
    fn reports_calculator_errors_only_in_explicit_calculator_searches() {
        let mut manager = manager();
        assert!(
            manager
                .search("12 +", SearchMode::All)
                .expect("search")
                .notice
                .is_none()
        );
        assert!(
            manager
                .search("=12 +", SearchMode::All)
                .expect("search")
                .notice
                .is_some()
        );
        assert!(
            manager
                .search("12 +", SearchMode::Calculator)
                .expect("search")
                .notice
                .is_some()
        );
        assert!(
            manager
                .search("", SearchMode::Calculator)
                .expect("search")
                .notice
                .is_none()
        );
    }

    #[test]
    fn actions_validate_kind_and_copy_the_issued_value() {
        let mut manager = manager();
        let result = manager
            .search("12 * 8", SearchMode::All)
            .expect("search")
            .results
            .remove(0);
        manager
            .search("1 + 1", SearchMode::All)
            .expect("new search");
        assert!(
            matches!(manager.resolve_action(&result.id, Action::Copy), Ok(ResolvedAction::Copy(value)) if value == "96")
        );
        assert!(manager.resolve_action(&result.id, Action::Launch).is_err());
        assert!(
            manager
                .resolve_action("calculation:forged", Action::Copy)
                .is_err()
        );
        assert!(
            manager
                .resolve_action("app:/apps/Code.app", Action::Copy)
                .is_err()
        );
        assert!(manager.resolve_action("emoji:🚀", Action::Reveal).is_err());
        assert!(
            matches!(manager.resolve_action("emoji:🚀", Action::Copy), Ok(ResolvedAction::Copy(value)) if value == "🚀")
        );
    }

    #[test]
    fn usage_changes_empty_and_fuzzy_results_but_cannot_add_nonmatches() {
        let mut manager = manager();
        let id = "app:/apps/Xcode.app";
        let now = ranking::now();
        manager.record_usage(id, now);
        assert_eq!(
            manager
                .search("", SearchMode::Apps)
                .expect("search")
                .results[0]
                .id,
            id
        );
        assert_eq!(
            manager
                .search("xc", SearchMode::Apps)
                .expect("search")
                .results[0]
                .id,
            id
        );
        assert!(
            manager
                .search("unknown-application", SearchMode::Apps)
                .expect("search")
                .results
                .is_empty()
        );
        assert_eq!(
            manager
                .search("code", SearchMode::Apps)
                .expect("search")
                .results[0]
                .title,
            "Code"
        );
        assert_eq!(manager.record_usage(id, now).count, 2);
        manager.set_usage(HashMap::from([(
            id.into(),
            ranking::Usage {
                count: u32::MAX,
                last_used_at: now,
            },
        )]));
        assert_eq!(manager.record_usage(id, now).count, u32::MAX);
        assert_eq!(
            manager
                .search("12 * 8", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "96"
        );
    }

    #[test]
    fn a_new_search_manager_ranks_from_persisted_usage() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("tinydash.sqlite3");
        let now = ranking::now();
        let app_id = "app:/apps/Xcode.app";
        {
            let mut search = manager();
            let database = crate::db::Database::open(&path).expect("open");
            for _ in 0..3 {
                let usage = search.record_usage(app_id, now);
                database.save_usage(app_id, usage).expect("save");
            }
            let usage = search.record_usage("emoji:🚀", now);
            database.save_usage("emoji:🚀", usage).expect("save emoji");
        }
        let database = crate::db::Database::open(&path).expect("reopen");
        let mut search = manager();
        search.set_usage(database.load_usage().expect("load"));
        assert_eq!(
            search.search("", SearchMode::Apps).expect("search").results[0].id,
            app_id
        );
        assert_eq!(
            search.search(":", SearchMode::All).expect("emoji").results[0].id,
            "emoji:🚀"
        );
        assert_eq!(search.record_usage(app_id, now).count, 4);
    }
}
