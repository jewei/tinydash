use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    features::{
        datetime::{self, MAX_CLOCK_CITIES},
        emoji::EmojiLanguage,
        web::SearchEngine,
    },
    search::Category,
};

pub const FILE_NAME: &str = "settings.json";
pub const CLIPBOARD_LIMIT_MAX: u32 = 1000;
/// Most entries in each folder list. On Linux each indexed folder uses a
/// watch from a per-user limit.
const FOLDER_LIST_MAX: usize = 50;

/// User preferences, stored as camelCase JSON in the app config folder.
/// Missing fields take their defaults, so older and newer files both load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    /// Global shortcut that toggles the launcher, in Tauri accelerator syntax.
    pub shortcut: String,
    pub theme: Theme,
    pub hide_on_blur: bool,
    pub launch_at_login: bool,
    pub show_tray_icon: bool,
    pub clipboard_history_enabled: bool,
    pub clipboard_history_limit: u32,
    pub clipboard_capture_images: bool,
    pub clipboard_capture_files: bool,
    /// Folders to index. A leading `~` means the home folder.
    pub file_search_folders: Vec<String>,
    /// Folder names that are never indexed, at any depth.
    pub file_search_excluded_dirs: Vec<String>,
    /// 0 is the default yellow; 1–5 are light to dark.
    pub emoji_skin_tone: u8,
    pub emoji_languages: Vec<EmojiLanguage>,
    pub currency_rates_enabled: bool,
    /// Look for a newer version when the launcher opens, at most every few
    /// hours. Only builds that can update themselves look.
    pub check_for_updates: bool,
    pub search_engine: SearchEngine,
    /// Where the user dragged the launcher, in the units of
    /// `platform::launcher_position`. `None` centers it on the screen with
    /// the pointer. A spot no longer on any screen counts as `None`.
    pub launcher_position: Option<LauncherPosition>,
    /// Every tab after All, in launcher order, whether it shows, and
    /// whether its results join All. All is always first, so it is never
    /// listed.
    pub tabs: Vec<LauncherTab>,
    /// The widget pane shows to the right of an empty All search. Each
    /// widget has its own switch.
    pub show_clocks: bool,
    /// Places the clocks widget shows next to local time, as typed.
    pub clock_cities: Vec<String>,
}

/// The launcher's top-left corner: logical points on macOS, physical pixels
/// on Windows and Linux.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LauncherPosition {
    pub x: f64,
    pub y: f64,
}

/// One of the launcher's tabs after All.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LauncherTab {
    pub category: Category,
    pub shown: bool,
    /// Its search results and suggestions show in All. Out of All, its pins
    /// still head the empty All view (pinned clips never do).
    /// Missing in files from before the switch, which meant yes.
    #[serde(default = "LauncherTab::joins_all_by_default")]
    pub in_all: bool,
}

impl LauncherTab {
    fn joins_all_by_default() -> bool {
        true
    }
}

/// Drop the saved tabs this version cannot read, such as a typo or a tab
/// from a newer version, so one bad entry never resets every setting.
/// `normalized` then adds any tab that is missing.
fn drop_unreadable_tabs(file: &mut Value) {
    if let Some(Value::Array(tabs)) = file.get_mut("tabs") {
        tabs.retain(|tab| serde_json::from_value::<LauncherTab>(tab.clone()).is_ok());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: "Control+Shift+Space".into(),
            theme: Theme::System,
            hide_on_blur: true,
            launch_at_login: false,
            // macOS users reach TinyDash from the shortcut; the menu bar is crowded.
            show_tray_icon: !cfg!(target_os = "macos"),
            clipboard_history_enabled: false,
            clipboard_history_limit: 200,
            clipboard_capture_images: false,
            clipboard_capture_files: false,
            file_search_folders: ["~/Desktop", "~/Documents", "~/Downloads"]
                .map(String::from)
                .into(),
            file_search_excluded_dirs: ["node_modules", "target"].map(String::from).into(),
            emoji_skin_tone: 0,
            emoji_languages: Vec::new(),
            currency_rates_enabled: true,
            check_for_updates: true,
            search_engine: SearchEngine::Google,
            launcher_position: None,
            tabs: Category::ALL[1..]
                .iter()
                .map(|&category| LauncherTab {
                    category,
                    shown: true,
                    in_all: true,
                })
                .collect(),
            show_clocks: true,
            clock_cities: Vec::new(),
        }
    }
}

