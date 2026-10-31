use std::{
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    features::{
        datetime::{self, MAX_CLOCK_CITIES},
        emoji::EmojiLanguage,
        weather::{MAX_CITY_CHARS, TemperatureUnit},
        web::SearchEngine,
    },
    search::Category,
};

pub const FILE_NAME: &str = "settings.json";
pub const CLIPBOARD_LIMIT_MAX: u32 = 1000;
pub const FOCUS_MINUTES_MAX: u16 = 120;
pub const BREAK_MINUTES_MAX: u16 = 60;
pub const SESSIONS_MAX: u8 = 8;
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
    /// The Files tab also asks the OS's file index (Spotlight on macOS) for
    /// names under the home folder. Off by default.
    pub spotlight_files: bool,
    /// 0 is the default yellow; 1–5 are light to dark.
    pub emoji_skin_tone: u8,
    pub emoji_languages: Vec<EmojiLanguage>,
    pub currency_rates_enabled: bool,
    /// Look for a newer version when the launcher opens, at most every few
    /// hours. Only builds that can update themselves look.
    pub check_for_updates: bool,
    pub search_engine: SearchEngine,
    /// `>`, a space, and a command run that command in the terminal. Off by
    /// default.
    pub shell_commands: bool,
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
    /// The rest of today's events and the reminders due today (macOS).
    /// Off by default: turning it on asks the OS for access to them.
    pub show_up_next: bool,
    pub show_disk_space: bool,
    pub show_notepad: bool,
    pub show_focus_timer: bool,
    pub focus_minutes: u16,
    pub short_break_minutes: u16,
    pub long_break_minutes: u16,
    /// Focus sessions in a cycle; the last one earns the long break.
    pub sessions_before_long_break: u8,
    pub show_weather: bool,
    /// The city of the weather widget, as typed; empty until one is set.
    pub weather_city: String,
    pub temperature_unit: TemperatureUnit,
    /// A card for a color, a Unix time, or JSON on the clipboard. The text
    /// is read when the pane shows, never saved. Off by default, as
    /// clipboard history is: macOS may ask before an app reads the clipboard.
    pub show_clipboard_cards: bool,
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

/// Drop the saved values this version cannot read, such as a typo or a
/// choice from a newer version, so one bad value never resets every
/// setting. A list loses only its bad entries; any other field takes its
/// default. `normalized` then adds any tab that is missing. Returns the
/// names of the fields that lost a value, so `load` can tell the user.
fn drop_unreadable_values(file: &mut Value) -> Vec<String> {
    let Value::Object(fields) = file else {
        return Vec::new();
    };
    let mut dropped = Vec::new();
    if keep_readable_entries::<LauncherTab>(fields, "tabs") {
        dropped.push("tabs".to_owned());
    }
    if keep_readable_entries::<EmojiLanguage>(fields, "emojiLanguages") {
        dropped.push("emojiLanguages".to_owned());
    }
    let Ok(Value::Object(defaults)) = serde_json::to_value(Settings::default()) else {
        return dropped;
    };
    // Each field on top of the defaults alone, so a bad one cannot hide
    // behind another.
    fields.retain(|name, value| {
        if !defaults.contains_key(name) {
            return true;
        }
        let mut alone = defaults.clone();
        alone.insert(name.clone(), value.clone());
        let readable = serde_json::from_value::<Settings>(Value::Object(alone)).is_ok();
        if !readable {
            dropped.push(name.clone());
        }
        readable
    });
    dropped
}

