use super::{query::SearchMode, result::Action, search::SearchManager};
use crate::{
    providers::apps::{AppEntry, AppProvider},
    settings::{ItemPreference, Settings},
};

#[test]
fn item_shortcuts_reject_conflicts_and_invalid_aliases() {
    let mut settings = Settings::default();
    settings.item_preferences.insert(
        "system:settings".into(),
        ItemPreference {
            shortcut: settings.shortcut.clone(),
            ..Default::default()
        },
    );
    assert!(settings.validate().is_err());
    settings
        .item_preferences
        .get_mut("system:settings")
        .unwrap()
        .shortcut = "Control+Alt+KeyS".into();
    assert!(settings.validate().is_ok());
    assert!(settings.shortcuts().contains(&"Control+Alt+KeyS"));
    settings
        .item_preferences
        .get_mut("system:settings")
        .unwrap()
        .disabled = true;
    assert!(!settings.shortcuts().contains(&"Control+Alt+KeyS"));
    settings
        .item_preferences
        .get_mut("system:settings")
        .unwrap()
        .aliases = vec!["\n".into()];
    assert!(settings.validate().is_err());
}

#[test]
fn hidden_item_retains_explicit_action_but_disabled_item_does_not() {
    let entry = AppEntry::new("Browser".into(), "/Applications/Browser.app".into(), vec![]);
    let id = entry.id.clone();
    let mut search = SearchManager::default();
    search.replace_apps(AppProvider::new(vec![entry]));
    let mut settings = Settings::default();
    settings.item_preferences.insert(
        id.clone(),
        ItemPreference {
            aliases: vec!["surf".into()],
            ..Default::default()
        },
    );
    search.apply_settings(&settings);
    assert_eq!(
        search.search("surf", SearchMode::Apps).unwrap().results[0].id,
        id
    );
    settings.item_preferences.get_mut(&id).unwrap().hidden = true;
    search.apply_settings(&settings);
    assert!(
        search
            .search("surf", SearchMode::Apps)
            .unwrap()
            .results
            .is_empty()
    );
    assert!(search.resolve_action(&id, Action::Launch).is_ok());
    settings.item_preferences.get_mut(&id).unwrap().disabled = true;
    search.apply_settings(&settings);
    assert!(search.resolve_action(&id, Action::Launch).is_err());
}

#[test]
fn native_commands_have_searchable_aliases_and_cannot_execute_when_disabled() {
    let mut search = SearchManager::default();
    let mut settings = Settings::default();
    settings.item_preferences.insert(
        "command:colors".into(),
        ItemPreference {
            aliases: vec!["paint".into()],
            ..Default::default()
        },
    );
    search.apply_settings(&settings);
    assert_eq!(
        search.search("paint", SearchMode::All).unwrap().results[0].id,
        "command:colors"
    );
    settings
        .item_preferences
        .get_mut("command:colors")
        .unwrap()
        .disabled = true;
    search.apply_settings(&settings);
    assert!(
        search
            .search("paint", SearchMode::All)
            .unwrap()
            .results
            .iter()
            .all(|result| result.id != "command:colors")
    );
    assert!(
        search
            .resolve_action("command:colors", Action::Run)
            .is_err()
    );
}

#[test]
fn new_capture_and_index_settings_are_backward_compatible_and_bounded() {
    let defaults: Settings = serde_json::from_str("{}").unwrap();
    assert!(!defaults.clipboard_capture_images && !defaults.clipboard_capture_files);
    assert!(!defaults.file_search_include_hidden);
    assert_eq!(defaults.clipboard_retention_days, 0);
    let mut changed = defaults.clone();
    changed.file_search_ignore_patterns = vec!["*.log".into()];
    assert!(!changed.same_file_settings(&defaults));
    changed.file_search_ignore_patterns = vec!["".into()];
    assert!(changed.validate().is_err());
    changed.file_search_ignore_patterns.clear();
    changed.clipboard_retention_days = 3651;
    assert!(changed.validate().is_err());
}
