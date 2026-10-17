//! More Files tab results from the OS's own file index (Spotlight on
//! macOS), after TinyDash's index has answered. Off by default; names only,
//! under the home folder, with the index's own rules for hidden and skipped
//! names. Paths it returns are remembered, so Open and Reveal accept them.

use std::path::{Path, PathBuf};

use crate::{
    features::files,
    platform,
    search::{id::Source, matcher::Matcher, result::SearchResult},
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
    let found = platform::find_files(query, home, LIMIT);
    results(state, query, &found)
}

/// The paths of `found` that the Files tab would list but has not: not in
/// the index, not hidden by the user, and listed by the index's rules.
/// They are ranked and remembered. The OS search skips the hidden-results
/// filter in `search::search`, so it is applied here.
fn results(state: &State, query: &str, found: &[PathBuf]) -> Vec<SearchResult> {
    let settings = state.settings.get();
    let home = &state.dirs.home;
    let index = state.files.get();
    let hidden_results = state.hidden.get();
    let mut matcher = Matcher::new(query);
    let mut hits: Vec<(u32, &Path, bool)> = Vec::new();
    for path in found {
        let Some(text) = path.to_str() else { continue };
        if index.contains(text) || hidden_results.contains(&Source::File.id(path.display())) {
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
    use crate::{features::files::FileIndex, search::usage::Hidden, settings::Settings};

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

    #[test]
    fn hidden_and_indexed_paths_are_skipped() {
        let home = std::env::temp_dir().join(format!("tinydash-spotlight-{}", std::process::id()));
        std::fs::create_dir_all(home.join("indexed")).unwrap();
        let hidden = home.join("hidden.txt");
        let indexed = home.join("indexed/notes.txt");
        let other = home.join("other.txt");
        for path in [&hidden, &indexed, &other] {
            std::fs::write(path, "").unwrap();
        }
        let mut state = state(true);
        state.dirs.home = home.clone();
        state
            .files
            .set(FileIndex::scan(&[home.join("indexed")], &[], &[], None));
        state
            .hidden
            .set(Hidden::new([Source::File.id(hidden.display())]));

        let found = [hidden.clone(), indexed.clone(), other.clone()];
        let ids: Vec<String> = results(&state, "txt", &found)
            .into_iter()
            .map(|result| result.id)
            .collect();
        assert_eq!(ids, [Source::File.id(other.display())]);
        let remembered = state.found.lock().unwrap();
        assert!(remembered.contains(&other.display().to_string()));
        assert!(!remembered.contains(&hidden.display().to_string()));
        assert!(!remembered.contains(&indexed.display().to_string()));
        drop(remembered);
        std::fs::remove_dir_all(&home).unwrap();
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
