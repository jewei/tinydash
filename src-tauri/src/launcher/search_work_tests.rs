// The eager evaluator comes from 06475d2 and follows the current ranking policy.
// It remains test-only so selection equivalence includes the real pin, action,
// and icon response paths.
use super::*;
use crate::{
    launcher::result::ResultKind,
    providers::{search_work, system::SystemCommand},
};

impl SearchManager {
    pub(super) fn search_unpinned_eager(&mut self, query: &Query<'_>) -> SearchOutcome {
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
            // preserves file ranking without allocating a result per file.
            results.extend(self.files.search(
                query.text,
                &mut self.matcher,
                &self.usage,
                now,
                RESULT_LIMIT,
            ));
        }
        let mut results = ranking::top_results(results, usize::MAX);
        if query.mode == SearchMode::All && SystemCommandProvider::is_prefix_query(query.text) {
            results.sort_by_key(|result| match result.kind {
                ResultKind::Calculation => 0,
                ResultKind::SystemCommand => 1,
                _ => 2,
            });
        }
        results.truncate(RESULT_LIMIT);
        SearchOutcome { results, notice }
    }
}

fn fixture(apps: usize, files: usize) -> SearchManager {
    let mut manager = SearchManager::default();
    manager.replace_apps(AppProvider::new(
        (0..apps)
            .map(|index| {
                AppEntry::new(
                    format!("Safari {index:03}"),
                    format!("/fixture/app-{index:03}").into(),
                    vec!["work".into(), "日历".into()],
                )
            })
            .collect(),
    ));
    manager.replace_files(FileProvider::new(
        (0..files)
            .map(|index| {
                let path = format!("/fixture/Safari-{index:05}.txt");
                FileEntry {
                    id: format!("file:{path}"),
                    name: format!("Safari-{index:05}.txt"),
                    path,
                    folder: false,
                }
            })
            .collect(),
    ));
    manager.clipboard = ClipboardProvider::new(
        (1..=100)
            .rev()
            .map(|id| ClipboardEntry {
                id,
                content: format!("Safari notes {id}"),
                created_at: id,
                last_used_at: None,
            })
            .collect(),
    );
    manager.system = Some(SystemCommandProvider::new(SystemCommand::ALL.to_vec()));
    manager
}

#[test]
fn full_app_results_skip_lower_provider_work() {
    let mut eager = fixture(40, 50_000);
    eager.eager_search = true;
    search_work::take();
    let expected = eager.search("sa", SearchMode::All).unwrap();
    let before = search_work::take();
    let actual = fixture(40, 50_000).search("sa", SearchMode::All).unwrap();
    let after = search_work::take();
    assert_eq!(
        serde_json::to_value(actual.results).unwrap(),
        serde_json::to_value(expected.results).unwrap()
    );
    assert_eq!(actual.notice, expected.notice);
    println!(
        "Provider work, apps/files/clipboard/system/emoji: before={before:?}; after={after:?}"
    );
    assert_eq!(after.calls, [1, 0, 0, 0, 0]);
    assert_eq!(after.candidates[1], 0);
}

#[test]
fn files_fill_only_the_remaining_slots_then_skip_lower_categories() {
    let mut eager = fixture(25, 200);
    eager.eager_search = true;
    eager.record_usage("file:/fixture/Safari-00199.txt", 0);
    let expected = eager.search("sa", SearchMode::All).unwrap();
    let mut manager = fixture(25, 200);
    manager.record_usage("file:/fixture/Safari-00199.txt", 0);
    search_work::take();
    let actual = manager.search("sa", SearchMode::All).unwrap();
    let work = search_work::take();
    assert_eq!(
        serde_json::to_value(&actual.results).unwrap(),
        serde_json::to_value(expected.results).unwrap()
    );
    assert_eq!(actual.results[25].id, "file:/fixture/Safari-00199.txt");
    assert_eq!(work.calls, [1, 1, 0, 0, 0]);
}