/// Keep only the entries of the list `name` that read as `T`. Returns
/// whether any entry was removed.
fn keep_readable_entries<T: serde::de::DeserializeOwned>(
    fields: &mut Map<String, Value>,
    name: &str,
) -> bool {
    let Some(Value::Array(entries)) = fields.get_mut(name) else {
        return false;
    };
    let before = entries.len();
    entries.retain(|entry| serde_json::from_value::<T>(entry.clone()).is_ok());
    entries.len() != before
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
            spotlight_files: false,
            emoji_skin_tone: 0,
            emoji_languages: Vec::new(),
            // Off until the user turns it on: it is the one search
            // feature that downloads.
            currency_rates_enabled: false,
            check_for_updates: true,
            search_engine: SearchEngine::Google,
            shell_commands: false,
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
            show_up_next: false,
            show_disk_space: true,
            show_notepad: false,
            show_focus_timer: false,
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            sessions_before_long_break: 4,
            show_weather: false,
            weather_city: String::new(),
            temperature_unit: TemperatureUnit::Celsius,
            show_clipboard_cards: false,
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
        self.focus_minutes = self.focus_minutes.clamp(1, FOCUS_MINUTES_MAX);
        self.short_break_minutes = self.short_break_minutes.clamp(1, BREAK_MINUTES_MAX);
        self.long_break_minutes = self.long_break_minutes.clamp(1, BREAK_MINUTES_MAX);
        self.sessions_before_long_break = self.sessions_before_long_break.clamp(1, SESSIONS_MAX);
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
        self.weather_city = self
            .weather_city
            .trim()
            .chars()
            .take(MAX_CITY_CHARS)
            .collect();
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

/// Settings read at startup, with the file to save changes to.
pub struct Loaded {
    pub settings: Settings,
    pub warning: Option<String>,
    pub file: SettingsFile,
}

/// The settings file. It refuses to save after a read failed for a reason
/// other than its content, such as a lock held by a sync or antivirus tool:
/// that file may still hold the user's settings, and this session has only
/// defaults to write over it.
pub struct SettingsFile {
    dir: PathBuf,
    replaceable: bool,
}

impl SettingsFile {
    #[cfg(test)]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            replaceable: true,
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        self.save_with(settings, std::fs::File::sync_all)
    }

    /// `save` passes `File::sync_all`. A test passes a sync that fails,
    /// because the local disk of a test run accepts every sync.
    fn save_with(
        &self,
        settings: &Settings,
        sync: fn(&std::fs::File) -> std::io::Result<()>,
    ) -> Result<()> {
        if !self.replaceable {
            return Err(Error::msg(format!(
                "TinyDash could not use {} at startup, so it did not replace it. Quit and reopen TinyDash to change settings.",
                self.dir.join(FILE_NAME).display()
            )));
        }
        let path = self.dir.join(FILE_NAME);
        let temporary = self.dir.join(format!("{FILE_NAME}.tmp"));
        let text = serde_json::to_vec_pretty(settings)?;
        std::fs::create_dir_all(&self.dir)
            .and_then(|()| {
                let mut file = std::fs::File::create(&temporary)?;
                file.write_all(&text)?;
                // The data should be on disk before the rename is: after a
                // crash, a renamed but empty file would load as defaults. Some
                // volumes refuse the sync, as SMB shares refuse the full sync
                // of macOS. The save still goes ahead there, as it did before
                // the sync was added, so that no settings change fails on
                // such a volume.
                if let Err(error) = sync(&file) {
                    tracing::warn!(%error, "Could not write the settings to disk before replacing them");
                }
                Ok(())
            })
            .and_then(|()| std::fs::rename(&temporary, &path))
            .map_err(|error| save_failed(&self.dir, &error))
    }
}

/// Why settings could not be saved. Here the folder is known, so a refused
/// write or a full disk can say what to do; other causes have no step that
/// fits them all.
fn save_failed(dir: &Path, error: &std::io::Error) -> Error {
    use std::io::ErrorKind;
    let step = match error.kind() {
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => {
            " Free some disk space, then try again.".to_owned()
        }
        // Only Windows refuses to replace a file that another app holds open.
        ErrorKind::PermissionDenied if cfg!(windows) => format!(
            " Check that you can write to {} and that no other app has {FILE_NAME} open.",
            dir.display()
        ),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => {
            format!(" Check that you can write to {}.", dir.display())
        }
        _ => String::new(),
    };
    Error::msg(format!(
        "Could not save settings in {}: {error}.{step}",
        dir.display()
    ))
}

