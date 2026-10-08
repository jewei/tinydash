//! More Files tab results from the OS's own file index (Spotlight on
//! macOS), after TinyDash's index has answered. Off by default; names only,
//! under the home folder, with the index's own rules for hidden and skipped
//! names. Paths it returns are remembered, so Open and Reveal accept them.

use std::path::Path;

use crate::{
    features::files,
    platform,
    search::{matcher::Matcher, result::SearchResult},
    state::State,
};

/// Shorter queries match too much to be useful.
const MIN_QUERY_CHARS: usize = 2;
/// Most results a search adds.
const LIMIT: usize = 100;

pub fn search(state: &State, query: &str) -> Vec<SearchResult> {
    let settings = state.settings.get();
    let query = query.trim();
    let home = &state.dirs.home;
    if !settings.spotlight_files || query.chars().count() < MIN_QUERY_CHARS || !home.is_absolute() {
        return Vec::new();
    }
    let index = state.files.get();
    let mut matcher = Matcher::new(query);
    let mut hits: Vec<(u32, &Path, bool)> = Vec::new();
    let found = platform::find_files(query, home, LIMIT);
    for path in &found {
        let Some(text) = path.to_str() else { continue };
        if index.contains(text) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            continue;
        };
        let is_dir = metadata.is_dir();
        let listed = files::lists(
            path,
            is_dir,
            std::slice::from_ref(home),
            &settings.file_search_excluded_dirs,
            platform::PACKAGE_EXTENSIONS,
            |part| {
                platform::HIDDEN
                    .is_some_and(|hidden| std::fs::symlink_metadata(part).is_ok_and(|m| hidden(&m)))
            },
        );
        if !listed {
            continue;
        }
        // Spotlight matched the name; the matcher ranks it as the index would.
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let score = matcher.name(&name).unwrap_or(0);
        hits.push((score, path, is_dir));
    }
    hits.sort_by_key(|hit| std::cmp::Reverse(hit.0));
    let mut remembered = state.found.lock().unwrap_or_else(|e| e.into_inner());
    hits.into_iter()
        .map(|(_, path, is_dir)| {
            remembered.remember(&path.display().to_string());
            files::found_result(path, is_dir)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn state(spotlight_files: bool) -> State {
        State::for_tests(Settings {
            spotlight_files,
            ..Settings::default()
        })
    }

    #[test]
    fn asks_nothing_while_off_or_for_a_short_query() {
        assert!(search(&state(false), "readme").is_empty());
        // On, but one character, and the test state has no home folder.
        assert!(search(&state(true), "r").is_empty());
        assert!(search(&state(true), "readme").is_empty());
    }

    /// Runs Spotlight for real: `TINYDASH_LIVE_SPOTLIGHT=1 cargo test live_spotlight`.
    #[test]
    fn live_spotlight_results_follow_the_index_rules() {
        if std::env::var_os("TINYDASH_LIVE_SPOTLIGHT").is_none() {
            return;
        }
        let home = std::env::home_dir().expect("a home folder");
        let mut state = state(true);
        state.dirs.home = home.clone();
        let results = search(&state, "readme");
        assert!(!results.is_empty(), "Spotlight found no readme");
        for result in &results {
            let path = result.id.strip_prefix("file:").unwrap();
            let rest = Path::new(path).strip_prefix(&home).unwrap();
            assert!(
                rest.components()
                    .all(|part| !part.as_os_str().to_string_lossy().starts_with('.')),
                "{path}"
            );
            assert!(state.found.lock().unwrap().contains(path));
        }
    }
}