impl Settings {
    /// These settings with `changes`, camelCase fields as the windows send
    /// them, on top. Each window sends only what it changed, so two windows
    /// that save at once do not undo each other.
    pub fn with_changes(&self, changes: Map<String, Value>) -> Result<Self> {
        let Ok(Value::Object(mut fields)) = serde_json::to_value(self) else {
            return Err(Error::msg("Invalid settings."));
        };
        let mut merged = self.clone();
        // One field at a time, so an error names the setting it is about;
        // serde's own message does not.
        for (name, value) in changes {
            if !fields.contains_key(&name) {
                return Err(Error::msg(format!("Unknown setting “{name}”.")));
            }
            fields.insert(name.clone(), value);
            merged = serde_json::from_value(Value::Object(fields.clone())).map_err(|error| {
                Error::msg(format!("Invalid value for the setting “{name}”: {error}"))
            })?;
        }
        Ok(merged)
    }

    /// Clamp numbers, drop blank or duplicate list entries, and cut lists
    /// to their limit. `tabs` loses All, and gains any tab it lacks (such
    /// as one added in a newer version) at the end, shown and in All. A tab
    /// that is hidden stays in All.
    pub fn normalized(mut self) -> Self {
        self.shortcut = self.shortcut.trim().to_owned();
        self.clipboard_history_limit = self.clipboard_history_limit.clamp(1, CLIPBOARD_LIMIT_MAX);
        self.emoji_skin_tone = self.emoji_skin_tone.min(5);
        self.emoji_languages.sort();
        self.emoji_languages.dedup();
        // Each tab once, in the saved order; a tab the list lacks, such as
        // one added in a newer version, joins at the end and shows.
        let mut listed = std::collections::HashSet::new();
        self.tabs
            .retain(|tab| tab.category != Category::All && listed.insert(tab.category));
        for &category in &Category::ALL[1..] {
            if listed.insert(category) {
                self.tabs.push(LauncherTab {
                    category,
                    shown: true,
                    in_all: true,
                });
            }
        }
        // A tab neither shown nor in All would leave its results nowhere.
        for tab in &mut self.tabs {
            tab.in_all |= !tab.shown;
        }
        for list in [
            &mut self.file_search_folders,
            &mut self.file_search_excluded_dirs,
        ] {
            let mut seen = std::collections::HashSet::new();
            list.retain_mut(|item| {
                *item = item.trim().to_owned();
                !item.is_empty() && seen.insert(item.clone())
            });
            list.truncate(FOLDER_LIST_MAX);
        }
        let mut seen = std::collections::HashSet::new();
        self.clock_cities.retain_mut(|city| {
            *city = city.trim().to_owned();
            !city.is_empty() && seen.insert(city.to_lowercase())
        });
        self.clock_cities.truncate(MAX_CLOCK_CITIES);
        self
    }

    /// The first clock city that names no time zone. A hand-edited file may
    /// still hold one; the widget skips it.
    pub fn unknown_clock_city(&self) -> Option<&str> {
        let now = chrono::Local::now();
        self.clock_cities
            .iter()
            .find(|city| datetime::city_clock(city, now).is_none())
            .map(String::as_str)
    }

    /// Whether a category's results and suggestions show in All. A category
    /// with no entry, which only All itself is after `normalized`, does.
    pub fn in_all(&self, category: Category) -> bool {
        self.tabs
            .iter()
            .find(|tab| tab.category == category)
            .is_none_or(|tab| tab.in_all)
    }

    /// The first folder entry that is not a full path, which would never be
    /// indexed. A hand-edited file may still hold one; `file_folders` skips it.
    pub fn relative_folder(&self, home: &Path) -> Option<&str> {
        self.file_search_folders
            .iter()
            .find(|folder| !expand_home(folder, home).is_absolute())
            .map(String::as_str)
    }

    /// Indexed folders as absolute paths. Relative entries are ignored.
    pub fn file_folders(&self, home: &Path) -> Vec<PathBuf> {
        self.file_search_folders
            .iter()
            .map(|folder| expand_home(folder, home))
            .filter(|path| path.is_absolute())
            .collect()
    }
}

