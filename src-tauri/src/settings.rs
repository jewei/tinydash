use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    error::Result,
    features::{emoji::EmojiLanguage, web::SearchEngine},
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
    pub search_engine: SearchEngine,
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
            search_engine: SearchEngine::Google,
        }
    }
}

impl Settings {
    /// Clamp numbers, drop blank or duplicate list entries, and cut lists
    /// to their limit.
    pub fn normalized(mut self) -> Self {
        self.shortcut = self.shortcut.trim().to_owned();
        self.clipboard_history_limit = self.clipboard_history_limit.clamp(1, CLIPBOARD_LIMIT_MAX);
        self.emoji_skin_tone = self.emoji_skin_tone.min(5);
        self.emoji_languages.sort();
        self.emoji_languages.dedup();
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
        self
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
    match serde_json::from_str::<Settings>(&text) {
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
    fn missing_fields_take_defaults_and_unknown_fields_are_ignored() {
        let settings: Settings =
            serde_json::from_str(r#"{"hideOnBlur": false, "appearance": "sage"}"#).unwrap();
        assert!(!settings.hide_on_blur);
        assert_eq!(settings.shortcut, Settings::default().shortcut);
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

        let many = Settings {
            file_search_excluded_dirs: (0..99).map(|n| format!("dir{n}")).collect(),
            ..Settings::default()
        }
        .normalized();
        assert_eq!(many.file_search_excluded_dirs.len(), FOLDER_LIST_MAX);
        assert_eq!(many.file_search_excluded_dirs[0], "dir0");
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
