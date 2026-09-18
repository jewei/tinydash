use std::path::{Path, PathBuf};

use crate::launcher::query::SearchMode;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SHORTCUT: &str = "Control+Shift+Space";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub clear_query_on_open: bool,
    pub hide_on_blur: bool,
    pub shortcut: String,
    pub clipboard_history_enabled: bool,
    pub clipboard_history_limit: u16,
    pub file_search_roots: Option<Vec<PathBuf>>,
    pub file_search_limit: u32,
    pub file_search_excluded_dirs: Vec<String>,
    pub file_watch_enabled: bool,
    pub currency_rates_enabled: bool,
    pub visible_categories: Vec<SearchMode>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clear_query_on_open: true,
            hide_on_blur: true,
            shortcut: DEFAULT_SHORTCUT.into(),
            clipboard_history_enabled: true,
            clipboard_history_limit: 100,
            file_search_roots: None,
            file_search_limit: 50_000,
            file_search_excluded_dirs: vec!["node_modules".into(), "target".into()],
            file_watch_enabled: true,
            currency_rates_enabled: true,
            visible_categories: vec![
                SearchMode::All,
                SearchMode::Apps,
                SearchMode::Files,
                SearchMode::Clipboard,
                SearchMode::Emoji,
                SearchMode::Calculator,
                SearchMode::System,
                SearchMode::Password,
                SearchMode::Timezone,
                SearchMode::Url,
                SearchMode::Web,
            ],
        }
    }
}

impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::ensure;
        use tauri_plugin_global_shortcut::{Modifiers, Shortcut};
        ensure!(
            !self.visible_categories.is_empty(),
            "Select at least one category."
        );
        ensure!(
            self.visible_categories
                .iter()
                .enumerate()
                .all(|(index, category)| !self.visible_categories[..index].contains(category)),
            "Select each category only once."
        );
        let shortcut: Shortcut = self
            .shortcut
            .parse()
            .map_err(|_| anyhow::anyhow!("Use a modifier and one key for the launch shortcut."))?;
        ensure!(
            shortcut
                .mods
                .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER),
            "Include Control, Option / Alt, or Command / Windows in the shortcut."
        );
        ensure!(
            (1..=500).contains(&self.clipboard_history_limit),
            "Clipboard history must contain 1 to 500 entries."
        );
        ensure!(
            (1..=100_000).contains(&self.file_search_limit),
            "The file limit must be 1 to 100,000."
        );
        if let Some(roots) = &self.file_search_roots {
            ensure!(roots.len() <= 64, "Use no more than 64 search folders.");
            for root in roots {
                let text = root.to_string_lossy();
                ensure!(
                    text.len() <= 4096
                        && !text.contains('\0')
                        && (root.is_absolute()
                            || text == "~"
                            || text.starts_with("~/")
                            || text.starts_with("~\\")),
                    "Use an absolute folder path or ~/folder: {}",
                    root.display()
                );
            }
        }
        ensure!(
            self.file_search_excluded_dirs.len() <= 128,
            "Use no more than 128 excluded folder names."
        );
        for name in &self.file_search_excluded_dirs {
            ensure!(
                !name.trim().is_empty()
                    && name.len() <= 255
                    && !name.contains(['/', '\\', '\0'])
                    && name != "."
                    && name != "..",
                "Excluded folders must be folder names, not paths or patterns."
            );
        }
        Ok(())
    }

    pub fn same_file_settings(&self, other: &Self) -> bool {
        self.file_search_roots == other.file_search_roots
            && self.file_search_limit == other.file_search_limit
            && self.file_search_excluded_dirs == other.file_search_excluded_dirs
            && self.file_watch_enabled == other.file_watch_enabled
    }

    pub fn clipboard_limit(&self) -> usize {
        usize::from(self.clipboard_history_limit.clamp(1, 500))
    }

    pub fn file_limit(&self) -> usize {
        self.file_search_limit.clamp(1, 100_000) as usize
    }
}

