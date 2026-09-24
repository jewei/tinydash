use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::LauncherState;
use crate::{providers::clipboard::entry_id, settings::Settings};

const FORMAT_VERSION: u64 = 1;
const MAX_PORTABILITY_BYTES: u64 = 512 * 1024;
const MAX_APPEARANCE_BYTES: usize = 32;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsImport {
    pub settings: Settings,
    pub ignored_keys: Vec<String>,
    pub appearance: Option<String>,
    pub compact: Option<bool>,
    pub follow_system_glass: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsExport<'a> {
    format_version: u64,
    settings: &'a Settings,
    appearance: &'a str,
    compact: bool,
    follow_system_glass: bool,
}

#[tauri::command]
pub async fn export_settings(
    app: AppHandle,
    appearance: String,
    compact: bool,
    follow_system_glass: bool,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_settings_blocking(&app, &appearance, compact, follow_system_glass)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn export_settings_blocking(
    app: &AppHandle,
    appearance: &str,
    compact: bool,
    follow_system_glass: bool,
) -> Result<bool, String> {
    validate_appearance(appearance)?;
    let settings = app.state::<LauncherState>().settings();
    settings.validate().map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(&SettingsExport {
        format_version: FORMAT_VERSION,
        settings: &settings,
        appearance,
        compact,
        follow_system_glass,
    })
    .map_err(|error| format!("Could not encode settings: {error}"))?;
    ensure_size(bytes.len() as u64)?;
    let Some(path) = choose_save_path(
        app,
        "Export TinyDash settings",
        "tinydash-settings.json",
        "JSON",
        &["json"],
    )?
    else {
        return Ok(false);
    };
    ensure_output_path_safe(app, &path)?;
    write_private_atomic(&path, &bytes)?;
    Ok(true)
}

#[tauri::command]
pub async fn preview_settings_import(app: AppHandle) -> Result<Option<SettingsImport>, String> {
    tauri::async_runtime::spawn_blocking(move || preview_settings_import_blocking(&app))
        .await
        .map_err(|error| error.to_string())?
}

fn preview_settings_import_blocking(app: &AppHandle) -> Result<Option<SettingsImport>, String> {
    let Some(path) = choose_open_path(app, "Import TinyDash settings", "JSON", &["json"])? else {
        return Ok(None);
    };
    let bytes = read_bounded(&path)?;
    parse_settings_import(&bytes).map(Some)
}

fn parse_settings_import(bytes: &[u8]) -> Result<SettingsImport, String> {
    ensure_size(bytes.len() as u64)?;
    let mut document: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("The settings export is not valid JSON: {error}"))?;
    let object = document
        .as_object_mut()
        .ok_or("The settings export must contain a JSON object.")?;
    let mut ignored_keys = object
        .keys()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "formatVersion" | "settings" | "appearance" | "compact" | "followSystemGlass"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let version = object
        .get("formatVersion")
        .and_then(Value::as_u64)
        .ok_or("The settings export has an invalid formatVersion.")?;
    if version != FORMAT_VERSION {
        return Err(format!(
            "This settings export uses unsupported format version {version}."
        ));
    }
    let settings = object
        .get_mut("settings")
        .ok_or("The settings export does not contain settings.")?
        .as_object_mut()
        .ok_or("The settings export settings value must be an object.")?;
    let known_settings = serde_json::to_value(Settings::default())
        .map_err(|error| format!("Could not inspect settings fields: {error}"))?;
    let known_settings = known_settings
        .as_object()
        .ok_or("Could not inspect settings fields.")?;
    let unknown_settings = settings
        .keys()
        .filter(|key| !known_settings.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    for key in &unknown_settings {
        settings.remove(key);
        ignored_keys.push(format!("settings.{key}"));
    }
    let settings = serde_json::from_value::<Settings>(Value::Object(settings.clone()))
        .map_err(|error| format!("The imported settings are invalid: {error}"))?;
    settings
        .validate()
        .map_err(|error| format!("The imported settings are invalid: {error}"))?;
    let appearance = match object.get("appearance") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => {
            validate_appearance(value)?;
            Some(value.clone())
        }
        Some(_) => return Err("The imported appearance must be a string or null.".into()),
    };
    let compact = match object.get("compact") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => return Err("The imported compact layout must be a boolean or null.".into()),
    };
    let follow_system_glass = match object.get("followSystemGlass") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => {
            return Err("The imported Liquid Glass preference must be a boolean or null.".into());
        }
    };
    ignored_keys.sort();
    Ok(SettingsImport {
        settings,
        ignored_keys,
        appearance,
        compact,
        follow_system_glass,
    })
}

