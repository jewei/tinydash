//! File and folder names under the configured folders. The index stores
//! paths only; it never reads file contents.

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use unicode_normalization::UnicodeNormalization;

use crate::{
    platform,
    search::{
        Context,
        id::Source,
        matcher::Matcher,
        result::{Action, Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
        top,
    },
};

/// Most entries an index holds. Larger trees are cut off, and a warning is logged.
pub const LIMIT: usize = 50_000;

struct Entry {
    /// NFC path for matching. macOS can return decomposed accents.
    path: Box<str>,
    /// The path as the OS spelled it, kept only when it differs from `path`.
    original: Option<Box<str>>,
    /// Byte offset of the file name in `path`.
    name_start: usize,
    is_dir: bool,
}

impl Entry {
    fn name(&self) -> &str {
        &self.path[self.name_start..]
    }

    fn os_path(&self) -> &Path {
        Path::new(self.original.as_deref().unwrap_or(&self.path))
    }
}

#[derive(Default)]
pub struct FileIndex {
    entries: Vec<Entry>,
    /// The scan stopped at [`LIMIT`].
    pub truncated: bool,
}

impl FileIndex {
    /// Walk `folders` without following links, skipping hidden entries and
    /// excluded folder names. Unreadable folders are skipped. A folder whose
    /// extension is in `packages` is listed, but not its contents.
    pub fn scan(folders: &[PathBuf], excluded: &[String], packages: &[&str]) -> Self {
        let mut index = Self::default();
        for folder in distinct_roots(folders) {
            let mut walker = walkdir::WalkDir::new(&folder)
                .follow_links(false)
                .min_depth(1)
                .into_iter()
                .filter_entry(|entry| {
                    let name = entry.file_name().to_string_lossy();
                    !name.starts_with('.')
                        && !(entry.file_type().is_dir() && excluded.iter().any(|ex| *ex == name))
                });
            while let Some(entry) = walker.next() {
                let Ok(entry) = entry else {
                    continue;
                };
                if index.entries.len() == LIMIT {
                    index.truncated = true;
                    return index;
                }
                let file_type = entry.file_type();
                if file_type.is_symlink() {
                    continue;
                }
                let is_package = entry.path().extension().is_some_and(|extension| {
                    packages
                        .iter()
                        .any(|package| extension.eq_ignore_ascii_case(package))
                });
                if file_type.is_dir() && is_package {
                    walker.skip_current_dir();
                }
                let Some(os_path) = entry.path().to_str() else {
                    continue;
                };
                let path: String = os_path.nfc().collect();
                let original = (path != os_path).then(|| os_path.into());
                let name_start = path.rfind(std::path::MAIN_SEPARATOR).map_or(0, |i| i + 1);
                index.entries.push(Entry {
                    path: path.into(),
                    original,
                    name_start,
                    is_dir: file_type.is_dir(),
                });
            }
        }
        index
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    fn entry(&self, path: &str) -> Option<&Entry> {
        let path = Path::new(path);
        self.entries.iter().find(|entry| entry.os_path() == path)
    }

    pub fn get(&self, path: &str, ctx: &Context) -> Option<SearchResult> {
        self.entry(path)
            .map(|entry| result(entry.os_path(), entry.is_dir, ctx))
    }

    /// Match names; with two or more words or a separator, also match whole
    /// paths, so `project readme` finds `project/README.md`.
    pub fn search(&self, matcher: &mut Matcher, ctx: &Context, limit: usize) -> Vec<Scored> {
        let query = matcher.query();
        let match_paths = query.contains(' ') || query.contains(['/', '\\']);
        let mut hits = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let score = matcher
                .name(entry.name())
                .or_else(|| match_paths.then(|| matcher.words(&entry.path)).flatten());
            if let Some(score) = score {
                hits.push((score, index));
            }
        }
        // Rank by match first, then apply usage to the best candidates only.
        let hits = top(hits, limit * 4)
            .into_iter()
            .map(|(score, index)| {
                let entry = &self.entries[index];
                (score + ctx.boost(&id(entry.os_path())), entry)
            })
            .collect();
        top(hits, limit)
            .into_iter()
            .map(|(score, entry)| Scored {
                score,
                result: result(entry.os_path(), entry.is_dir, ctx),
            })
            .collect()
    }
}

/// Drop folders that sit inside another configured folder.
fn distinct_roots(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = folders.iter().filter(|f| f.is_dir()).cloned().collect();
    roots.sort();
    roots.dedup();
    let mut distinct: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !distinct.iter().any(|kept| root.starts_with(kept)) {
            distinct.push(root);
        }
    }
    distinct
}

