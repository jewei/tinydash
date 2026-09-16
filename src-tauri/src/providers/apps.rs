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
    pub path: PathBuf,
    pub aliases: Vec<String>,
}

impl AppEntry {
    pub fn new(name: String, path: PathBuf, aliases: Vec<String>) -> Self {
        Self {
            id: format!("app:{}", path.to_string_lossy()),
            name,
            path,
            aliases,
        }
    }
}

struct IndexedApp {
    entry: AppEntry,
    name: Utf32String,
    normalized_name: String,
    aliases: Vec<(Utf32String, String)>,
    path: Utf32String,
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
            .find(|app| app.entry.id == id)
            .map(|app| &app.entry)
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
            .map(|(app, score)| SearchResult {
                id: app.entry.id.clone(),
                kind: ResultKind::App,
                title: app.entry.name.clone(),
                subtitle: app.entry.path.to_string_lossy().into_owned(),
                score,
                icon: None,
                primary_action: Action::Launch,
                secondary_actions: vec![Action::Reveal],
            })
            .collect()
    }
}
