use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::query::SearchMode;

pub type Pins = HashMap<SearchMode, HashSet<String>>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPin {
    pub key: String,
    pub categories: Vec<SearchMode>,
}

// Persist the input and selected output, never a temporary result ID or password.
#[derive(Deserialize, Serialize)]
pub struct QueryPin {
    pub mode: SearchMode,
    pub text: String,
    pub index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_keyword: Option<String>,
}

impl QueryPin {
    pub fn key(&self) -> String {
        format!(
            "query:{}",
            serde_json::to_string(self).expect("serializable query pin")
        )
    }

    pub fn from_key(key: &str) -> Option<Self> {
        serde_json::from_str(key.strip_prefix("query:")?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::Database,
        launcher::{actions::ResolvedAction, result::Action, search::SearchManager},
        providers::{
            apps::{AppEntry, AppProvider},
            clipboard::{ClipboardEntry, ClipboardProvider},
            files::{ScanReport, scan},
        },
    };

    fn manager(directory: &std::path::Path) -> SearchManager {
        let mut manager = SearchManager::default();
        manager.replace_apps(AppProvider::new(
            (0..100)
                .map(|index| {
                    AppEntry::new(
                        format!("App {index:03}"),
                        format!("/app/{index}").into(),
                        vec![],
                    )
                })
                .collect(),
        ));
        manager.replace_files(scan(
            vec![directory.to_owned()],
            &[],
            1000,
            &mut ScanReport::default(),
        ));
        manager.clipboard = ClipboardProvider::new(vec![ClipboardEntry {
            id: 1,
            content: "Saved text".into(),
            created_at: 1,
            last_used_at: None,
        }]);
        manager
    }

    #[test]
    fn every_category_has_independent_durable_pins_with_working_actions() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..100 {
            std::fs::write(directory.path().join(format!("file-{index:03}.txt")), "").unwrap();
        }
        for (mode, query, index) in [
            (SearchMode::Apps, "App 099", 0),
            (SearchMode::Files, "file-099.txt", 0),
            (SearchMode::Clipboard, "Saved text", 0),
            (SearchMode::Emoji, "rocket", 0),
            (SearchMode::Calculator, "12 * 8", 0),
            (SearchMode::System, "settings", 0),
            (SearchMode::Password, "password 16", 1),
            (SearchMode::Timezone, "time in Tokyo", 0),
            (
                SearchMode::Url,
                "https://example.com/page?utm_source=test",
                0,
            ),
            (SearchMode::Web, "web rust language", 2),
        ] {
            let store = tempfile::tempdir().unwrap();
            let path = store.path().join("pins.sqlite3");
            let database = Database::open(&path).unwrap();
            let mut search = manager(directory.path());
            let original = search.search(query, mode).unwrap().results[index].clone();
            let key = search.pin_key(&original.id, mode).unwrap();
            assert!(
                search
                    .pin_key(
                        &original.id,
                        if mode == SearchMode::Apps {
                            SearchMode::Files
                        } else {
                            SearchMode::Apps
                        }
                    )
                    .is_err()
            );
            assert!(search.pin_key("forged", mode).is_err());
            assert_ne!(
                key, original.title,
                "A generated value must not become the saved key"
            );
            database.set_pinned(&key, mode, true).unwrap();
            search.set_pinned(&key, mode, true);
            assert!(
                !search
                    .search("", SearchMode::All)
                    .unwrap()
                    .results
                    .iter()
                    .any(|result| {
                        result
                            .pin
                            .as_ref()
                            .unwrap()
                            .categories
                            .contains(&SearchMode::All)
                    })
            );
            database.set_pinned(&key, SearchMode::All, true).unwrap();
            drop(database);

            // Load into a new manager, with new calculation and tool result IDs.
            let database = Database::open(&path).unwrap();
            let mut restored = manager(directory.path());
            restored.set_pins(database.load_pins().unwrap());
            for category in [mode, SearchMode::All] {
                let outcome = restored.search("", category).unwrap();
                let pinned = &outcome.results[0];
                assert_eq!(
                    pinned.pin.as_ref().unwrap().key,
                    key,
                    "{mode:?} in {category:?}"
                );
                assert!(pinned.pin.as_ref().unwrap().categories.contains(&category));
                assert_eq!(pinned.kind, original.kind);
                assert!(
                    restored
                        .resolve_action(&pinned.id, pinned.primary_action)
                        .is_ok(),
                    "{mode:?}"
                );
                assert_eq!(
                    outcome
                        .results
                        .iter()
                        .filter(|result| result.pin.as_ref().unwrap().key == key)
                        .count(),
                    1
                );
            }
            assert!(
                !restored
                    .search("unmatchedzzzzz", SearchMode::All)
                    .unwrap()
                    .results
                    .iter()
                    .any(|result| { result.pin.as_ref().unwrap().key == key })
            );
            restored.set_pinned(&key, SearchMode::All, false);
            assert!(
                restored
                    .search("", SearchMode::All)
                    .unwrap()
                    .results
                    .iter()
                    .all(|result| {
                        !result
                            .pin
                            .as_ref()
                            .unwrap()
                            .categories
                            .contains(&SearchMode::All)
                    })
            );
            assert_eq!(
                restored.search("", mode).unwrap().results[0]
                    .pin
                    .as_ref()
                    .unwrap()
                    .categories,
                [mode]
            );
        }
    }

    #[test]
    fn pinned_password_generators_keep_the_displayed_value_and_can_regenerate() {
        let mut search = SearchManager::default();
        let original = search
            .search("password letters 16", SearchMode::All)
            .unwrap()
            .results
            .remove(0);
        let key = search.pin_key(&original.id, SearchMode::All).unwrap();
        search.set_pinned(&key, SearchMode::All, true);
        for _ in 0..3 {
            let pinned = search
                .search("", SearchMode::All)
                .unwrap()
                .results
                .remove(0);
            assert_eq!(pinned.id, original.id);
            assert_eq!(pinned.title, original.title);
        }
        search.tools.regenerate(&original.id).unwrap();
        let changed = search
            .search("", SearchMode::All)
            .unwrap()
            .results
            .remove(0);
        assert_ne!(changed.id, original.id);
        assert_eq!(changed.pin.as_ref().unwrap().key, key);
        assert!(
            matches!(search.resolve_action(&changed.id, Action::Copy).unwrap(), ResolvedAction::Copy(value) if value == changed.title)
        );
        let same_generator = search
            .search("password 16", SearchMode::Password)
            .unwrap()
            .results
            .remove(1);
        assert_eq!(same_generator.pin.as_ref().unwrap().key, key);
        assert_eq!(
            same_generator.pin.as_ref().unwrap().categories,
            [SearchMode::All]
        );
        assert_eq!(search.search("", SearchMode::All).unwrap().results.len(), 1);
    }

    #[test]
    fn missing_clipboard_items_are_not_restored_by_pins() {
        let directory = tempfile::tempdir().unwrap();
        let mut search = manager(directory.path());
        search.set_pinned("clipboard:1", SearchMode::All, true);
        assert_eq!(
            search.search("", SearchMode::All).unwrap().results[0].id,
            "clipboard:1"
        );
        search.clipboard.remove(1);
        search.forget_clipboard_pins(Some(1));
        assert!(search.pin_key("clipboard:1", SearchMode::All).is_err());
        assert!(
            search
                .search("", SearchMode::All)
                .unwrap()
                .results
                .iter()
                .all(|result| result.id != "clipboard:1")
        );
    }
}
