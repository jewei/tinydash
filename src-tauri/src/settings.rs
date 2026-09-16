use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DEFAULT_SHORTCUT: &str = "Control+Shift+Space";

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        }
    }
}

impl Settings {
    pub fn clipboard_limit(&self) -> usize {
        usize::from(self.clipboard_history_limit.clamp(1, 500))
    }

    pub fn file_limit(&self) -> usize {
        self.file_search_limit.clamp(1, 100_000) as usize
    }
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