#[tauri::command]
pub async fn save_clipboard_file(app: AppHandle, id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || save_clipboard_file_blocking(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

fn save_clipboard_file_blocking(app: &AppHandle, id: &str) -> Result<bool, String> {
    let numeric_id = entry_id(id).ok_or("The clipboard entry is no longer available.")?;
    let content = app
        .state::<LauncherState>()
        .search
        .lock()
        .map_err(|_| "Search results are unavailable.")?
        .clipboard
        .get(&format!("clipboard:{numeric_id}"))
        .ok_or("The clipboard entry is no longer available.")?
        .content
        .clone();
    let Some(path) = choose_save_path(
        app,
        "Save clipboard text",
        "clipboard.txt",
        "Text",
        &["txt"],
    )?
    else {
        return Ok(false);
    };
    ensure_output_path_safe(app, &path)?;
    write_private_atomic(&path, content.as_bytes())?;
    Ok(true)
}

#[tauri::command]
pub async fn reveal_backup(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("recovery.tar");
        if !path.is_file() {
            return Err(
                "No recovery archive exists yet. TinyDash creates one before a database migration."
                    .into(),
            );
        }
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn choose_save_path(
    app: &AppHandle,
    title: &str,
    file_name: &str,
    filter_name: &str,
    extensions: &[&str],
) -> Result<Option<PathBuf>, String> {
    app.dialog()
        .file()
        .set_title(title)
        .set_file_name(file_name)
        .add_filter(filter_name, extensions)
        .blocking_save_file()
        .map(|path| path.into_path().map_err(|error| error.to_string()))
        .transpose()
}

fn choose_open_path(
    app: &AppHandle,
    title: &str,
    filter_name: &str,
    extensions: &[&str],
) -> Result<Option<PathBuf>, String> {
    app.dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, extensions)
        .blocking_pick_file()
        .map(|path| path.into_path().map_err(|error| error.to_string()))
        .transpose()
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|error| format!("Could not inspect the import file: {error}"))?
        .len();
    ensure_size(size)?;
    let mut bytes = Vec::with_capacity(size as usize);
    File::open(path)
        .map_err(|error| format!("Could not open the import file: {error}"))?
        .take(MAX_PORTABILITY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read the import file: {error}"))?;
    ensure_size(bytes.len() as u64)?;
    Ok(bytes)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|error| format!("Could not create the output file: {error}"))?;
    temporary
        .write_all(bytes)
        .map_err(|error| format!("Could not write the output file: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Could not save the output file: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Could not replace the output file: {}", error.error))?;
    Ok(())
}

fn ensure_output_path_safe(app: &AppHandle, path: &Path) -> Result<(), String> {
    let config = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let protected = [
        config.join("settings.json"),
        data.join("tinydash.sqlite3"),
        data.join("tinydash.sqlite3-wal"),
        data.join("tinydash.sqlite3-shm"),
        data.join("tinydash.sqlite3-journal"),
        data.join("recovery.tar"),
    ];
    if protected
        .iter()
        .any(|candidate| paths_equal(path, candidate))
    {
        return Err(
            "Choose a separate output file. TinyDash will not overwrite its live settings or data."
                .into(),
        );
    }
    Ok(())
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| {
        if let Ok(path) = std::fs::canonicalize(path) {
            return path;
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let parent = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_owned());
        parent.join(path.file_name().unwrap_or_default())
    };
    let left = normalize(left);
    let right = normalize(right);
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn ensure_size(size: u64) -> Result<(), String> {
    if size > MAX_PORTABILITY_BYTES {
        Err(format!(
            "The settings file is larger than {} KiB.",
            MAX_PORTABILITY_BYTES / 1024
        ))
    } else {
        Ok(())
    }
}

fn validate_appearance(value: &str) -> Result<(), String> {
    if value.len() > MAX_APPEARANCE_BYTES
        || !matches!(
            value,
            "light" | "dark" | "sage" | "rose" | "ink" | "compact"
        )
    {
        return Err(
            "Theme must be light, dark, sage, rose, or ink. Legacy compact is also accepted."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(settings: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "formatVersion": 1,
            "settings": settings,
            "appearance": "dark",
        }))
        .unwrap()
    }

    #[test]
    fn import_round_trip_and_report_unknown_keys() {
        let settings = serde_json::to_value(Settings::default()).unwrap();
        let mut value: Value = serde_json::from_slice(&export(settings)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("futureTopLevel".into(), Value::Bool(true));
        value
            .get_mut("settings")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("futureSetting".into(), Value::String("kept".into()));
        let imported = parse_settings_import(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(imported.appearance.as_deref(), Some("dark"));
        assert_eq!(
            imported.ignored_keys,
            ["futureTopLevel", "settings.futureSetting"]
        );
    }

    #[test]
    fn themes_and_compact_layout_round_trip_independently() {
        for appearance in ["light", "dark", "sage", "rose", "ink"] {
            for compact in [false, true] {
                for follow_system_glass in [false, true] {
                    let settings = Settings::default();
                    let bytes = serde_json::to_vec(&SettingsExport {
                        format_version: FORMAT_VERSION,
                        settings: &settings,
                        appearance,
                        compact,
                        follow_system_glass,
                    })
                    .unwrap();
                    let imported = parse_settings_import(&bytes).unwrap();
                    assert_eq!(imported.appearance.as_deref(), Some(appearance));
                    assert_eq!(imported.compact, Some(compact));
                    assert_eq!(imported.follow_system_glass, Some(follow_system_glass));
                    assert!(imported.ignored_keys.is_empty());
                }
            }
        }
        let mut legacy: Value =
            serde_json::from_slice(&export(serde_json::to_value(Settings::default()).unwrap()))
                .unwrap();
        legacy["appearance"] = Value::from("compact");
        let imported = parse_settings_import(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(imported.appearance.as_deref(), Some("compact"));
        assert_eq!(imported.compact, None);
        assert_eq!(imported.follow_system_glass, None);
        legacy["followSystemGlass"] = Value::from("false");
        assert!(parse_settings_import(&serde_json::to_vec(&legacy).unwrap()).is_err());
        legacy["followSystemGlass"] = Value::Null;
        legacy["compact"] = Value::from("true");
        assert!(parse_settings_import(&serde_json::to_vec(&legacy).unwrap()).is_err());
    }

    #[test]
    fn import_rejects_bad_version_and_invalid_file_roots() {
        let mut value: Value =
            serde_json::from_slice(&export(serde_json::to_value(Settings::default()).unwrap()))
                .unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("formatVersion".into(), Value::from(2));
        assert!(parse_settings_import(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value: Value =
            serde_json::from_slice(&export(serde_json::to_value(Settings::default()).unwrap()))
                .unwrap();
        value
            .get_mut("settings")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("fileSearchRoots".into(), serde_json::json!(["relative"]));
        assert!(parse_settings_import(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn import_rejects_oversized_and_wrong_types() {
        assert!(parse_settings_import(&vec![b'x'; MAX_PORTABILITY_BYTES as usize + 1]).is_err());

        let mut value: Value =
            serde_json::from_slice(&export(serde_json::to_value(Settings::default()).unwrap()))
                .unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("appearance".into(), Value::Null);
        assert!(parse_settings_import(&serde_json::to_vec(&value).unwrap()).is_ok());
        value
            .as_object_mut()
            .unwrap()
            .insert("appearance".into(), Value::from(3));
        assert!(parse_settings_import(&serde_json::to_vec(&value).unwrap()).is_err());
        value
            .as_object_mut()
            .unwrap()
            .insert("settings".into(), Value::Null);
        assert!(parse_settings_import(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn atomic_write_preserves_exact_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard.txt");
        let bytes = b"  Cafe\xCC\x81 \xF0\x9F\x9A\x80\n";
        write_private_atomic(&path, bytes).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(directory.path().join("clipboard.txt"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn atomic_write_keeps_existing_directory_when_target_is_not_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output");
        std::fs::create_dir(&target).unwrap();
        assert!(write_private_atomic(&target, b"text").is_err());
        assert!(target.is_dir());
    }
}