/// `~` or `~/rest` relative to `home`; other paths unchanged.
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix('~') {
        Some("") => home.to_owned(),
        Some(rest) if rest.starts_with(['/', '\\']) => home.join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

/// Load settings. A damaged file is renamed aside, and defaults are used,
/// so a bad edit never stops the launcher; the warning tells the user.
pub fn load(dir: &Path) -> (Settings, Option<String>) {
    let path = dir.join(FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), None);
        }
        Err(error) => {
            return (
                Settings::default(),
                Some(format!("Could not read settings: {error}")),
            );
        }
    };
    let parsed = serde_json::from_str::<Value>(&text).and_then(|mut file| {
        drop_unreadable_tabs(&mut file);
        serde_json::from_value::<Settings>(file)
    });
    match parsed {
        Ok(settings) => (settings.normalized(), None),
        Err(error) => {
            let backup = dir.join("settings.invalid.json");
            let moved = std::fs::rename(&path, &backup).is_ok();
            let note = if moved {
                format!(" The old file is at {}.", backup.display())
            } else {
                String::new()
            };
            (
                Settings::default(),
                Some(format!(
                    "Settings were invalid ({error}), so defaults are in use.{note}"
                )),
            )
        }
    }
}

pub fn save(dir: &Path, settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&temporary, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(&temporary, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_replace_only_their_fields() {
        let old = Settings {
            launch_at_login: true,
            ..Settings::default()
        };
        let changes = serde_json::json!({ "clipboardHistoryEnabled": false });
        let Value::Object(changes) = changes else {
            unreachable!()
        };
        let new = old.with_changes(changes).unwrap();
        assert!(!new.clipboard_history_enabled);
        assert!(new.launch_at_login);

        let Value::Object(unknown) = serde_json::json!({ "appearance": "sage" }) else {
            unreachable!()
        };
        assert!(old.with_changes(unknown).is_err());
        let Value::Object(wrong) = serde_json::json!({ "hideOnBlur": false, "theme": "sage" })
        else {
            unreachable!()
        };
        let error = old.with_changes(wrong).unwrap_err().to_string();
        assert!(error.contains("“theme”"), "{error}");
    }

    #[test]
    fn an_unreadable_tab_is_dropped_without_resetting_the_rest() {
        let dir = std::env::temp_dir().join(format!("tinydash-tabs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE_NAME),
            r#"{"hideOnBlur": false, "tabs": [{"category": "notes", "shown": true},
                {"category": "emoji", "shown": false}, "clipbord"]}"#,
        )
        .unwrap();
        let (settings, warning) = load(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(warning, None);
        assert!(!settings.hide_on_blur);
        assert_eq!(
            settings.tabs[0],
            LauncherTab {
                category: Category::Emoji,
                shown: false,
                in_all: true
            }
        );
        assert_eq!(settings.tabs.len(), Category::ALL.len() - 1);
    }

    #[test]
    fn missing_fields_take_defaults_and_unknown_fields_are_ignored() {
        let settings: Settings =
            serde_json::from_str(r#"{"hideOnBlur": false, "appearance": "sage"}"#).unwrap();
        assert!(!settings.hide_on_blur);
        assert_eq!(settings.shortcut, Settings::default().shortcut);
    }

    #[test]
    fn a_dragged_position_saves_and_clears_like_any_setting() {
        let Value::Object(moved) =
            serde_json::json!({ "launcherPosition": { "x": 40.5, "y": -900.0 } })
        else {
            unreachable!()
        };
        let moved = Settings::default().with_changes(moved).unwrap();
        assert_eq!(
            moved.launcher_position,
            Some(LauncherPosition { x: 40.5, y: -900.0 })
        );
        let Value::Object(reset) = serde_json::json!({ "launcherPosition": null }) else {
            unreachable!()
        };
        assert_eq!(moved.with_changes(reset).unwrap().launcher_position, None);
    }

    #[test]
    fn normalizing_clamps_and_deduplicates() {
        let settings = Settings {
            clipboard_history_limit: 0,
            emoji_skin_tone: 9,
            emoji_languages: vec![EmojiLanguage::Zh, EmojiLanguage::Es, EmojiLanguage::Zh],
            file_search_folders: vec![" ~/A ".into(), "~/A".into(), "  ".into()],
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.clipboard_history_limit, 1);
        assert_eq!(settings.emoji_skin_tone, 5);
        assert_eq!(
            settings.emoji_languages,
            [EmojiLanguage::Zh, EmojiLanguage::Es]
        );
        assert_eq!(settings.file_search_folders, ["~/A"]);

        let cities = Settings {
            clock_cities: [" Tokyo", "tokyo", "", "London", "Paris", "Lima"]
                .map(String::from)
                .into(),
            ..Settings::default()
        }
        .normalized()
        .clock_cities;
        assert_eq!(cities, ["Tokyo", "London", "Paris"]);

        let tab = |category, shown| LauncherTab {
            category,
            shown,
            in_all: true,
        };
        let tabs = Settings {
            tabs: vec![
                tab(Category::Emoji, false),
                tab(Category::All, true),
                tab(Category::Apps, true),
                tab(Category::Emoji, true),
            ],
            ..Settings::default()
        }
        .normalized()
        .tabs;
        assert_eq!(
            tabs[..2],
            [tab(Category::Emoji, false), tab(Category::Apps, true)]
        );
        // A tab that is neither shown nor in All would be unreachable.
        let hidden = Settings {
            tabs: vec![LauncherTab {
                category: Category::Files,
                shown: false,
                in_all: false,
            }],
            ..Settings::default()
        }
        .normalized();
        assert!(hidden.tabs[0].in_all && hidden.in_all(Category::Files));
        let old: LauncherTab =
            serde_json::from_str(r#"{"category": "apps", "shown": true}"#).unwrap();
        assert!(old.in_all);
        // The tabs the list lacked follow, shown, in their usual order.
        let rest: Vec<_> = tabs[2..]
            .iter()
            .map(|tab| (tab.category, tab.shown))
            .collect();
        assert_eq!(
            rest,
            [
                Category::Files,
                Category::Clipboard,
                Category::Snippets,
                Category::System
            ]
            .map(|category| (category, true))
        );

        let many = Settings {
            file_search_excluded_dirs: (0..99).map(|n| format!("dir{n}")).collect(),
            ..Settings::default()
        }
        .normalized();
        assert_eq!(many.file_search_excluded_dirs.len(), FOLDER_LIST_MAX);
        assert_eq!(many.file_search_excluded_dirs[0], "dir0");
    }

    #[test]
    fn finds_a_folder_that_is_not_a_full_path() {
        let home = std::env::temp_dir();
        let settings = |folders: &[&str]| Settings {
            file_search_folders: folders.iter().map(|f| String::from(*f)).collect(),
            ..Settings::default()
        };
        assert_eq!(
            settings(&["~/Notes", "Projects"]).relative_folder(&home),
            Some("Projects")
        );
        assert_eq!(settings(&["~", "~/Notes"]).relative_folder(&home), None);
    }

    #[test]
    fn finds_a_clock_city_that_names_no_time_zone() {
        let settings = |cities: &[&str]| Settings {
            clock_cities: cities.iter().map(|city| String::from(*city)).collect(),
            ..Settings::default()
        };
        assert_eq!(settings(&["Tokyo", "kl"]).unknown_clock_city(), None);
        assert_eq!(
            settings(&["Tokyo", "Atlantis"]).unknown_clock_city(),
            Some("Atlantis")
        );
    }

    #[test]
    fn expands_the_home_folder_and_drops_relative_paths() {
        let home = std::env::temp_dir();
        let settings = Settings {
            file_search_folders: vec!["~".into(), "~/Notes".into(), "relative".into()],
            ..Settings::default()
        };
        assert_eq!(
            settings.file_folders(&home),
            [home.clone(), home.join("Notes")]
        );
    }

    #[test]
    fn a_damaged_file_is_moved_aside_and_defaults_are_used() {
        let dir = std::env::temp_dir().join(format!("tinydash-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "{ not json").unwrap();
        let (settings, warning) = load(&dir);
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
        assert!(dir.join("settings.invalid.json").exists());
        save(&dir, &settings).unwrap();
        assert_eq!(load(&dir), (Settings::default(), None));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
