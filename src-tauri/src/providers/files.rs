use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
};

use nucleo_matcher::{
    Matcher, Utf32Str, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

use crate::{
    error::{Error, Result},
    launcher::result::{Action, ResultKind, SearchResult},
    platform, ranking,
};

const VISIT_LIMIT: usize = 500_000;
pub const WATCH_LIMIT: usize = 8192;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub folder: bool,
}

impl FileEntry {
    // A folder keeps the `file:` ID prefix. Paths are unique across both kinds,
    // so actions, pins, and usage do not change.
    fn new(path: &Path, folder: bool) -> Option<Self> {
        // The opener accepts UTF-8 paths. Never use a lossy path for an action ID.
        let path = path.to_str()?;
        #[cfg(target_os = "windows")]
        let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
        };
        #[cfg(not(target_os = "windows"))]
        let path = path.to_owned();
        Some(Self {
            name: Path::new(&path).file_name()?.to_str()?.to_owned(),
            id: format!("file:{path}"),
            path,
            folder,
        })
    }

    pub fn result(&self, score: u32) -> SearchResult {
        SearchResult {
            id: self.id.clone(),
            kind: if self.folder {
                ResultKind::Folder
            } else {
                ResultKind::File
            },
            title: self.name.clone(),
            subtitle: self.path.clone(),
            path: Some(self.path.clone()),
            score,
            icon: None,
            primary_action: Action::Open,
            secondary_actions: vec![Action::Reveal],
            pin: None,
            confirmation: None,
            detail: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        // Indexed paths are canonical. Reject a directory replaced by a link
        // since the scan, not just a symlink at the final filename.
        for ancestor in Path::new(&self.path).ancestors().skip(1) {
            if std::fs::symlink_metadata(ancestor)?
                .file_type()
                .is_symlink()
            {
                return Err(Error::FileNotFound);
            }
        }
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Error::FileNotFound
            } else {
                error.into()
            }
        })?;
        let kind_matches = if self.folder {
            metadata.is_dir()
        } else {
            metadata.is_file()
        };
        if kind_matches && !metadata.file_type().is_symlink() {
            Ok(())
        } else {
            Err(Error::FileNotFound)
        }
    }
}

struct IndexedFile {
    entry: FileEntry,
    // ASCII matcher input can borrow the entry's existing UTF-8 bytes. Cache
    // only text that needs Unicode normalization or path separator conversion.
    name: Option<Utf32String>,
    normalized_name: String,
    path: Option<Utf32String>,
}

impl IndexedFile {
    fn name(&self) -> Utf32Str<'_> {
        match &self.name {
            Some(name) => name.slice(..),
            None => Utf32Str::Ascii(self.entry.name.as_bytes()),
        }
    }

    fn path(&self) -> Utf32Str<'_> {
        match &self.path {
            Some(path) => path.slice(..),
            None => Utf32Str::Ascii(self.entry.path.as_bytes()),
        }
    }
}

#[derive(Default)]
pub struct FileProvider {
    files: Vec<IndexedFile>,
}

impl FileProvider {
    pub(crate) fn new(entries: Vec<FileEntry>) -> Self {
        let mut files: Vec<_> = entries
            .into_iter()
            .map(|entry| {
                // macOS can return decomposed accents. Canonicalize matching
                // text only; keep the exact OS path for IDs and file actions.
                let name: Cow<'_, str> = if entry.name.is_ascii() {
                    Cow::Borrowed(&entry.name)
                } else {
                    Cow::Owned(entry.name.nfc().collect())
                };
                IndexedFile {
                    name: (!entry.name.is_ascii()).then(|| name.as_ref().into()),
                    normalized_name: ranking::normalize(&name),
                    // Accept forward slashes in Windows path queries as well.
                    path: (!entry.path.is_ascii() || entry.path.contains('\\')).then(|| {
                        entry
                            .path
                            .replace('\\', "/")
                            .nfc()
                            .collect::<String>()
                            .into()
                    }),
                    entry,
                }
            })
            .collect();
        files.sort_unstable_by(|a, b| {
            a.normalized_name
                .cmp(&b.normalized_name)
                .then_with(|| a.entry.path.cmp(&b.entry.path))
        });
        Self { files }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn get(&self, id: &str) -> Option<&FileEntry> {
        self.files
            .iter()
            .find(|file| file.entry.id == id)
            .map(|file| &file.entry)
    }

