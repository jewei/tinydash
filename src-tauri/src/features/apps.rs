//! Installed applications. The platform layer discovers them; this module
//! indexes and ranks them.

use unicode_normalization::UnicodeNormalization;

use std::path::Path;

use crate::{
    actions::Action,
    features::files,
    platform,
    search::{
        Context,
        matcher::Matcher,
        result::{Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
        top,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub name: String,
    /// Bundle, shortcut, or desktop entry path. Launching goes through the platform.
    pub path: String,
    /// Other names that should find the app, such as its executable.
    pub aliases: Vec<String>,
}

#[derive(Default)]
pub struct AppIndex {
    apps: Vec<App>,
}

impl AppIndex {
    /// Sort by name and keep one entry per path.
    pub fn new(apps: Vec<App>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut apps: Vec<App> = apps
            .into_iter()
            .filter(|app| seen.insert(app.path.clone()))
            .map(|app| App {
                name: app.name.nfc().collect(),
                ..app
            })
            .collect();
        apps.sort_by_cached_key(|app| (app.name.to_lowercase(), app.path.clone()));
        Self { apps }
    }

    pub fn len(&self) -> usize {
        self.apps.len()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.apps.iter().any(|app| app.path == path)
    }

    pub fn search(&self, matcher: &mut Matcher, ctx: &Context, limit: usize) -> Vec<Scored> {
        let hits = self
            .apps
            .iter()
            .filter_map(|app| {
                let score = matcher.best(&app.name, app.aliases.iter().map(String::as_str))?;
                Some((score + ctx.boost(&id(app)), app))
            })
            .collect();
        top(hits, limit)
            .into_iter()
            .map(|(score, app)| Scored {
                score,
                result: result(app, ctx),
            })
            .collect()
    }

    /// Every app, most used first, then by name.
    pub fn browse(&self, ctx: &Context) -> Vec<SearchResult> {
        let mut apps: Vec<_> = self.apps.iter().collect();
        apps.sort_by_key(|app| std::cmp::Reverse(ctx.boost(&id(app))));
        apps.into_iter().map(|app| result(app, ctx)).collect()
    }

    pub fn get(&self, path: &str, ctx: &Context) -> Option<SearchResult> {
        self.apps
            .iter()
            .find(|app| app.path == path)
            .map(|app| result(app, ctx))
    }
}

fn id(app: &App) -> String {
    format!("app:{}", app.path)
}

fn result(app: &App, ctx: &Context) -> SearchResult {
    let id = id(app);
    SearchResult {
        kind: ResultKind::App,
        title: app.name.clone(),
        subtitle: Path::new(&app.path)
            .parent()
            .map(files::display_path)
            .unwrap_or_default(),
        icon: if platform::NATIVE_ICONS {
            Icon::File {
                path: app.path.clone(),
            }
        } else {
            Icon::Symbol { name: Symbol::App }
        },
        actions: vec![
            ResultAction::new(
                "Open",
                Action::Launch {
                    path: app.path.clone(),
                },
            ),
            ResultAction::new(
                format!("Show in {}", platform::FILE_MANAGER),
                Action::Reveal {
                    path: app.path.clone(),
                },
            ),
            ResultAction::new(
                "Copy Path",
                Action::Copy {
                    text: app.path.clone(),
                },
            ),
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

    fn app(name: &str, path: &str) -> App {
        App {
            name: name.into(),
            path: path.into(),
            aliases: vec![],
        }
    }

    #[test]
    fn index_sorts_by_name_and_drops_duplicate_paths() {
        let index = AppIndex::new(vec![
            app("zed", "/z"),
            app("Arc", "/a"),
            app("Arc copy", "/a"),
        ]);
        let names: Vec<_> = index.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Arc", "zed"]);
    }

    #[test]
    fn usage_orders_equal_matches() {
        let index = AppIndex::new(vec![app("Notes", "/n"), app("Notion", "/o")]);
        let mut usage = Usage::default();
        usage.record("app:/o", 0);
        let pins = Pins::default();
        let ctx = Context {
            usage: &usage,
            pins: &pins,
            now: 0,
            skin_tone: 0,
        };
        let hits = index.search(&mut Matcher::new("no"), &ctx, 10);
        assert_eq!(hits[0].result.title, "Notion");
        assert_eq!(index.browse(&ctx)[0].title, "Notion");
    }
}
