use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use nucleo_matcher::{
    Config, Matcher,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::{
    actions::ResolvedAction,
    pins::{Pins, QueryPin, ResultPin},
    query::{Query, SearchMode},
    result::{Action, SearchResult},
};
use crate::{
    currency::Rates,
    error::{Error, Result},
    providers::{
        apps::{AppEntry, AppProvider},
        calculator::CalculatorProvider,
        clipboard::{ClipboardEntry, ClipboardProvider},
        emoji::EmojiProvider,
        files::{FileEntry, FileProvider},
        system::SystemCommandProvider,
        tools::ToolProvider,
    },
    ranking,
};

pub const RESULT_LIMIT: usize = 30;

pub struct SearchManager {
    app_preferences: std::collections::BTreeMap<String, crate::settings::AppPreference>,
    apps: AppProvider,
    files: FileProvider,
    matcher: Matcher,
    emoji: Option<EmojiProvider>,
    system: Option<SystemCommandProvider>,
    calculator: CalculatorProvider,
    pub clipboard: ClipboardProvider,
    pub tools: ToolProvider,
    usage: HashMap<String, ranking::Usage>,
    pins: Pins,
    issued_pins: VecDeque<(String, String, SearchMode)>,
}

pub struct SearchOutcome {
    pub results: Vec<SearchResult>,
    pub notice: Option<String>,
}

impl Default for SearchManager {
    fn default() -> Self {
        Self {
            app_preferences: Default::default(),
            apps: AppProvider::default(),
            files: FileProvider::default(),
            matcher: Matcher::new(Config::DEFAULT),
            emoji: None,
            system: None,
            calculator: CalculatorProvider::default(),
            clipboard: ClipboardProvider::default(),
            tools: ToolProvider::default(),
            usage: HashMap::new(),
            pins: Pins::new(),
            issued_pins: VecDeque::new(),
        }
    }
}

impl SearchManager {
    pub fn apply_settings(&mut self, settings: &crate::settings::Settings) {
        self.app_preferences = settings.app_preferences.clone();
        self.apps.apply_preferences(&self.app_preferences);
        self.tools.set_web_searches(&settings.web_searches);
    }

    pub fn app_catalog(&self) -> Vec<SearchResult> {
        self.apps.catalog()
    }
    pub fn set_pins(&mut self, pins: Pins) {
        self.pins = pins;
    }

    pub fn set_pinned(&mut self, key: &str, category: SearchMode, pinned: bool) {
        let pins = self.pins.entry(category).or_default();
        if pinned {
            pins.insert(key.to_owned());
        } else {
            pins.remove(key);
        }
    }

    pub fn pin_key(&self, id: &str, category: SearchMode) -> Result<String> {
        let (_, key, own_category) = self
            .issued_pins
            .iter()
            .rev()
            .find(|(issued, _, _)| issued == id)
            .ok_or(Error::ResultExpired)?;
        if category != SearchMode::All && category != *own_category {
            return Err(Error::InvalidAction);
        }
        Ok(key.clone())
    }

    pub fn forget_clipboard_pins(&mut self, id: Option<i64>) {
        let removed = id.map(|id| format!("clipboard:{id}"));
        let keep = |key: &str| match &removed {
            Some(removed) => key != removed,
            None => !key.starts_with("clipboard:"),
        };
        for pins in self.pins.values_mut() {
            pins.retain(|key| keep(key));
        }
        self.issued_pins.retain(|(_, key, _)| keep(key));
    }

    fn describe_pin(&self, key: String) -> ResultPin {
        let mut categories: Vec<_> = self
            .pins
            .iter()
            .filter(|(_, keys)| keys.contains(&key))
            .map(|(mode, _)| *mode)
            .collect();
        categories.sort_by_key(|mode| mode.as_str());
        ResultPin { key, categories }
    }

    fn describe_results(&self, results: &mut [SearchResult], query: &Query<'_>) {
        let mut indices = HashMap::<SearchMode, usize>::new();
        for result in results {
            let mode = result.kind.category();
            let index = indices.entry(mode).or_default();
            let key = if result.kind.has_query_pin() {
                let password_query = self.tools.password_pin_query(&result.id);
                let web_pin = self.tools.web_pin(&result.id);
                QueryPin {
                    mode,
                    index: if password_query.is_some() || web_pin.is_some() {
                        0
                    } else {
                        *index
                    },
                    text: web_pin
                        .as_ref()
                        .map(|(_, text)| text.clone())
                        .or(password_query)
                        .unwrap_or_else(|| query.text.to_owned()),
                    web_keyword: web_pin.map(|(keyword, _)| keyword),
                }
                .key()
            } else {
                result.id.clone()
            };
            *index += 1;
            result.pin = Some(self.describe_pin(key));
        }
    }

    fn pinned_result(&mut self, key: &str) -> Option<SearchResult> {
        let result = if let Some(saved) = QueryPin::from_key(key) {
            if let Some(keyword) = &saved.web_keyword {
                if saved.mode != SearchMode::Web {
                    return None;
                }
                self.tools.search_web_pin(keyword, &saved.text)
            } else {
                let query = Query::parse(&saved.text, saved.mode).ok()?;
                if query.mode == SearchMode::Calculator {
                    self.calculator.search(query.text).ok()
                } else {
                    self.tools.search_pinned(&query, saved.index)
                }
            }
        } else if key.starts_with("app:") {
            self.apps.get(key).map(|app| app.result(0))
        } else if key.starts_with("file:") {
            self.files.get(key).map(|file| file.result(0))
        } else if key.starts_with("emoji:") {
            self.emoji
                .get_or_insert_with(EmojiProvider::default)
                .search(EmojiProvider::copy_value(key)?, &mut self.matcher)
                .into_iter()
                .find(|result| result.id == key)
        } else if key.starts_with("clipboard:") {
            self.clipboard
                .search("", &mut self.matcher)
                .into_iter()
                .find(|result| result.id == key)
        } else if key.starts_with("system:") {
            self.system
                .get_or_insert_with(SystemCommandProvider::default)
                .search("", &mut self.matcher)
                .into_iter()
                .find(|result| result.id == key)
        } else {
            None
        };
        result.map(|mut result| {
            result.pin = Some(self.describe_pin(key.to_owned()));
            result
        })
    }

    pub fn rates(&self) -> Option<&Rates> {
        self.calculator.rates.as_deref()
    }

    pub fn set_rates(&mut self, rates: Rates) -> bool {
        if self
            .rates()
            .is_some_and(|previous| previous.date > rates.date)
        {
            return false;
        }
        self.calculator.rates = Some(Arc::new(rates));
        true
    }

    pub fn set_usage(&mut self, usage: HashMap<String, ranking::Usage>) {
        self.usage = usage;
    }

    pub fn record_usage(&mut self, id: &str, now: i64) -> ranking::Usage {
        let usage = self.usage.entry(id.to_owned()).or_default();
        usage.count = usage.count.saturating_add(1);
        usage.last_used_at = now.max(0);
        *usage
    }

    pub fn replace_apps(&mut self, mut apps: AppProvider) {
        apps.apply_preferences(&self.app_preferences);
        self.apps = apps;
    }

    pub fn app_count(&self) -> usize {
        self.apps.len()
    }

    pub fn replace_files(&mut self, files: FileProvider) -> FileProvider {
        std::mem::replace(&mut self.files, files)
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn file(&self, id: &str) -> Result<FileEntry> {
        self.files.get(id).cloned().ok_or(Error::FileNotFound)
    }

    pub fn app(&self, id: &str) -> Result<AppEntry> {
        self.apps.get(id).cloned().ok_or(Error::AppNotFound)
    }

    pub fn clipboard_entry(&self, id: &str) -> Result<ClipboardEntry> {
        self.clipboard.get(id).cloned().ok_or(Error::ResultExpired)
    }

    pub fn resolve_action(&self, id: &str, action: Action) -> Result<ResolvedAction> {
        if id.starts_with("password:") || id.starts_with("tool:") {
            return self.tools.resolve(id, action);
        }
        match action {
            Action::Run => self
                .system
                .as_ref()
                .and_then(|provider| provider.get(id))
                .map(ResolvedAction::System)
                .ok_or(Error::InvalidAction),
            Action::Open | Action::Reveal if id.starts_with("file:") => {
                Ok(ResolvedAction::File(self.file(id)?, action))
            }
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
        let mut outcome = self.search_unpinned(&query);
        self.describe_results(&mut outcome.results, &query);
        if input.trim().is_empty() {
            let mut keys: Vec<_> = self
                .pins
                .get(&mode)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            keys.sort();
            let present: HashSet<_> = outcome
                .results
                .iter()
                .filter_map(|result| result.pin.as_ref().map(|pin| pin.key.clone()))
                .collect();
            // Resolve missing pins directly so file/provider limits cannot hide them.
            // Keep issued tool values bounded, even when a category has many pins.
            let mut added = 0;
            for key in keys.iter().filter(|key| !present.contains(*key)) {
                if added == RESULT_LIMIT {
                    break;
                }
                if let Some(result) = self.pinned_result(key) {
                    outcome.results.push(result);
                    added += 1;
                }
            }
            outcome.results.sort_by_key(|result| {
                !result
                    .pin
                    .as_ref()
                    .is_some_and(|pin| pin.categories.contains(&mode))
            });
            if !outcome.results.is_empty() {
                outcome.notice = None;
            }
        }
        // Pins remain first, but leave a place for the most recent clipboard
        // item even when pinned items fill the result limit.
        if mode == SearchMode::Clipboard
            && input.trim().is_empty()
            && let Some(newest) = self.clipboard.newest_id()
            && let Some(index) = outcome
                .results
                .iter()
                .position(|result| result.id == newest)
            && index >= RESULT_LIMIT
        {
            let latest = outcome.results.remove(index);
            outcome.results.truncate(RESULT_LIMIT - 1);
            outcome.results.push(latest);
        }
        outcome.results.truncate(RESULT_LIMIT);
        for result in &outcome.results {
            if let Some(pin) = &result.pin {
                self.issued_pins.push_back((
                    result.id.clone(),
                    pin.key.clone(),
                    result.kind.category(),
                ));
            }
        }
        while self.issued_pins.len() > RESULT_LIMIT * 4 {
            self.issued_pins.pop_front();
        }
        Ok(outcome)
    }

    fn search_unpinned(&mut self, query: &Query<'_>) -> SearchOutcome {
        if query.mode == SearchMode::Clipboard && query.text.is_empty() {
            return SearchOutcome {
                results: self
                    .clipboard
                    .search("", &mut self.matcher)
                    .into_iter()
                    .take(RESULT_LIMIT)
                    .collect(),
                notice: None,
            };
        }
        if let Some(outcome) = self.tools.search(query) {
            return match outcome {
                Ok(results) => SearchOutcome {
                    results: results.into_iter().take(RESULT_LIMIT).collect(),
                    notice: None,
                },
                Err(notice) => SearchOutcome {
                    results: vec![],
                    notice: Some(notice),
                },
            };
        }
        let mut results = Vec::new();
        let mut notice = None;
        if query.mode == SearchMode::Clipboard
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
            results.extend(self.clipboard.search(query.text, &mut self.matcher));
        }
        if query.mode == SearchMode::Apps
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
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
                Err(error)
                    if query.mode == SearchMode::Calculator
                        || matches!(
                            error,
                            crate::providers::calculator::CalculationError::Currency(_)
                        ) =>
                {
                    notice = Some(error.to_string())
                }
                Err(_) => {} // Ordinary app names and partial input are not calculator errors.
            }
        }
        if query.mode == SearchMode::System
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
            results.extend(
                self.system
                    .get_or_insert_with(SystemCommandProvider::default)
                    .search(query.text, &mut self.matcher),
            );
        }
        let now = ranking::now();
        ranking::apply_usage(&mut results, &self.usage, now);
        if query.mode == SearchMode::Files
            || (query.mode == SearchMode::All && !query.text.is_empty())
        {
            // File results include usage before their own top-N limit. This
            // preserves global ranking without allocating a result per file.
            results.extend(self.files.search(
                query.text,
                &mut self.matcher,
                &self.usage,
                now,
                RESULT_LIMIT,
            ));
        }
        SearchOutcome {
            results: ranking::top_results(results, RESULT_LIMIT),
            notice,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_precede_the_result_limit_but_do_not_change_typed_search() {
        let mut manager = SearchManager::default();
        let apps = || {
            AppProvider::new(
                (0..100)
                    .map(|i| {
                        AppEntry::new(format!("App {i:03}"), format!("/app/{i}").into(), vec![])
                    })
                    .collect(),
            )
        };
        manager.replace_apps(apps());
        for mode in [SearchMode::All, SearchMode::Apps] {
            manager.set_pinned("app:/app/99", mode, true);
            manager.set_pinned("app:/missing", mode, true);
        }
        for mode in [SearchMode::All, SearchMode::Apps] {
            let results = manager.search("  ", mode).unwrap().results;
            assert_eq!(
                results.len(),
                if mode == SearchMode::All {
                    1
                } else {
                    RESULT_LIMIT
                }
            );
            assert_eq!(results[0].title, "App 099");
            assert!(!results.iter().any(|result| result.id == "app:/missing"));
            assert_eq!(
                manager.search("App 000", mode).unwrap().results[0].title,
                "App 000"
            );
        }
        manager.replace_apps(apps());
        assert_eq!(
            manager.search("", SearchMode::Apps).unwrap().results[0].title,
            "App 099"
        );
        manager.set_pinned("app:/app/99", SearchMode::Apps, false);
        assert_eq!(
            manager.search("", SearchMode::Apps).unwrap().results[0].title,
            "App 000"
        );
    }

    #[test]
    fn bare_web_aliases_find_apps_and_search_text_selects_the_engine() {
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(vec![
            AppEntry::new("Ghostty".into(), "/apps/Ghostty.app".into(), vec![]),
            AppEntry::new("Brave Browser".into(), "/apps/Brave.app".into(), vec![]),
            AppEntry::new("Google Chrome".into(), "/apps/Chrome.app".into(), vec![]),
            AppEntry::new("GitHub Desktop".into(), "/apps/GitHub.app".into(), vec![]),
        ]));

        for (input, app) in [
            ("gh", "Ghostty"),
            ("g", "Ghostty"),
            ("gh   ", "Ghostty"),
            (" GH ", "Ghostty"),
            ("brave", "Brave Browser"),
            ("google", "Google Chrome"),
            ("github", "GitHub Desktop"),
        ] {
            for mode in [SearchMode::Apps, SearchMode::All] {
                let outcome = manager.search(input, mode).unwrap();
                assert!(
                    outcome.results.iter().any(|result| result.title == app),
                    "{input:?} must find {app} in {mode:?}; notice: {:?}",
                    outcome.notice
                );
                assert!(outcome.notice.is_none());
            }
        }

        for input in ["gh rust", " GH  rust ", "gh\trust"] {
            let outcome = manager.search(input, SearchMode::All).unwrap();
            assert!(outcome.notice.is_none());
            assert_eq!(outcome.results.len(), 1);
            assert!(matches!(
                manager.resolve_action(&outcome.results[0].id, Action::Open).unwrap(),
                ResolvedAction::OpenUrl(url) if url == "https://github.com/search?q=rust"
            ));
        }

        let explicit_web = manager.search("gh", SearchMode::Web).unwrap();
        assert!(explicit_web.results.is_empty());
        assert!(explicit_web.notice.is_some());
    }

    #[test]
    fn tools_are_scoped_and_actions_copy_issued_values() {
        let mut manager = manager();
        for input in [
            "password 32",
            "time in tokyo",
            "https://example.com/?utm_source=test",
            "web rust",
        ] {
            assert!(
                !manager
                    .search(input, SearchMode::All)
                    .unwrap()
                    .results
                    .is_empty(),
                "{input}"
            );
            assert!(
                manager
                    .search(input, SearchMode::Apps)
                    .unwrap()
                    .results
                    .is_empty(),
                "{input}"
            );
        }
        let passwords = manager
            .search("password 32", SearchMode::All)
            .unwrap()
            .results;
        assert_eq!(passwords.len(), 4);
        let password = &passwords[1];
        assert!(
            matches!(manager.resolve_action(&password.id, Action::Copy).unwrap(), ResolvedAction::Copy(value) if value == password.title)
        );
        let refreshed = manager
            .search("password 32", SearchMode::All)
            .unwrap()
            .results;
        assert_eq!(refreshed[1].id, password.id);
        manager.tools.regenerate(&password.id).unwrap();
        let regenerated = manager
            .search("password 32", SearchMode::All)
            .unwrap()
            .results;
        assert_ne!(regenerated[1].id, password.id);
        assert!(
            matches!(manager.resolve_action(&password.id, Action::Copy).unwrap(), ResolvedAction::Copy(value) if value == password.title)
        );
        assert!(manager.resolve_action(&password.id, Action::Open).is_err());
        let cleaned = &manager
            .search(
                "https://example.com/?q=keep&utm_source=test",
                SearchMode::All,
            )
            .unwrap()
            .results[0];
        assert!(
            matches!(manager.resolve_action(&cleaned.id, Action::Open).unwrap(), ResolvedAction::OpenUrl(value) if value == "https://example.com/?q=keep")
        );
        assert!(
            manager
                .resolve_action(&cleaned.id, Action::Regenerate)
                .is_err()
        );
        assert!(manager.resolve_action("tool:forged", Action::Open).is_err());
    }

    #[test]
    fn returning_to_password_search_generates_new_values_without_changing_issued_actions() {
        let mut manager = manager();
        let first = manager
            .search("password 32", SearchMode::All)
            .unwrap()
            .results;
        manager.search("", SearchMode::All).unwrap();
        let next = manager
            .search("password 32", SearchMode::All)
            .unwrap()
            .results;
        assert_eq!(first.len(), next.len());
        for (previous, current) in first.iter().zip(&next) {
            assert_ne!(previous.id, current.id);
            assert!(matches!(
                manager.resolve_action(&previous.id, Action::Copy).unwrap(),
                ResolvedAction::Copy(value) if value == previous.title
            ));
        }
    }

    #[test]
    fn tool_cache_is_bounded_and_long_urls_do_not_raise_the_app_query_limit() {
        let mut manager = manager();
        let first = manager
            .search("web original", SearchMode::All)
            .unwrap()
            .results[0]
            .id
            .clone();
        for index in 0..20 {
            manager
                .search(&format!("web query {index}"), SearchMode::All)
                .unwrap();
        }
        assert!(manager.resolve_action(&first, Action::Open).is_err());
        let long_url = format!(
            "https://example.com/?token={}&utm_source=test",
            "a".repeat(2000)
        );
        assert_eq!(
            manager
                .search(&long_url, SearchMode::All)
                .unwrap()
                .results
                .len(),
            1
        );
        assert!(manager.search(&"a".repeat(257), SearchMode::All).is_err());
        assert!(
            manager
                .search(
                    &format!("https://example.com/{}", "a".repeat(9000)),
                    SearchMode::All
                )
                .is_err()
        );
    }

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
    fn system_search_is_lazy_scoped_and_uses_durable_ranking() {
        use crate::providers::system::SystemCommand;
        let mut manager = manager();
        manager.search("", SearchMode::All).expect("home");
        assert!(manager.system.is_none());
        manager.system = Some(SystemCommandProvider::new(SystemCommand::ALL.to_vec()));
        assert!(
            manager
                .search("reboot", SearchMode::Apps)
                .expect("apps")
                .results
                .is_empty()
        );
        assert_eq!(
            manager
                .search("reboot", SearchMode::All)
                .expect("all")
                .results[0]
                .id,
            "system:restart"
        );
        assert_eq!(
            manager
                .search("", SearchMode::System)
                .expect("commands")
                .results
                .len(),
            5
        );
        manager.record_usage("system:settings", ranking::now());
        assert_eq!(
            manager
                .search("", SearchMode::System)
                .expect("ranked")
                .results[0]
                .id,
            "system:settings"
        );
        assert!(
            manager
                .search("unknown-command", SearchMode::System)
                .expect("no match")
                .results
                .is_empty()
        );
        assert!(matches!(
            manager.resolve_action("system:restart", Action::Run),
            Ok(ResolvedAction::System(SystemCommand::Restart))
        ));
        for (id, action) in [
            ("system:arbitrary", Action::Run),
            ("system:restart", Action::Launch),
            ("system:restart", Action::Copy),
            ("app:/apps/Code.app", Action::Run),
        ] {
            assert!(manager.resolve_action(id, action).is_err());
        }
        manager.system = Some(SystemCommandProvider::new(vec![SystemCommand::Settings]));
        assert!(
            manager
                .resolve_action("system:restart", Action::Run)
                .is_err()
        );
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
    fn empty_all_query_shows_no_unpinned_apps() {
        let mut manager = manager();
        for input in ["", " \t "] {
            assert!(
                manager
                    .search(input, SearchMode::All)
                    .unwrap()
                    .results
                    .is_empty()
            );
        }
        assert!(
            !manager
                .search("code", SearchMode::All)
                .unwrap()
                .results
                .is_empty()
        );
        assert!(
            !manager
                .search("", SearchMode::Apps)
                .unwrap()
                .results
                .is_empty()
        );
    }

    #[test]
    fn empty_query_is_deterministic_and_results_are_bounded() {
        let mut manager = manager();
        assert_eq!(
            manager
                .search(" \t ", SearchMode::Apps)
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
                .search("", SearchMode::Apps)
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
    fn currency_errors_are_visible_in_all_mode_and_older_rates_cannot_replace_the_cache() {
        let mut manager = manager();
        let query = "100 USD to MYR";
        assert_eq!(
            manager
                .search(query, SearchMode::All)
                .expect("missing rates")
                .notice,
            Some(crate::currency::Error::Unavailable.to_string())
        );
        let mut rates = crate::currency::fixture();
        assert!(manager.set_rates(rates.clone()));
        assert_eq!(
            manager
                .search(query, SearchMode::All)
                .expect("cached conversion")
                .results[0]
                .title,
            "400 MYR"
        );
        rates.date = "2026-09-15".into();
        rates.values.insert("MYR".into(), 6.0);
        assert!(!manager.set_rates(rates));
        assert_eq!(
            manager
                .search(query, SearchMode::Calculator)
                .expect("retained rates")
                .results[0]
                .title,
            "400 MYR"
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

    #[test]
    fn newest_clipboard_entry_survives_a_full_page_of_older_pins_and_usage() {
        let mut search = SearchManager {
            clipboard: ClipboardProvider::new(
                (1..=500)
                    .rev()
                    .map(|id| ClipboardEntry {
                        id,
                        content: format!("Saved text {id}"),
                        created_at: id,
                        last_used_at: None,
                    })
                    .collect(),
            ),
            ..SearchManager::default()
        };
        for id in 1..=40 {
            let key = format!("clipboard:{id}");
            search.set_pinned(&key, SearchMode::Clipboard, true);
            search.record_usage(&key, ranking::now());
        }
        let results = search.search("", SearchMode::Clipboard).unwrap().results;
        assert_eq!(results.len(), RESULT_LIMIT);
        assert_eq!(results.last().unwrap().id, "clipboard:500");
        assert!(results[..RESULT_LIMIT - 1].iter().all(|result| {
            result
                .pin
                .as_ref()
                .unwrap()
                .categories
                .contains(&SearchMode::Clipboard)
        }));
        assert_eq!(
            search
                .search("Saved text 499", SearchMode::Clipboard)
                .unwrap()
                .results[0]
                .id,
            "clipboard:499"
        );
        assert_eq!(
            search.clipboard.newest_id().as_deref(),
            Some("clipboard:500")
        );
    }

    #[test]
    fn custom_web_pins_keep_their_engine_when_searches_are_reordered_or_disabled() {
        use crate::settings::{Settings, WebSearch};
        let custom = |keyword: &str| WebSearch {
            name: "Same name".into(),
            keyword: keyword.into(),
            enabled: true,
            template: format!("https://{keyword}.example/search?q={{query}}"),
        };
        let mut settings = Settings {
            web_searches: vec![custom("first"), custom("second")],
            ..Settings::default()
        };
        let mut search = SearchManager::default();
        search.apply_settings(&settings);
        let results = search
            .search("web coffee & 東京", SearchMode::Web)
            .unwrap()
            .results;
        assert_eq!(results.len(), 8);
        let chosen = &results[7];
        let key = search.pin_key(&chosen.id, SearchMode::All).unwrap();
        search.set_pinned(&key, SearchMode::All, true);
        settings.web_searches.reverse();
        search.apply_settings(&settings);
        assert!(search.resolve_action(&chosen.id, Action::Open).is_err());
        let pinned = search
            .search("", SearchMode::All)
            .unwrap()
            .results
            .remove(0);
        assert_eq!(pinned.pin.as_ref().unwrap().key, key);
        let ResolvedAction::OpenUrl(url) = search.resolve_action(&pinned.id, Action::Open).unwrap()
        else {
            panic!("web action");
        };
        let target = url::Url::parse(&url).unwrap();
        assert_eq!(target.host_str(), Some("second.example"));
        assert_eq!(target.query_pairs().next().unwrap().1, "coffee & 東京");
        settings.web_searches[0].enabled = false;
        search.apply_settings(&settings);
        assert!(
            search
                .search("", SearchMode::All)
                .unwrap()
                .results
                .is_empty()
        );
        settings.web_searches[0].enabled = true;
        let mut restored = SearchManager::default();
        restored.apply_settings(&settings);
        restored.set_pinned(&key, SearchMode::All, true);
        assert_eq!(
            restored.search("", SearchMode::All).unwrap().results.len(),
            1
        );
        settings.web_searches.clear();
        restored.apply_settings(&settings);
        assert!(
            restored
                .search("", SearchMode::All)
                .unwrap()
                .results
                .is_empty()
        );
    }
}