pub fn save(directory: &Path, settings: &Settings) -> anyhow::Result<()> {
    use anyhow::{Context, ensure};
    use std::io::Write;
    settings.validate()?;
    std::fs::create_dir_all(directory).context("Create the settings directory")?;
    let path = directory.join("settings.json");
    // Preserve settings from newer versions and never overwrite a damaged file.
    let mut document = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes)
            .context("The settings file contains invalid JSON. Repair it before saving.")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => return Err(error).context("Read the settings file before saving"),
    };
    ensure!(
        document.is_object(),
        "The settings file must contain a JSON object."
    );
    let object = document.as_object_mut().expect("checked object");
    if let serde_json::Value::Object(values) = serde_json::to_value(settings)? {
        object.extend(values);
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).context("Create the settings file")?;
    serde_json::to_writer_pretty(&mut temporary, &document).context("Write settings")?;
    temporary.write_all(b"\n")?;
    temporary
        .as_file()
        .sync_all()
        .context("Save settings to disk")?;
    temporary
        .persist(&path)
        .context("Replace the settings file")?;
    Ok(())
}

pub fn load(directory: &Path) -> anyhow::Result<Settings> {
    use anyhow::Context;
    let path = directory.join("settings.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mut settings: Settings = serde_json::from_slice(&bytes)
                .with_context(|| format!("Read settings from {}", path.display()))?;
            // macOS 27 uses Command+Shift+Space for Siri Visual Intelligence.
            // Upgrade only our old default. Keep custom shortcuts and the file intact.
            if cfg!(target_os = "macos") && settings.shortcut == "CommandOrControl+Shift+Space" {
                settings.shortcut = DEFAULT_SHORTCUT.into();
                tracing::info!("Replaced the old macOS shortcut with Control+Shift+Space");
            }
            Ok(settings)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let settings = Settings::default();
            std::fs::create_dir_all(directory).context("Create the settings directory")?;
            std::fs::write(&path, serde_json::to_vec_pretty(&settings)?)
                .context("Write default settings")?;
            Ok(settings)
        }
        Err(error) => Err(error).context("Read launcher settings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_show_every_category_without_changing_other_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = r#"{"shortcut":"Alt+KeyJ","hideOnBlur":false}"#;
        std::fs::write(&path, original).unwrap();
        let settings = load(directory.path()).unwrap();
        assert_eq!(
            settings.visible_categories,
            Settings::default().visible_categories
        );
        assert_eq!(settings.visible_categories.len(), 11);
        assert_eq!(settings.shortcut, "Alt+KeyJ");
        assert!(!settings.hide_on_blur);
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn invalid_categories_cannot_replace_saved_settings() {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        save(directory.path(), &settings).unwrap();
        let path = directory.path().join("settings.json");
        let original = std::fs::read(&path).unwrap();
        for categories in [vec![], vec![SearchMode::Apps, SearchMode::Apps]] {
            settings.visible_categories = categories;
            assert!(save(directory.path(), &settings).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        assert!(serde_json::from_str::<Settings>(r#"{"visibleCategories":["unknown"]}"#).is_err());
    }

    #[test]
    fn saves_all_preferences_and_preserves_unknown_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"futureSetting":{"enabled":true}}"#,
        )
        .unwrap();
        let settings = Settings {
            shortcut: "Alt+Shift+KeyJ".into(),
            visible_categories: vec![SearchMode::Apps, SearchMode::Calculator],
            clipboard_history_enabled: false,
            clipboard_history_limit: 42,
            file_search_roots: Some(vec!["~/Projects".into()]),
            file_watch_enabled: false,
            currency_rates_enabled: false,
            ..Settings::default()
        };
        save(dir.path(), &settings).expect("save");
        assert_eq!(load(dir.path()).unwrap(), settings);
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(document["futureSetting"]["enabled"], true);
    }

    #[test]
    fn save_preserves_invalid_files_and_rejects_invalid_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for original in ["bad json", "[]"] {
            std::fs::write(&path, original).unwrap();
            assert!(save(dir.path(), &Settings::default()).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
        std::fs::write(&path, "{}").unwrap();
        let mut settings = Settings {
            clipboard_history_limit: 0,
            ..Settings::default()
        };
        assert!(save(dir.path(), &settings).is_err());
        settings.clipboard_history_limit = 100;
        for shortcut in ["KeyA", "Shift+KeyA", "Control", "Unknown+Space"] {
            settings.shortcut = shortcut.into();
            assert!(save(dir.path(), &settings).is_err());
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), "{}");
    }

    #[test]
    fn validates_search_paths_and_folder_names() {
        let mut settings = Settings::default();
        for path in ["relative/path", "", "~/bad\0path"] {
            settings.file_search_roots = Some(vec![path.into()]);
            assert!(settings.validate().is_err());
        }
        settings.file_search_roots = Some(vec!["~/Projects".into()]);
        assert!(settings.validate().is_ok());
        settings.file_search_excluded_dirs = vec!["a/b".into()];
        assert!(settings.validate().is_err());
    }

    #[test]
    fn creates_defaults_and_preserves_user_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let defaults = load(dir.path()).expect("defaults");
        assert!(defaults.clear_query_on_open);
        assert_eq!(defaults.shortcut, "Control+Shift+Space");
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"clearQueryOnOpen":false}"#,
        )
        .expect("write");
        let settings = load(dir.path()).expect("settings");
        assert!(!settings.clear_query_on_open);
        assert!(settings.hide_on_blur);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn legacy_shortcut_avoids_siri_and_preserves_saved_settings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        let saved = r#"{"shortcut":"CommandOrControl+Shift+Space","clearQueryOnOpen":false,"clipboardHistoryEnabled":false,"fileSearchRoots":[],"customSetting":true}"#;
        std::fs::write(&path, saved).expect("write settings");

        let settings = load(dir.path()).expect("load legacy settings");
        assert_eq!(settings.shortcut, "Control+Shift+Space");
        assert!(!settings.clear_query_on_open);
        assert!(!settings.clipboard_history_enabled);
        assert_eq!(settings.file_search_roots, Some(vec![]));
        assert_eq!(std::fs::read_to_string(path).expect("read settings"), saved);
    }

    #[test]
    fn preserves_custom_shortcuts() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"shortcut":"Alt+Shift+Space"}"#,
        )
        .expect("write settings");
        assert_eq!(
            load(dir.path()).expect("custom shortcut").shortcut,
            "Alt+Shift+Space"
        );
    }

    #[test]
    fn clipboard_settings_have_bounded_defaults_and_can_disable_capture() {
        let settings = Settings::default();
        assert!(settings.clipboard_history_enabled);
        assert_eq!(settings.clipboard_limit(), 100);
        let settings: Settings =
            serde_json::from_str(r#"{"clipboardHistoryEnabled":false,"clipboardHistoryLimit":0}"#)
                .expect("settings");
        assert!(!settings.clipboard_history_enabled);
        assert_eq!(settings.clipboard_limit(), 1);
        let settings = Settings {
            clipboard_history_limit: 1000,
            ..settings
        };
        assert_eq!(settings.clipboard_limit(), 500);
    }

    #[test]
    fn does_not_overwrite_invalid_settings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "bad json").expect("write");
        assert!(load(dir.path()).is_err());
        assert_eq!(std::fs::read_to_string(path).expect("read"), "bad json");
    }

    #[test]
    fn file_defaults_allow_explicit_roots_disable_and_bounded_limits() {
        let defaults: Settings = serde_json::from_str("{}").expect("settings");
        assert!(defaults.file_search_roots.is_none());
        assert!(defaults.file_watch_enabled);
        assert!(defaults.currency_rates_enabled);
        let offline: Settings =
            serde_json::from_str(r#"{"fileWatchEnabled":false,"currencyRatesEnabled":false}"#)
                .expect("offline settings");
        assert!(!offline.file_watch_enabled);
        assert!(!offline.currency_rates_enabled);
        assert_eq!(defaults.file_limit(), 50_000);
        let disabled: Settings =
            serde_json::from_str(r#"{"fileSearchRoots":[],"fileSearchLimit":0}"#)
                .expect("settings");
        assert_eq!(disabled.file_search_roots, Some(vec![]));
        assert_eq!(disabled.file_limit(), 1);
        let custom: Settings = serde_json::from_str(r#"{"fileSearchRoots":["~/Documents"],"fileSearchLimit":999999,"fileSearchExcludedDirs":[]}"#).expect("settings");
        assert_eq!(
            custom.file_search_roots,
            Some(vec![PathBuf::from("~/Documents")])
        );
        assert_eq!(custom.file_limit(), 100_000);
        assert!(custom.file_search_excluded_dirs.is_empty());
    }
}