/// A path for display: NFC, with the home folder shown as `~`.
pub fn display_path(path: &Path) -> String {
    static HOME: LazyLock<Option<PathBuf>> = LazyLock::new(std::env::home_dir);
    let shown = match HOME
        .as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
    };
    shown.nfc().collect()
}

fn id(path: &Path) -> String {
    Source::File.id(path.display())
}

fn result(path: &Path, is_dir: bool, ctx: &Context) -> SearchResult {
    let id = id(path);
    let text = path.display().to_string();
    let name = path.file_name().map_or_else(
        || text.clone(),
        |name| name.to_string_lossy().nfc().collect(),
    );
    let parent = path.parent().map(display_path).unwrap_or_default();
    SearchResult {
        kind: if is_dir {
            ResultKind::Folder
        } else {
            ResultKind::File
        },
        title: name,
        subtitle: parent,
        icon: if platform::NATIVE_ICONS {
            Icon::File { path: text.clone() }
        } else {
            Icon::Symbol {
                name: if is_dir { Symbol::Folder } else { Symbol::File },
            }
        },
        actions: vec![
            ResultAction::new("Open", Action::Open { path: text.clone() }),
            ResultAction::new(
                format!("Show in {}", platform::FILE_MANAGER),
                Action::Reveal { path: text.clone() },
            ),
            ResultAction::new("Copy Path", Action::Copy { text }),
            ctx.pin_action(&id),
        ],
        pinned: ctx.pinned(&id),
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::usage::{Pins, Usage};

    fn tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("tinydash-files-{}", std::process::id()));
        for dir in [
            "docs/project",
            "node_modules/pkg",
            ".hidden",
            "notes",
            "Tool.APP/Contents",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            "docs/project/README.md",
            "node_modules/pkg/index.js",
            ".hidden/secret.txt",
            "notes/todo.txt",
            "Tool.APP/Contents/Info.plist",
        ] {
            std::fs::write(root.join(file), "").unwrap();
        }
        root
    }

    #[test]
    fn scans_names_skips_hidden_and_excluded_and_matches_paths() {
        let root = tree();
        let index = FileIndex::scan(
            &[root.clone(), root.join("notes")],
            &["node_modules".into()],
            &["app"],
        );
        let mut names: Vec<_> = index.entries.iter().map(Entry::name).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "README.md",
                "Tool.APP",
                "docs",
                "notes",
                "project",
                "todo.txt"
            ]
        );
        assert!(!index.truncated);

        let (usage, pins) = (Usage::default(), Pins::default());
        let ctx = Context {
            usage: &usage,
            pins: &pins,
            now: 0,
            skin_tone: 0,
        };
        let titles = |query: &str| -> Vec<String> {
            index
                .search(&mut Matcher::new(query), &ctx, 10)
                .into_iter()
                .map(|hit| hit.result.title)
                .collect()
        };
        assert_eq!(titles("readme"), ["README.md"]);
        assert_eq!(titles("project readme")[0], "README.md");
        assert!(index.contains(&root.join("notes/todo.txt").display().to_string()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn display_paths_shorten_the_home_folder() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(display_path(&home), "~");
        assert!(display_path(&home.join("Notes")).starts_with('~'));
        assert_eq!(display_path(Path::new("/Applications")), "/Applications");
    }

    #[test]
    fn nested_roots_are_scanned_once() {
        let outer = std::env::temp_dir();
        let inner = outer.join(format!("tinydash-nested-{}", std::process::id()));
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(distinct_roots(&[inner.clone(), outer.clone()]), [outer]);
        std::fs::remove_dir_all(inner).unwrap();
    }
}