#[test]
fn full_app_results_preserve_calculations_and_currency_notices() {
    for query in ["12 * 8", "100 USD to MYR", "1 / 0"] {
        let prepare = |eager| {
            let mut manager = fixture(0, 30);
            manager.eager_search = eager;
            manager.replace_apps(AppProvider::new(
                (0..40)
                    .map(|i| AppEntry::new(query.into(), format!("/fixture/{i}").into(), vec![]))
                    .collect(),
            ));
            manager
        };
        let expected = prepare(true).search(query, SearchMode::All).unwrap();
        search_work::take();
        let actual = prepare(false).search(query, SearchMode::All).unwrap();
        let work = search_work::take();
        assert_eq!(
            serde_json::to_value(&actual.results).unwrap(),
            serde_json::to_value(expected.results).unwrap(),
            "{query}"
        );
        assert_eq!(actual.notice, expected.notice, "{query}");
        assert_eq!(work.calls, [1, 0, 0, 0, 0], "{query}");
        if query == "12 * 8" {
            assert_eq!(actual.results[0].title, "96");
        } else if query == "100 USD to MYR" {
            assert!(actual.notice.is_some());
        }
    }
}

#[test]
fn bounded_search_matches_eager_selection_across_seeded_corpora() {
    // Fixed pseudo-random values make ties, aliases, hidden apps, Unicode,
    // pins and usage reproducible. Every mode uses the real final response path.
    for seed in 0..16_u64 {
        let prepare = |eager| {
            let mut random = seed + 1;
            let mut next = || {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                random >> 32
            };
            let mut manager = fixture(0, (next() % 70) as usize);
            manager.eager_search = eager;
            let count = [0, 1, 15, 29, 30, 31, 55, 100][seed as usize % 8];
            let names = [
                "Safari",
                "Same app",
                "Café",
                "Cafe\u{301}",
                "日历",
                "App!",
                "Logseq",
            ];
            let mut settings = crate::settings::Settings::default();
            let mut entries = Vec::new();
            for index in 0..count {
                let entry = AppEntry::new(
                    names[next() as usize % names.len()].into(),
                    format!("/fixture/app-{index:03}").into(),
                    vec!["sa".into(), "work".into(), "日历".into()],
                );
                manager.record_usage(&entry.id, 0);
                manager.usage.get_mut(&entry.id).unwrap().count = (next() % 25) as u32;
                settings.app_preferences.insert(
                    entry.id.clone(),
                    crate::settings::AppPreference {
                        hidden: next() % 7 == 0,
                        aliases: vec!["work".into(), "日历".into()],
                    },
                );
                if next() % 3 == 0 {
                    manager.set_pinned(&entry.id, SearchMode::Apps, true);
                    manager.set_pinned(&entry.id, SearchMode::All, true);
                }
                entries.push(entry);
            }
            manager.replace_apps(AppProvider::new(entries));
            manager.apply_settings(&settings);
            for id in 1..=40 {
                let key = format!("clipboard:{id}");
                manager.set_pinned(&key, SearchMode::Clipboard, true);
                manager.set_pinned(&key, SearchMode::All, true);
                manager.record_usage(&key, 0);
            }
            manager.set_pinned("app:/missing", SearchMode::Apps, true);
            manager
        };
        let mut actual = prepare(false);
        let mut expected = prepare(true);
        for mode in [
            SearchMode::All,
            SearchMode::Apps,
            SearchMode::Files,
            SearchMode::Clipboard,
            SearchMode::Emoji,
            SearchMode::System,
            SearchMode::Calculator,
        ] {
            for query in [
                "",
                "sa",
                "app",
                "lo",
                "oc",
                "s",
                "sl",
                "sle",
                "re",
                "res",
                "sh",
                "shu",
                "su",
                "sta",
                "sleep",
                " SLEEP ",
                "suspend",
                "reboot",
                "shutdown",
                "preferences",
                "dark mode",
                "em",
                "empty trash",
                "log",
                "sign out",
                "loc",
                "des",
                "show desktop",
                "mu",
                "unmute",
                "work",
                "日历",
                "cafe",
                "App!",
                ":sa:",
                "12 * 8",
                "1 / 0",
                "100 USD to MYR",
                "no such result",
            ] {
                let before = expected.search(query, mode).unwrap();
                let after = actual.search(query, mode).unwrap();
                assert_eq!(
                    serde_json::to_value(after.results).unwrap(),
                    serde_json::to_value(before.results).unwrap(),
                    "seed={seed}, mode={mode:?}, query={query}"
                );
                assert_eq!(
                    after.notice, before.notice,
                    "seed={seed}, mode={mode:?}, query={query}"
                );
            }
        }
    }
}
