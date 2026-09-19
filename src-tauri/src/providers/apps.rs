use std::{collections::HashSet, path::PathBuf};

use nucleo_matcher::{Matcher, Utf32String, pattern::Pattern};

use crate::{
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

#[derive(Clone, Debug)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub aliases: Vec<String>,
    pub icon: Option<String>,
}

impl AppEntry {
    pub fn new(name: String, path: PathBuf, aliases: Vec<String>) -> Self {
        Self {
            id: format!("app:{}", path.to_string_lossy()),
            name,
            description: "Application".to_owned(),
            path,
            aliases,
            icon: None,
        }
    }

    pub fn result(&self, score: u32) -> SearchResult {
        SearchResult {
            id: self.id.clone(),
            kind: ResultKind::App,
            title: self.name.clone(),
            subtitle: self.description.clone(),
            path: Some(self.path.to_string_lossy().into_owned()),
            score,
            icon: None,
            primary_action: Action::Launch,
            secondary_actions: vec![Action::Reveal],
            pin: None,
            confirmation: None,
            detail: None,
        }
    }

    pub fn icon_payload(&self) -> Option<String> {
        #[cfg(test)]
        if let Some(icon) = &self.icon {
            ICON_COPIES.with(|counts| {
                let (count, bytes) = counts.get();
                counts.set((count + 1, bytes + icon.len()));
            });
        }
        self.icon.clone()
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static ICON_COPIES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

struct IndexedApp {
    entry: AppEntry,
    name: Utf32String,
    normalized_name: String,
    aliases: Vec<(Utf32String, String)>,
    path: Utf32String,
    hidden: bool,
}

#[derive(Default)]
pub struct AppProvider {
    apps: Vec<IndexedApp>,
}

impl AppProvider {
    pub fn new(mut entries: Vec<AppEntry>) -> Self {
        entries.sort_by_cached_key(|entry| (ranking::normalize(&entry.name), entry.id.clone()));
        let mut seen = HashSet::new();
        let apps = entries
            .into_iter()
            .filter(|entry| !entry.name.trim().is_empty() && seen.insert(entry.id.clone()))
            .map(|entry| IndexedApp {
                name: entry.name.as_str().into(),
                normalized_name: ranking::normalize(&entry.name),
                aliases: entry
                    .aliases
                    .iter()
                    .filter(|alias| !alias.trim().is_empty())
                    .map(|alias| (alias.as_str().into(), ranking::normalize(alias)))
                    .collect(),
                path: entry.path.to_string_lossy().as_ref().into(),
                hidden: false,
                entry,
            })
            .collect();
        Self { apps }
    }

    pub fn len(&self) -> usize {
        self.apps.len()
    }

    pub fn get(&self, id: &str) -> Option<&AppEntry> {
        self.apps
            .iter()
            .find(|app| app.entry.id == id && !app.hidden)
            .map(|app| &app.entry)
    }

    pub fn catalog(&self) -> Vec<SearchResult> {
        self.apps.iter().map(|app| app.entry.result(0)).collect()
    }

    pub fn apply_preferences(
        &mut self,
        preferences: &std::collections::BTreeMap<String, crate::settings::AppPreference>,
    ) {
        for app in &mut self.apps {
            let preference = preferences.get(&app.entry.id);
            app.hidden = preference.is_some_and(|value| value.hidden);
            app.aliases = app
                .entry
                .aliases
                .iter()
                .chain(preference.into_iter().flat_map(|value| &value.aliases))
                .filter(|alias| !alias.trim().is_empty())
                .map(|alias| (alias.as_str().into(), ranking::normalize(alias)))
                .collect();
        }
    }

    pub fn search(
        &self,
        query: &str,
        pattern: &Pattern,
        matcher: &mut Matcher,
    ) -> Vec<SearchResult> {
        let matches: Vec<_> = self
            .apps
            .iter()
            .filter(|app| !app.hidden)
            .filter_map(|app| {
                if query.is_empty() {
                    return Some((app, 0));
                }
                let name = pattern
                    .score(app.name.slice(..), matcher)
                    .map(|score| ranking::name_score(score, &app.normalized_name, query));
                let aliases = app
                    .aliases
                    .iter()
                    .filter_map(|(alias, normalized)| {
                        pattern.score(alias.slice(..), matcher).map(|score| {
                            ranking::name_score(score, normalized, query).saturating_sub(100)
                        })
                    })
                    .max();
                let path = pattern
                    .score(app.path.slice(..), matcher)
                    .map(|score| score / 2);
                name.into_iter()
                    .chain(aliases)
                    .chain(path)
                    .max()
                    .map(|score| (app, score))
            })
            .collect();
        matches
            .into_iter()
            .map(|(app, score)| app.entry.result(score))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nucleo_matcher::{
        Config, Matcher,
        pattern::{AtomKind, CaseMatching, Normalization},
    };

    fn search(provider: &AppProvider, query: &str) -> Vec<SearchResult> {
        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::new(
            &ranking::normalize(query),
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        provider.search(query, &pattern, &mut matcher)
    }

    #[test]
    fn exact_alias_beats_a_weaker_alias_match() {
        let provider = AppProvider::new(vec![
            AppEntry::new(
                "Notepad".into(),
                "/apps/notepad".into(),
                vec!["notes".into()],
            ),
            AppEntry::new("Noteshelf".into(), "/apps/noteshelf".into(), vec![]),
        ]);
        let results = search(&provider, "notes");
        assert_eq!(results[0].title, "Notepad");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn matches_literal_localized_names_and_aliases() {
        let provider = AppProvider::new(vec![
            AppEntry::new(
                "カレンダー".into(),
                "/apps/calendar-jp".into(),
                vec!["日历".into(), "Calendário".into()],
            ),
            AppEntry::new("Éditeur".into(), "/apps/editor-fr".into(), vec![]),
        ]);

        for (query, expected) in [
            ("カレンダー", "カレンダー"),
            ("日历", "カレンダー"),
            ("Calendário", "カレンダー"),
            ("éditeur", "Éditeur"),
        ] {
            let results = search(&provider, query);
            assert_eq!(
                results.first().map(|result| result.title.as_str()),
                Some(expected)
            );
        }
    }

    #[test]
    fn preferences_remove_aliases_and_restore_hidden_apps() {
        let entry = AppEntry::new("Editor".into(), "/apps/editor".into(), vec!["edit".into()]);
        let id = entry.id.clone();
        let mut provider = AppProvider::new(vec![entry]);
        let mut preferences = std::collections::BTreeMap::new();
        preferences.insert(
            id.clone(),
            crate::settings::AppPreference {
                aliases: vec!["write".into()],
                hidden: true,
            },
        );
        provider.apply_preferences(&preferences);
        assert!(provider.get(&id).is_none());
        assert!(search(&provider, "write").is_empty());

        provider.apply_preferences(&std::collections::BTreeMap::new());
        assert!(provider.get(&id).is_some());
        assert_eq!(search(&provider, "edit")[0].title, "Editor");
    }
}
