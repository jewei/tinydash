use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub clear_query_on_open: bool,
    pub hide_on_blur: bool,
    pub shortcut: String,
    pub clipboard_history_enabled: bool,
    pub clipboard_history_limit: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clear_query_on_open: true,
            hide_on_blur: true,
            shortcut: "CommandOrControl+Shift+Space".into(),
            clipboard_history_enabled: true,
            clipboard_history_limit: 100,
        }
    }
}

impl Settings {
    pub fn clipboard_limit(&self) -> usize {
        usize::from(self.clipboard_history_limit.clamp(1, 500))
    }
}

pub fn load(directory: &Path) -> anyhow::Result<Settings> {
    use anyhow::Context;
    let path = directory.join("settings.json");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("Read settings from {}", path.display())),
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
        assert!(load(dir.path()).expect("defaults").clear_query_on_open);
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"clearQueryOnOpen":false}"#,
        )
        .expect("write");
        let settings = load(dir.path()).expect("settings");
        assert!(!settings.clear_query_on_open);
        assert!(settings.hide_on_blur);
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
}