    pub fn search(
        &self,
        query: &str,
        matcher: &mut Matcher,
        usage: &HashMap<String, ranking::Usage>,
        now: i64,
        limit: usize,
    ) -> Vec<SearchResult> {
        #[cfg(test)]
        super::search_work::record(super::search_work::Provider::Files, self.files.len());
        let normalized: String = ranking::normalize(query).nfc().collect();
        let pattern = Pattern::new(
            &normalized,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let path_pattern = Pattern::new(
            &normalized.replace('\\', "/"),
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        // Keep only scores and positions until ranking finishes. Do not clone
        // thousands of paths into result objects on each keystroke.
        let mut matches: Vec<_> = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                let score = if normalized.is_empty() {
                    Some(0)
                } else {
                    let name = pattern.score(file.name(), matcher).map(|score| {
                        ranking::name_score(score, &file.normalized_name, &normalized)
                    });
                    let path = path_pattern
                        .score(file.path(), matcher)
                        .map(|score| score / 2);
                    name.into_iter().chain(path).max()
                }?;
                Some((
                    index,
                    ranking::score_with_usage(score, &file.entry.id, usage, now),
                ))
            })
            .collect();
        let compare =
            |a: &(usize, u32), b: &(usize, u32)| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0));
        if matches.len() > limit {
            matches.select_nth_unstable_by(limit, compare);
            matches.truncate(limit);
        }
        matches.sort_unstable_by(compare);
        matches
            .into_iter()
            .map(|(index, score)| self.files[index].entry.result(score))
            .collect()
    }
}

#[derive(Default)]
pub struct ScanReport {
    pub issue_count: usize,
    pub first_issue: Option<String>,
    pub limited: bool,
    pub directories: Vec<PathBuf>,
    pub watches_limited: bool,
}

impl ScanReport {
    fn directory(&mut self, path: &Path) {
        if self.directories.len() < WATCH_LIMIT {
            self.directories.push(path.to_owned());
        } else {
            self.watches_limited = true;
        }
    }

    pub fn issue(&mut self, path: &Path, error: impl std::fmt::Display) {
        self.issue_count += 1;
        if self.first_issue.is_none() {
            self.first_issue = Some(format!("{}: {error}", path.display()));
        }
    }

    pub fn warning(&self) -> Option<String> {
        let mut messages = Vec::new();
        if self.limited {
            messages.push("File scan reached its limit. Choose smaller folders or increase fileSearchLimit in settings.json.".to_owned());
        }
        if let Some(issue) = &self.first_issue {
            messages.push(format!(
                "File scan skipped {} item(s). First: {issue}",
                self.issue_count
            ));
        }
        if messages.is_empty() {
            None
        } else {
            Some(messages.join(" "))
        }
    }
}

pub fn expand_root(path: &Path, home: Option<&Path>) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix("~") {
        home.map(|home| home.join(relative))
    } else if path.is_absolute() {
        Some(path.to_owned())
    } else {
        None
    }
}

/// Scan using the original defaults. File contents are never read by the index.
#[cfg(test)]
pub fn scan(
    roots: Vec<PathBuf>,
    excluded: &[String],
    limit: usize,
    report: &mut ScanReport,
) -> FileProvider {
    scan_with_rules(roots, excluded, limit, false, &[], report)
}

const IGNORE_PATTERN_LIMIT: usize = 64;
const IGNORE_PATTERN_LENGTH: usize = 256;
const IGNORE_PATH_LENGTH: usize = 4096;

pub fn valid_ignore_patterns(patterns: &[String]) -> bool {
    patterns.len() <= IGNORE_PATTERN_LIMIT
        && patterns.iter().all(|pattern| {
            !pattern.trim().is_empty()
                && !pattern.chars().any(char::is_control)
                && IgnorePattern::parse(pattern).is_some()
        })
}