/// Load settings. A damaged file is renamed aside, and defaults are used,
/// so a bad edit never stops the launcher; the warning tells the user.
/// A value this version cannot read only takes its default; the file is
/// copied aside, and the warning names the field.
pub fn load(dir: &Path) -> Loaded {
    let path = dir.join(FILE_NAME);
    let loaded = |settings, warning, replaceable| Loaded {
        settings,
        warning,
        file: SettingsFile {
            dir: dir.to_owned(),
            replaceable,
        },
    };
    let mut dropped = Vec::new();
    let parsed = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Value>(&text)
            .and_then(|mut file| {
                dropped = drop_unreadable_values(&mut file);
                serde_json::from_value::<Settings>(file)
            })
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return loaded(Settings::default(), None, true);
        }
        // Not UTF-8: damaged content, like a parse error.
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => Err(error.to_string()),
        Err(error) => {
            return loaded(
                Settings::default(),
                Some(format!(
                    "Could not read settings ({error}), so defaults are in use and changes will not be saved. Quit and reopen TinyDash to try again."
                )),
                false,
            );
        }
    };
    let backup = dir.join("settings.invalid.json");
    match parsed {
        Ok(settings) => {
            // The next save writes the defaults over the dropped values, so
            // keep a copy for a newer version or for the user to restore.
            let warning = (!dropped.is_empty()).then(|| {
                let note = if std::fs::copy(&path, &backup).is_ok() {
                    format!(" The old file is at {}.", backup.display())
                } else {
                    String::new()
                };
                format!(
                    "Some values under these names in settings.json could not be read: {}. \
                     TinyDash skipped only those values and uses defaults for them.{note}",
                    dropped.join(", ")
                )
            });
            loaded(settings.normalized(), warning, true)
        }
        Err(error) => {
            let moved = std::fs::rename(&path, &backup).is_ok();
            // Without a backup, a save would put defaults over the only copy.
            let note = if moved {
                format!(" The old file is at {}.", backup.display())
            } else {
                " Changes will not be saved. Fix or remove the file, then reopen TinyDash."
                    .to_owned()
            };
            loaded(
                Settings::default(),
                Some(format!(
                    "Settings were invalid ({error}), so defaults are in use.{note}"
                )),
                moved,
            )
        }
    }
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
        let Loaded {
            settings, warning, ..
        } = load(&dir);
        let backup = dir.join("settings.invalid.json");
        assert_eq!(
            warning,
            Some(format!(
                "Some values under these names in settings.json could not be read: tabs. \
                 TinyDash skipped only those values and uses defaults for them. \
                 The old file is at {}.",
                backup.display()
            ))
        );
        assert!(
            std::fs::read_to_string(&backup)
                .unwrap()
                .contains("clipbord")
        );
        assert!(dir.join(FILE_NAME).exists());
        std::fs::remove_dir_all(&dir).unwrap();
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
    fn a_value_from_a_newer_version_takes_its_default_without_resetting_the_rest() {
        let dir = std::env::temp_dir().join(format!("tinydash-newer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE_NAME),
            r#"{"shortcut": "Alt+Space", "searchEngine": "kagi",
                "emojiLanguages": ["zh", "xx-unknown"], "focusMinutes": "soon"}"#,
        )
        .unwrap();
        let Loaded {
            settings, warning, ..
        } = load(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        let warning = warning.unwrap();
        for field in ["emojiLanguages", "searchEngine", "focusMinutes"] {
            assert!(warning.contains(field), "{warning}");
        }
        assert!(!warning.contains("shortcut"), "{warning}");
        assert_eq!(settings.shortcut, "Alt+Space");
        assert_eq!(settings.search_engine, Settings::default().search_engine);
        assert_eq!(settings.emoji_languages, [EmojiLanguage::Zh]);
        assert_eq!(settings.focus_minutes, Settings::default().focus_minutes);
    }

    #[test]
    fn a_file_that_is_not_an_object_is_moved_aside() {
        let dir = std::env::temp_dir().join(format!("tinydash-array-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "[1, 2]").unwrap();
        let Loaded {
            settings, warning, ..
        } = load(&dir);
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
        assert!(dir.join("settings.invalid.json").exists());
        std::fs::remove_dir_all(dir).unwrap();
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
            focus_minutes: 0,
            long_break_minutes: 999,
            sessions_before_long_break: 0,
            emoji_languages: vec![EmojiLanguage::Zh, EmojiLanguage::Es, EmojiLanguage::Zh],
            file_search_folders: vec![" ~/A ".into(), "~/A".into(), "  ".into()],
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.clipboard_history_limit, 1);
        assert_eq!(settings.emoji_skin_tone, 5);
        assert_eq!(settings.focus_minutes, 1);
        assert_eq!(settings.long_break_minutes, BREAK_MINUTES_MAX);
        assert_eq!(settings.sessions_before_long_break, 1);
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

        let city = Settings {
            weather_city: format!("  Kuala Lumpur{}", " ".repeat(200)),
            ..Settings::default()
        }
        .normalized()
        .weather_city;
        assert_eq!(city, "Kuala Lumpur");

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
        let Loaded {
            settings,
            warning,
            file,
        } = load(&dir);
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
        assert!(dir.join("settings.invalid.json").exists());
        file.save(&settings).unwrap();
        let again = load(&dir);
        assert_eq!((again.settings, again.warning), (Settings::default(), None));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_save_goes_ahead_when_the_disk_refuses_to_sync() {
        let dir = std::env::temp_dir().join(format!("tinydash-unsynced-{}", std::process::id()));
        let changed = Settings {
            hide_on_blur: false,
            ..Settings::default()
        };
        let saved = SettingsFile::new(dir.clone()).save_with(&changed, |_| {
            Err(std::io::Error::other("full sync refused"))
        });
        let again = load(&dir);
        let leftover = dir.join(format!("{FILE_NAME}.tmp")).exists();
        std::fs::remove_dir_all(&dir).unwrap();
        saved.unwrap();
        assert_eq!((again.settings, again.warning), (changed, None));
        assert!(!leftover);
    }

    #[test]
    fn a_file_that_is_not_utf8_is_moved_aside() {
        let dir = std::env::temp_dir().join(format!("tinydash-latin1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // "São Paulo" saved as Latin-1.
        let bytes = b"{\"clockCities\": [\"S\xe3o Paulo\"]}";
        std::fs::write(dir.join(FILE_NAME), bytes).unwrap();
        let Loaded {
            settings, warning, ..
        } = load(&dir);
        let backup = std::fs::read(dir.join("settings.invalid.json"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains("settings.invalid.json"));
        assert_eq!(backup.unwrap(), bytes);
    }

    #[test]
    fn a_damaged_file_that_could_not_be_moved_is_never_replaced() {
        let dir = std::env::temp_dir().join(format!("tinydash-unmoved-{}", std::process::id()));
        // A folder that is not empty in the backup's place blocks the rename on every OS.
        std::fs::create_dir_all(dir.join("settings.invalid.json").join("taken")).unwrap();
        let bytes = b"{\"clockCities\": [\"S\xe3o Paulo\"]}";
        std::fs::write(dir.join(FILE_NAME), bytes).unwrap();
        let Loaded {
            settings,
            warning,
            file,
        } = load(&dir);
        let saved = file.save(&settings);
        let kept = std::fs::read(dir.join(FILE_NAME));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains("will not be saved"));
        assert!(saved.is_err());
        assert_eq!(kept.unwrap(), bytes);
    }

    #[test]
    fn a_failed_save_says_what_to_do_only_when_the_cause_is_known() {
        use std::io::ErrorKind;
        let error = |kind: ErrorKind| save_failed(Path::new("/settings"), &kind.into()).to_string();
        assert_eq!(
            error(ErrorKind::StorageFull),
            format!(
                "Could not save settings in /settings: {}. Free some disk space, then try again.",
                std::io::Error::from(ErrorKind::StorageFull)
            )
        );
        let open = if cfg!(windows) {
            " and that no other app has settings.json open"
        } else {
            ""
        };
        let denied = error(ErrorKind::PermissionDenied);
        assert!(
            denied.ends_with(&format!(". Check that you can write to /settings{open}.")),
            "{denied}"
        );
        let read_only = error(ErrorKind::ReadOnlyFilesystem);
        assert!(
            read_only.ends_with(". Check that you can write to /settings."),
            "{read_only}"
        );
        assert_eq!(
            error(ErrorKind::TimedOut),
            format!(
                "Could not save settings in /settings: {}.",
                std::io::Error::from(ErrorKind::TimedOut)
            )
        );

        // A file in the folder's place stops the save on every OS.
        let blocked = std::env::temp_dir().join(format!("tinydash-blocked-{}", std::process::id()));
        std::fs::write(&blocked, "").unwrap();
        let saved = SettingsFile::new(blocked.join("settings")).save(&Settings::default());
        std::fs::remove_file(&blocked).unwrap();
        let saved = saved.unwrap_err().to_string();
        assert!(saved.starts_with("Could not save settings in "), "{saved}");
    }

    #[test]
    fn a_file_that_could_not_be_read_is_never_replaced() {
        let dir = std::env::temp_dir().join(format!("tinydash-unread-{}", std::process::id()));
        // A folder in its place fails to read, as a locked file would.
        std::fs::create_dir_all(dir.join(FILE_NAME)).unwrap();
        let Loaded {
            settings,
            warning,
            file,
        } = load(&dir);
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains("will not be saved"));
        let error = file.save(&settings).unwrap_err().to_string();
        assert!(error.contains("did not replace"), "{error}");

        std::fs::remove_dir(dir.join(FILE_NAME)).unwrap();
        std::fs::write(dir.join(FILE_NAME), r#"{"hideOnBlur": false}"#).unwrap();
        let blocked = SettingsFile {
            dir: dir.clone(),
            replaceable: false,
        };
        assert!(blocked.save(&Settings::default()).is_err());
        let kept = std::fs::read_to_string(dir.join(FILE_NAME));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(kept.unwrap(), r#"{"hideOnBlur": false}"#);
    }
}
