use super::*;
use serde_json::{Value, json};

fn fixture() -> SearchManager {
    let mut manager = SearchManager::default();
    manager.replace_apps(AppProvider::new(
        (0..100)
            .map(|index| {
                let name = if index % 3 == 0 {
                    "Same app".into()
                } else {
                    format!("Application {index:03}")
                };
                let mut entry = AppEntry::new(
                    name,
                    format!("/fixture/app-{index:03}").into(),
                    vec!["work".into(), "日历".into()],
                );
                entry.icon = Some("x".repeat(32 * 1024));
                entry
            })
            .collect(),
    ));
    manager.usage.insert(
        "app:/fixture/app-097".into(),
        ranking::Usage {
            count: 20,
            last_used_at: 0,
        },
    );
    let mut settings = crate::settings::Settings::default();
    settings.app_preferences.insert(
        "app:/fixture/app-001".into(),
        crate::settings::AppPreference {
            hidden: true,
            aliases: vec![],
        },
    );
    manager.apply_settings(&settings);
    for index in 50..95 {
        manager.set_pinned(
            &format!("app:/fixture/app-{index:03}"),
            SearchMode::Apps,
            true,
        );
        manager.set_pinned(
            &format!("app:/fixture/app-{index:03}"),
            SearchMode::All,
            true,
        );
    }
    manager.set_pinned("app:/missing", SearchMode::Apps, true);
    manager
}

#[test]
fn selection_matches_the_frozen_baseline() {
    let mut cases = Vec::new();
    for (query, mode) in [
        ("", SearchMode::Apps),
        ("", SearchMode::All),
        ("app", SearchMode::Apps),
        ("same", SearchMode::Apps),
        ("work", SearchMode::Apps),
        ("日历", SearchMode::Apps),
        ("app", SearchMode::All),
    ] {
        let outcome = fixture().search(query, mode).unwrap();
        let mut results = serde_json::to_value(&outcome.results).unwrap();
        for result in results.as_array_mut().unwrap() {
            result.as_object_mut().unwrap().remove("icon");
        }
        cases.push(json!({"query": query,"mode":mode,"results":results,"notice":outcome.notice}));
    }
    let actual = Value::Array(cases);
    // Used once in an isolated copy of the original source to record the old
    // implementation. Normal test runs always compare, never update this file.
    if let Ok(path) = std::env::var("TINYDASH_WRITE_SELECTION_FIXTURE") {
        std::fs::write(path, serde_json::to_string(&actual).unwrap() + "\n").unwrap();
        return;
    }
    let expected: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/app-selection.json")).unwrap();
    assert_eq!(actual, expected);
}


#[test]
fn record_old_payload_copy_count() {
    use crate::providers::apps::ICON_COPIES;
    let mut manager = fixture();
    ICON_COPIES.with(|value| value.set((0,0)));
    assert_eq!(manager.search("app",SearchMode::Apps).unwrap().results.len(),30);
    let search=ICON_COPIES.with(|value| value.get());
    assert_eq!(search,(99,99*32768));
    ICON_COPIES.with(|value| value.set((0,0)));
    assert_eq!(manager.apps.catalog().len(),100);
    let catalog=ICON_COPIES.with(|value| value.get());
    assert_eq!(catalog,(100,100*32768));
    println!("baseline payload copies: search={search:?}, catalog={catalog:?}");
}