#[derive(Clone, Copy)]
enum GlobToken {
    Literal(char),
    Any,
    Star(bool),
    Directories,
}

struct IgnorePattern {
    tokens: Vec<GlobToken>,
    basename: bool,
    directory_only: bool,
}

impl IgnorePattern {
    fn parse(value: &str) -> Option<Self> {
        if value.is_empty()
            || value.len() > IGNORE_PATTERN_LENGTH
            || value.contains(['\\', '[', ']', '\0'])
            || value.starts_with('!')
        {
            return None;
        }
        let directory_only = value.ends_with('/');
        let pattern = value.trim_end_matches('/');
        if pattern.is_empty() {
            return None;
        }
        let basename = !pattern.contains('/');
        let mut chars = pattern.chars().peekable();
        let mut tokens = Vec::new();
        while let Some(ch) = chars.next() {
            tokens.push(match ch {
                '*' => {
                    let recursive = chars.peek() == Some(&'*');
                    if recursive {
                        chars.next();
                    }
                    if recursive && chars.peek() == Some(&'/') {
                        chars.next();
                        GlobToken::Directories
                    } else {
                        GlobToken::Star(recursive)
                    }
                }
                '?' => GlobToken::Any,
                ch => GlobToken::Literal(ch),
            });
        }
        Some(Self {
            tokens,
            basename,
            directory_only,
        })
    }

    // Bounded dynamic programming, never recursive/backtracking. Match Unicode
    // characters, not individual UTF-8 bytes. Patterns use forward slashes.
    fn matches(&self, relative: &str, name: &str, directory: bool) -> bool {
        if self.directory_only && !directory {
            return false;
        }
        let value = if self.basename { name } else { relative };
        if value.len() > IGNORE_PATH_LENGTH {
            return false;
        }
        let chars: Vec<_> = value.chars().collect();
        let mut previous = vec![false; chars.len() + 1];
        let mut next = vec![false; chars.len() + 1];
        previous[0] = true;
        for token in &self.tokens {
            next.fill(false);
            let mut prefix = false;
            for j in 0..=chars.len() {
                next[j] = match token {
                    GlobToken::Literal(ch) => j > 0 && previous[j - 1] && chars[j - 1] == *ch,
                    GlobToken::Any => j > 0 && previous[j - 1] && chars[j - 1] != '/',
                    GlobToken::Star(recursive) => {
                        previous[j] || (j > 0 && next[j - 1] && (*recursive || chars[j - 1] != '/'))
                    }
                    GlobToken::Directories => {
                        previous[j] || (j > 0 && prefix && chars[j - 1] == '/')
                    }
                };
                prefix |= previous[j];
            }
            std::mem::swap(&mut previous, &mut next);
        }
        previous[chars.len()]
    }
}

