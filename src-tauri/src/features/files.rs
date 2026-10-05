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

/// Whether `path` has one of the `packages` extensions, so the index lists
/// it but not its contents.
fn is_package(path: &Path, packages: &[&str]) -> bool {
    path.extension().is_some_and(|extension| {
        packages
            .iter()
            .any(|package| extension.eq_ignore_ascii_case(package))
    })
}

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
    /// Positions in `entries`, ordered by OS path, so a lookup by path (each
    /// used file in an empty Files tab, each Open) is a binary search.
    by_path: Vec<u32>,
    /// The scan stopped at [`LIMIT`].
    pub truncated: bool,
}

impl FileIndex {
    /// Walk `folders` without following links, skipping hidden entries and
    /// excluded folder names. Unreadable folders are skipped. A folder whose
    /// extension is in `packages` is listed, but not its contents. `hidden`
    /// says whether the OS hides an entry by attribute.
    pub fn scan(
        folders: &[PathBuf],
        excluded: &[String],
        packages: &[&str],
        hidden: impl Fn(&walkdir::DirEntry) -> bool,
    ) -> Self {
        let mut index = Self::default();
        'roots: for folder in distinct_roots(folders, excluded, packages) {
            let mut walker = walkdir::WalkDir::new(&folder)
                .follow_links(false)
                .min_depth(1)
                .into_iter()
                .filter_entry(|entry| {
                    let name = entry.file_name().to_string_lossy();
                    !name.starts_with('.')
                        && !(entry.file_type().is_dir() && excluded.iter().any(|ex| *ex == name))
                        && !hidden(entry)
                });
            while let Some(entry) = walker.next() {
                let Ok(entry) = entry else {
                    continue;
                };
                if index.entries.len() == LIMIT {
                    index.truncated = true;
                    break 'roots;
                }
                let file_type = entry.file_type();
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() && is_package(entry.path(), packages) {
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
        // LIMIT fits in u32.
        index.by_path = (0..index.entries.len() as u32).collect();
        let entries = &index.entries;
        index.by_path.sort_by(|&a, &b| {
            entries[a as usize]
                .os_path()
                .cmp(entries[b as usize].os_path())
        });
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
        let found = self
            .by_path
            .binary_search_by(|&i| self.entries[i as usize].os_path().cmp(path))
            .ok()?;
        Some(&self.entries[self.by_path[found] as usize])
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

/// Whether a scan of `roots` lists `path`: no folder on the way is hidden
/// (a name that starts with a dot), excluded, or a package. Attributes are
/// not read here, so on Windows a folder hidden by attribute counts as
/// listed. The last part may be a file with an excluded
/// name (`is_dir` is false), or a package itself. The caller reads `is_dir`,
/// so this stays free of disk access.
pub fn lists(
    path: &Path,
    is_dir: bool,
    roots: &[PathBuf],
    excluded: &[String],
    packages: &[&str],
) -> bool {
    roots.iter().any(|root| {
        path.strip_prefix(root).is_ok_and(|rest| {
            let parts: Vec<_> = rest.components().collect();
            parts.iter().enumerate().all(|(i, part)| {
                let name = part.as_os_str().to_string_lossy();
                let parent = i + 1 < parts.len();
                let folder = parent || is_dir;
                !name.starts_with('.')
                    && !(folder && excluded.iter().any(|skip| *skip == name))
                    && !(parent && is_package(Path::new(part.as_os_str()), packages))
            })
        })
    })
}

/// Drop folders whose contents a scan of another configured folder already
/// lists, so nothing is scanned twice and nothing is missed. Folders are
/// compared by real path: the walk follows a root that is a link but no
/// link inside it, so `~/Dropbox`, a link to a folder under `~/Library`, is
/// covered by `~` while a link to another disk is not.
fn distinct_roots(folders: &[PathBuf], excluded: &[String], packages: &[&str]) -> Vec<PathBuf> {
    let mut roots: Vec<(PathBuf, &PathBuf)> = folders
        .iter()
        .filter(|folder| folder.is_dir())
        .map(|folder| {
            (
                std::fs::canonicalize(folder).unwrap_or_else(|_| folder.clone()),
                folder,
            )
        })
        .collect();
    roots.sort();
    roots.dedup_by(|a, b| a.0 == b.0);
    let mut kept: Vec<PathBuf> = Vec::new();
    let mut distinct = Vec::new();
    for (real, folder) in roots {
        let covered = lists(&real, true, &kept, excluded, packages) && !is_package(&real, packages);
        if !covered {
            kept.push(real);
            distinct.push(folder.clone());
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
            "notes/hidden.txt",
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
            |entry| entry.file_name() == "hidden.txt",
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
        for entry in &index.entries {
            assert!(index.contains(&entry.os_path().display().to_string()));
        }
        assert!(!index.contains(&root.join("notes/missing.txt").display().to_string()));
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
    fn lists_only_what_the_scan_walks_into() {
        let roots = [PathBuf::from("/nowhere/Documents")];
        let listed = |rest: &str| {
            lists(
                &roots[0].join(rest),
                false,
                &roots,
                &["target".into()],
                &["app"],
            )
        };
        assert!(listed("notes/todo.txt"));
        assert!(listed("Tool.app"));
        assert!(listed("notes/target"));
        assert!(!listed(".git/HEAD"));
        assert!(!listed("target/debug/build"));
        assert!(!listed("Tool.app/Contents/Info.plist"));
        assert!(!lists(
            Path::new("/elsewhere/a.txt"),
            false,
            &roots,
            &[],
            &[]
        ));
    }

    #[test]
    fn nested_roots_are_scanned_once_unless_the_outer_scan_skips_them() {
        let outer = std::env::temp_dir().join(format!("tinydash-nested-{}", std::process::id()));
        let inner = |name: &str| outer.join(name);
        for name in ["docs", ".config", "target", "Tool.app"] {
            std::fs::create_dir_all(inner(name)).unwrap();
        }
        let folders: Vec<PathBuf> = ["docs", ".config", "target", "Tool.app"]
            .map(inner)
            .into_iter()
            .chain([outer.clone()])
            .collect();
        let mut kept = distinct_roots(&folders, &["target".into()], &["app"]);
        kept.sort();
        let mut expected = vec![
            outer.clone(),
            inner(".config"),
            inner("Tool.app"),
            inner("target"),
        ];
        expected.sort();
        assert_eq!(kept, expected);
        std::fs::remove_dir_all(outer).unwrap();
    }

    // Windows needs special rights to create a link.
    #[cfg(unix)]
    #[test]
    fn a_nested_root_reached_through_a_link_is_compared_by_real_path() {
        let outer = std::env::temp_dir().join(format!("tinydash-linked-{}", std::process::id()));
        let elsewhere = outer.with_extension("target");
        std::fs::create_dir_all(outer.join("Library/Dropbox")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        // A link to a folder the outer scan lists adds nothing; a link to a
        // folder outside it is scanned on its own.
        std::os::unix::fs::symlink(outer.join("Library/Dropbox"), outer.join("Dropbox")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, outer.join("External")).unwrap();
        let folders = [outer.clone(), outer.join("Dropbox"), outer.join("External")];
        let mut kept = distinct_roots(&folders, &[], &[]);
        kept.sort();
        assert_eq!(kept, [outer.clone(), outer.join("External")]);
        std::fs::remove_dir_all(&outer).unwrap();
        std::fs::remove_dir_all(elsewhere).unwrap();
    }
}