/// Ignore patterns match basenames or root-relative paths. Supports *, ?, **,
/// and trailing / for folders; no negation, escaping, or character classes.
/// An ignored folder is pruned, including its watcher registrations.
pub fn scan_with_rules(
    roots: Vec<PathBuf>,
    excluded: &[String],
    limit: usize,
    include_hidden: bool,
    ignore_patterns: &[String],
    report: &mut ScanReport,
) -> FileProvider {
    let patterns: Vec<_> = ignore_patterns
        .iter()
        .take(IGNORE_PATTERN_LIMIT)
        .filter_map(|pattern| {
            let parsed = IgnorePattern::parse(pattern);
            if parsed.is_none() {
                report.issue(
                    Path::new("fileSearchIgnorePatterns"),
                    "Invalid ignore pattern: use at most 256 bytes and only *, ?, ** wildcards.",
                );
            }
            parsed
        })
        .collect();
    if ignore_patterns.len() > IGNORE_PATTERN_LIMIT {
        report.issue(
            Path::new("fileSearchIgnorePatterns"),
            "Only the first 64 ignore patterns are used.",
        );
    }
    let mut resolved = Vec::new();
    for root in roots {
        match std::fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                match root.canonicalize() {
                    Ok(path) => resolved.push(path),
                    Err(error) => report.issue(&root, error),
                }
            }
            Ok(_) => report.issue(
                &root,
                "Scan roots must be folders, not symbolic links or files.",
            ),
            Err(error) => report.issue(&root, error),
        }
    }
    // Remove overlapping roots before walking them. Ancestors sort first.
    resolved.sort();
    let mut unique: Vec<PathBuf> = Vec::new();
    for root in resolved {
        if !unique.iter().any(|parent| root.starts_with(parent)) {
            unique.push(root);
        }
    }
    let mut entries = Vec::new();
    let mut visited = 0;
    for root in &unique {
        report.directory(root);
    }
    'roots: for root in unique {
        let mut walk = WalkDir::new(&root)
            .follow_links(false)
            .follow_root_links(false)
            .max_open(16)
            .into_iter();
        while let Some(item) = walk.next() {
            visited += 1;
            if visited > VISIT_LIMIT {
                report.limited = true;
                break 'roots;
            }
            let entry = match item {
                Ok(entry) => entry,
                Err(error) => {
                    report.issue(error.path().unwrap_or(&root), &error);
                    continue;
                }
            };
            if entry.depth() == 0 {
                continue;
            }
            let directory = entry.file_type().is_dir();
            let excluded = directory
                && excluded
                    .iter()
                    .any(|name| entry.file_name() == name.as_str());
            let hidden = match if include_hidden {
                Ok(false)
            } else {
                platform::file_is_hidden(&entry)
            } {
                Ok(hidden) => hidden,
                Err(error) => {
                    report.issue(entry.path(), error);
                    true
                }
            };
            let ignored = entry
                .path()
                .strip_prefix(&root)
                .ok()
                .and_then(Path::to_str)
                .is_some_and(|path| {
                    #[cfg(target_os = "windows")]
                    let normalized = path.replace('\\', "/");
                    #[cfg(target_os = "windows")]
                    let path = normalized.as_str();
                    patterns.iter().any(|pattern| {
                        pattern.matches(path, entry.file_name().to_str().unwrap_or(""), directory)
                    })
                });
            if excluded || hidden || ignored || entry.file_type().is_symlink() {
                if directory {
                    walk.skip_current_dir();
                }
                continue;
            }
            if !directory && !entry.file_type().is_file() {
                continue;
            }
            if directory {
                report.directory(entry.path());
            }
            if let Some(item) = FileEntry::new(entry.path(), directory) {
                if entries.len() >= limit {
                    report.limited = true;
                    break 'roots;
                }
                entries.push(item);
            } else {
                report.issue(entry.path(), "Path is not valid UTF-8.");
                if directory {
                    walk.skip_current_dir();
                }
            }
        }
    }
    FileProvider::new(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::{actions::ResolvedAction, query::SearchMode, search::SearchManager};
    use std::fs;

    #[test]
    #[ignore = "Measures search on 50,000 synthetic paths. Run in release mode with --ignored --nocapture."]
    fn profile_search_50k_files() {
        let files = (0..50_000)
            .map(|index| {
                FileEntry::new(
                    Path::new(&format!(
                        "/benchmark/Project-{}/Document-{index}.txt",
                        index % 100
                    )),
                    false,
                )
                .expect("entry")
            })
            .collect();
        let mut manager = SearchManager::default();
        let started = std::time::Instant::now();
        manager.replace_files(FileProvider::new(files));
        let indexing_ms = started.elapsed().as_millis();
        let mut elapsed = Vec::new();
        for _ in 0..25 {
            for query in ["doc", "dcm123", "Project-72", "no-such-filename"] {
                let started = std::time::Instant::now();
                let results = manager.search(query, SearchMode::Files).expect("search");
                std::hint::black_box(results);
                elapsed.push(started.elapsed().as_micros());
            }
        }
        elapsed.sort_unstable();
        eprintln!(
            "50,000 files: index={indexing_ms}ms; search p50={}µs p95={}µs max={}µs",
            elapsed[50], elapsed[95], elapsed[99]
        );
    }

    #[test]
    fn matches_composed_and_decomposed_unicode_without_changing_file_paths() {
        let path = Path::new("/Documents/cafe\u{301}.txt");
        let entry = FileEntry::new(path, false).expect("entry");
        let id = entry.id.clone();
        let provider = FileProvider::new(vec![entry]);
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        for query in ["café.txt", "cafe\u{301}.txt", "Documents/café.txt"] {
            let results = provider.search(query, &mut matcher, &HashMap::new(), 0, 30);
            assert_eq!(results.len(), 1, "Query: {query}");
            assert_eq!(results[0].id, id);
            assert_eq!(results[0].subtitle, path.to_str().expect("path"));
        }
        assert!(
            provider.search("café.txt", &mut matcher, &HashMap::new(), 0, 30)[0].score >= 10_000
        );
    }

    #[test]
    fn ascii_matching_borrows_existing_entry_storage() {
        let entry = FileEntry::new(Path::new("/Documents/Report.txt"), false).expect("entry");
        let provider = FileProvider::new(vec![entry]);
        let file = &provider.files[0];
        assert!(file.name.is_none());
        assert!(file.path.is_none());
        let Utf32Str::Ascii(name) = file.name() else {
            panic!("expected ASCII name");
        };
        let Utf32Str::Ascii(path) = file.path() else {
            panic!("expected ASCII path");
        };
        assert!(std::ptr::eq(name, file.entry.name.as_bytes()));
        assert!(std::ptr::eq(path, file.entry.path.as_bytes()));
    }

    #[test]
    fn borrowed_matching_preserves_owned_matching_results() {
        let entries = [
            ("Report.txt", "/Documents/Report.txt"),
            ("Report.txt", "/Archive/Report.txt"),
            ("Report.txt", r"C:\Documents\Report.txt"),
            ("Quarterly  Report.txt", "/Documents/Quarterly  Report.txt"),
            ("Tab\tReport.txt", "/Documents/Tab\tReport.txt"),
            ("Line\nReport.txt", "/Documents/Line\nReport.txt"),
            ("CRLF\r\nReport.txt", "/Documents/CRLF\r\nReport.txt"),
            ("café.txt", "/Documents/café.txt"),
            ("cafe\u{301}.txt", "/Documents/cafe\u{301}.txt"),
            // NFC can turn a non-ASCII source into ASCII. The original bytes
            // still cannot be passed to the matcher's ASCII variant.
            ("\u{212a}elvin.txt", "/Documents/\u{212a}elvin.txt"),
            ("report.txt", "/日本語/report.txt"),
            (r"report\draft.txt", r"/Documents/report\draft.txt"),
        ]
        .into_iter()
        .map(|(name, path)| FileEntry {
            id: format!("file:{path}"),
            name: name.into(),
            path: path.into(),
            folder: false,
        })
        .collect::<Vec<_>>();
        let provider = FileProvider::new(entries.clone());
        let mut owned = FileProvider::new(entries);
        // Recreate the previous index representation as a reference: every
        // matching name and path owns a normalized string, including ASCII.
        for file in &mut owned.files {
            file.name = Some(file.entry.name.nfc().collect::<String>().into());
            file.path = Some(
                file.entry
                    .path
                    .replace('\\', "/")
                    .nfc()
                    .collect::<String>()
                    .into(),
            );
        }
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut reference_matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut usage = HashMap::new();
        for with_usage in [false, true] {
            if with_usage {
                usage.insert(
                    "file:/Archive/Report.txt".into(),
                    ranking::Usage {
                        count: 15,
                        last_used_at: 100,
                    },
                );
            }
            for query in [
                "",
                "report",
                "REPORT.TXT",
                "quarterly report",
                "tab report",
                "line report",
                "crlf report",
                "cafe",
                "café.txt",
                "cafe\u{301}.txt",
                "Documents/café",
                r"Documents\Report",
                "日本語",
                "Kelvin",
                "no-such-file",
            ] {
                for limit in [0, 1, 4, 30, usize::MAX] {
                    let results = provider.search(query, &mut matcher, &usage, 100, limit);
                    let expected = owned.search(query, &mut reference_matcher, &usage, 100, limit);
                    assert_eq!(
                        serde_json::to_value(results).expect("results"),
                        serde_json::to_value(expected).expect("reference results"),
                        "query={query:?}, limit={limit}, with_usage={with_usage}"
                    );
                }
            }
        }
    }

    fn write(root: &Path, relative: &str, text: &str) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        fs::write(&path, text).expect("file");
        path
    }

    #[test]
    fn scans_names_only_filters_hidden_and_excluded_folders_and_deduplicates_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "Reports/Quarterly résumé.txt",
            "secret-file-content",
        );
        write(dir.path(), "Quarterly plan.txt", "");
        write(dir.path(), ".hidden.txt", "");
        write(dir.path(), ".private/hidden.txt", "");
        write(dir.path(), "node_modules/ignored.txt", "");
        let mut report = ScanReport::default();
        let files = scan(
            vec![
                dir.path().join("Reports"),
                dir.path().into(),
                dir.path().into(),
            ],
            &["node_modules".into()],
            100,
            &mut report,
        );
        // Two files and the Reports folder.
        assert_eq!(files.len(), 3);
        assert_eq!(report.warning(), None);
        let mut search = SearchManager::default();
        search.replace_files(files);
        assert_eq!(
            search
                .search("resume", SearchMode::Files)
                .expect("search")
                .results[0]
                .title,
            "Quarterly résumé.txt"
        );
        assert_eq!(
            search
                .search("Reports/Quarterly", SearchMode::Files)
                .expect("search")
                .results
                .len(),
            1
        );
        assert_eq!(
            search
                .search(r"Reports\Quarterly", SearchMode::Files)
                .expect("search")
                .results
                .len(),
            1
        );
        assert_eq!(
            search
                .search("QUARTERLY PLAN.TXT", SearchMode::All)
                .expect("search")
                .results[0]
                .title,
            "Quarterly plan.txt"
        );
        assert!(
            search
                .search("secret-file-content", SearchMode::Files)
                .expect("search")
                .results
                .is_empty()
        );
        assert!(
            search
                .search("Quarterly", SearchMode::Apps)
                .expect("search")
                .results
                .is_empty()
        );
        assert!(
            search
                .search("", SearchMode::All)
                .expect("search")
                .results
                .is_empty()
        );
        assert_eq!(
            search
                .search("", SearchMode::Files)
                .expect("search")
                .results
                .len(),
            3
        );
    }

    #[test]
    fn ignore_rules_and_hidden_opt_in_prune_trees_without_changing_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in [
            ".private/keep.txt",
            "keep.txt",
            "root.tmp",
            "nested/cache.tmp",
            "nested/keep.txt",
            "build/keep.txt",
            "node_modules/keep.txt",
        ] {
            write(dir.path(), name, "");
        }
        let mut report = ScanReport::default();
        let files = scan_with_rules(
            vec![dir.path().into()],
            &["node_modules".into()],
            100,
            true,
            &["**/*.tmp".into(), "build/".into()],
            &mut report,
        );
        let names: Vec<_> = files
            .files
            .iter()
            .map(|file| file.entry.name.as_str())
            .collect();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&".private"));
        assert!(!names.contains(&"build"));
        assert!(!names.contains(&"root.tmp"));
        assert!(!names.contains(&"cache.tmp"));
        assert!(
            report
                .directories
                .iter()
                .all(|path| !path.ends_with("build") && !path.ends_with("node_modules"))
        );
        assert!(report.warning().is_none());
        assert!(
            scan(
                vec![dir.path().into()],
                &[],
                100,
                &mut ScanReport::default()
            )
            .files
            .iter()
            .all(|file| !file.entry.path.contains(".private"))
        );
    }

    #[test]
    fn glob_matching_is_bounded_and_has_explicit_path_semantics() {
        let matches = |pattern, path: &str, directory| {
            IgnorePattern::parse(pattern).expect("pattern").matches(
                path,
                path.rsplit('/').next().unwrap(),
                directory,
            )
        };
        assert!(matches("*.txt", "nested/café.txt", false));
        assert!(matches("caf?.txt", "nested/café.txt", false));
        assert!(!matches("nested/*.txt", "nested/deeper/a.txt", false));
        assert!(matches("nested/**/*.txt", "nested/a.txt", false));
        assert!(matches("nested/**/*.txt", "nested/deeper/a.txt", false));
        assert!(!matches("build/", "build", false));
        assert!(matches("build/", "build", true));
        assert!(IgnorePattern::parse(&"*".repeat(257)).is_none());
        assert!(IgnorePattern::parse("[abc]").is_none());
        assert!(!matches("*", &"x".repeat(4097), false));
    }

    #[test]
    fn folders_are_results_that_open_and_validate_as_folders() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "Invoices/2026/march.pdf", "");
        write(dir.path(), "invoices.txt", "");
        write(dir.path(), ".hidden/Invoices/april.pdf", "");
        let files = scan(
            vec![dir.path().into()],
            &[],
            100,
            &mut ScanReport::default(),
        );
        let mut search = SearchManager::default();
        search.replace_files(files);
        let results = search
            .search("Invoices", SearchMode::Files)
            .expect("search")
            .results;
        let folder = results
            .iter()
            .find(|result| result.kind == ResultKind::Folder)
            .expect("folder result");
        assert_eq!(folder.title, "Invoices");
        assert!(
            results
                .iter()
                .all(|result| !result.subtitle.contains(".hidden")),
            "Hidden folders stay out of the index"
        );
        assert!(results.iter().any(|result| result.kind == ResultKind::File));
        assert_eq!(folder.primary_action, Action::Open);
        assert_eq!(folder.secondary_actions, [Action::Reveal]);
        let Ok(ResolvedAction::File(entry, Action::Open)) =
            search.resolve_action(&folder.id, Action::Open)
        else {
            panic!("folders open like files");
        };
        assert!(entry.validate().is_ok());
        let path = dir.path().join("Invoices");
        fs::remove_dir_all(&path).expect("delete");
        fs::write(&path, "").expect("replace with a file");
        assert!(matches!(entry.validate(), Err(Error::FileNotFound)));
        assert_eq!(
            search
                .search("2026", SearchMode::Files)
                .expect("search")
                .results[0]
                .kind,
            ResultKind::Folder
        );
    }

    #[test]
    fn scan_is_bounded_and_missing_roots_do_not_hide_good_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..8 {
            write(dir.path(), &format!("entry-{i}.txt"), "");
        }
        let mut report = ScanReport::default();
        let files = scan(
            vec![dir.path().join("missing"), dir.path().into()],
            &[],
            3,
            &mut report,
        );
        assert_eq!(files.len(), 3);
        assert!(report.limited);
        assert_eq!(report.issue_count, 1);
        assert!(report.warning().expect("warning").contains("limit"));
        let files = scan(vec![], &[], 100, &mut ScanReport::default());
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn usage_applies_before_limit_and_ids_survive_scans() {
        let dir = tempfile::tempdir().expect("tempdir");
        for i in 0..45 {
            write(dir.path(), &format!("report-{i:02}.txt"), "");
        }
        let make_index = || {
            scan(
                vec![dir.path().into()],
                &[],
                100,
                &mut ScanReport::default(),
            )
        };
        let mut search = SearchManager::default();
        search.replace_files(make_index());
        let used = search
            .search("report-44.txt", SearchMode::Files)
            .expect("search")
            .results
            .remove(0);
        search.record_usage(&used.id, ranking::now());
        search.replace_files(make_index());
        let results = search
            .search("report", SearchMode::Files)
            .expect("search")
            .results;
        assert_eq!(results.len(), 30);
        assert_eq!(results[0].id, used.id);
        assert_eq!(
            search
                .search("report-00.txt", SearchMode::Files)
                .expect("search")
                .results[0]
                .title,
            "report-00.txt"
        );
        assert!(search.resolve_action(&used.id, Action::Open).is_ok());
        assert!(search.resolve_action(&used.id, Action::Reveal).is_ok());
        assert!(search.resolve_action(&used.id, Action::Launch).is_err());
        assert!(
            search
                .resolve_action("file:/unindexed.txt", Action::Open)
                .is_err()
        );
    }

    #[test]
    fn deleted_files_fail_validation_and_refresh_removes_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "removed.txt", "");
        let files = scan(
            vec![dir.path().into()],
            &[],
            100,
            &mut ScanReport::default(),
        );
        let entry = &files.files[0].entry;
        assert!(entry.validate().is_ok());
        fs::remove_file(path).expect("delete");
        assert!(matches!(entry.validate(), Err(Error::FileNotFound)));
        assert_eq!(
            scan(
                vec![dir.path().into()],
                &[],
                100,
                &mut ScanReport::default()
            )
            .len(),
            0
        );
    }

    #[test]
    fn expands_home_without_shell_expansion_or_relative_roots() {
        let home = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            expand_root(Path::new("~/Documents"), Some(home.path())),
            Some(home.path().join("Documents"))
        );
        assert_eq!(
            expand_root(Path::new("~"), Some(home.path())),
            Some(home.path().to_owned())
        );
        assert_eq!(expand_root(home.path(), None), Some(home.path().to_owned()));
        assert!(expand_root(Path::new("~/Documents"), None).is_none());
        assert!(expand_root(Path::new("relative/folder"), Some(home.path())).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlink_files_directories_cycles_and_roots() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("tempdir");
        let path = write(dir.path(), "actual.txt", "");
        write(outside.path(), "outside.txt", "");
        symlink(&path, dir.path().join("link.txt")).expect("link");
        symlink(dir.path(), dir.path().join("cycle")).expect("cycle");
        symlink(outside.path(), dir.path().join("outside")).expect("outside");
        let mut report = ScanReport::default();
        let files = scan(
            vec![dir.path().into(), dir.path().join("outside")],
            &[],
            100,
            &mut report,
        );
        assert_eq!(files.len(), 1);
        assert_eq!(report.issue_count, 1);
        fs::remove_file(&path).expect("delete");
        symlink(outside.path().join("outside.txt"), &path).expect("replace with link");
        assert!(files.files[0].entry.validate().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_parent_directory_replaced_by_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "parent/file.txt", "original")
            .canonicalize()
            .unwrap();
        write(outside.path(), "file.txt", "outside");
        let entry = FileEntry::new(&path, false).unwrap();
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(outside.path(), path.parent().unwrap()).unwrap();
        assert!(entry.validate().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_paths_without_lossy_action_ids() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let path = Path::new(OsStr::from_bytes(b"/bad\xff.txt"));
        assert!(FileEntry::new(path, false).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reports_non_utf8_names_and_continues_scanning() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join(OsStr::from_bytes(b"bad\xff.txt")), "").expect("file");
        write(dir.path(), "valid.txt", "");
        let mut report = ScanReport::default();
        assert_eq!(
            scan(vec![dir.path().into()], &[], 100, &mut report).len(),
            1
        );
        assert_eq!(report.issue_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn continues_after_permission_errors() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let denied = dir.path().join("denied");
        write(dir.path(), "denied/private.txt", "");
        write(dir.path(), "available.txt", "");
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).expect("permissions");
        let cannot_read = fs::read_dir(&denied).is_err();
        let mut report = ScanReport::default();
        let files = scan(vec![dir.path().into()], &[], 100, &mut report);
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o700))
            .expect("restore permissions");
        if cannot_read {
            // The unreadable folder is listed. Its contents are not.
            assert_eq!(files.len(), 2);
            assert!(
                files
                    .get(&format!(
                        "file:{}",
                        denied.canonicalize().expect("path").display()
                    ))
                    .is_some_and(|entry| entry.folder)
            );
            assert!(report.issue_count > 0);
        }
    }
}
