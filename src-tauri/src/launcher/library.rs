//! Native, explicit-invocation quicklinks and snippets. No background listeners.
//! Integration: manage `LibraryState::open(app.path().app_data_dir()?)?`, register
//! the five `library_*` commands, and expose `search_items` in root search.
//! Root results should route to the panel by ID (arguments stay explicit), or call
//! `library_execute` with an ID and action. No frontend-supplied launch targets.

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

const MAX_ITEMS: usize = 256;
const MAX_TEXT: usize = 32_768;
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_RENDERED: usize = 65_536;
const MAX_ARGUMENTS: usize = 16;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum LibraryKind {
    Quicklink,
    Snippet,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryDraft {
    pub kind: LibraryKind,
    pub name: String,
    pub keywords: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryEntry {
    pub id: String,
    #[serde(flatten)]
    pub draft: LibraryDraft,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct LibraryItem {
    pub id: String,
    pub kind: LibraryKind,
    pub name: String,
    pub keywords: String,
    pub arguments: Vec<String>,
    pub uses_clipboard: bool,
}

impl LibraryEntry {
    pub fn inserts_directly(&self) -> bool {
        self.draft.kind == LibraryKind::Snippet && !self.draft.content.contains("{clipboard}")
    }

    fn item(&self) -> LibraryItem {
        LibraryItem {
            id: self.id.clone(),
            kind: self.draft.kind,
            name: self.draft.name.clone(),
            keywords: self.draft.keywords.clone(),
            arguments: arguments(&self.draft).unwrap_or_default(),
            uses_clipboard: self.draft.kind == LibraryKind::Snippet
                && self.draft.content.contains("{clipboard}"),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    entries: Vec<LibraryEntry>,
}

struct Store {
    path: PathBuf,
    document: Document,
}

pub struct LibraryState(Mutex<Result<Store>>);

impl LibraryState {
    /// Corruption is an error, never an excuse to overwrite the user's library.
    pub fn open(data_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&data_dir).map_err(|_| "Could not create library directory")?;
        let path = data_dir.join("library.json");
        let document = match File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_FILE + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "Could not read library")?;
                if bytes.len() as u64 > MAX_FILE {
                    return Err("Library exceeds its storage limit".into());
                }
                let document: Document = serde_json::from_slice(&bytes)
                    .map_err(|_| "Library is invalid; the original file was preserved")?;
                validate_document(&document)?;
                document
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Document {
                version: 1,
                entries: Vec::new(),
            },
            Err(_) => return Err("Could not read library".into()),
        };
        Ok(Self(Mutex::new(Ok(Store { path, document }))))
    }

    /// Keep a managed state after an initialization failure without creating a
    /// writable fallback or overwriting damaged storage. All operations fail
    /// with the retained warning until a later successful application startup.
    pub fn unavailable(message: String) -> Self {
        Self(Mutex::new(Err(message)))
    }

    /// In-memory metadata only: no filesystem, clipboard, or launch work. Call
    /// outside the SearchManager lock. Do not index snippet content or secrets.
    pub fn search_items(&self, query: &str) -> Result<Vec<LibraryItem>> {
        if query.len() > 512 {
            return Err("Search is too long".into());
        }
        let query = query.to_lowercase();
        let store = self.0.lock().map_err(|_| "Library is unavailable")?;
        let store = store.as_ref().map_err(Clone::clone)?;
        Ok(store
            .document
            .entries
            .iter()
            .filter(|entry| {
                format!("{} {}", entry.draft.name, entry.draft.keywords)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(LibraryEntry::item)
            .collect())
    }

    pub fn get(&self, id: &str) -> Result<LibraryEntry> {
        let store = self.0.lock().map_err(|_| "Library is unavailable")?;
        let store = store.as_ref().map_err(Clone::clone)?;
        store
            .document
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
            .ok_or_else(|| "Library item no longer exists".into())
    }

    pub fn save(&self, id: Option<&str>, draft: LibraryDraft) -> Result<LibraryEntry> {
        let mut store = self.0.lock().map_err(|_| "Library is unavailable")?;
        let store = store.as_mut().map_err(|message| message.clone())?;
        validate_draft(&draft)?;
        let mut next = store.document.clone();
        let entry = if let Some(id) = id {
            let entry = next
                .entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .ok_or("Library item no longer exists")?;
            entry.draft = draft;
            entry.clone()
        } else {
            if next.entries.len() >= MAX_ITEMS {
                return Err("Library is full (256 items)".into());
            }
            let mut random = [0_u8; 16];
            getrandom::fill(&mut random).map_err(|_| "Could not create item ID")?;
            let id = format!(
                "library:{}",
                random
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            let entry = LibraryEntry { id, draft };
            next.entries.push(entry.clone());
            entry
        };
        store.commit(next)?;
        Ok(entry)
    }

    pub fn delete(&self, id: &str, confirmed: bool) -> Result<()> {
        if !confirmed {
            return Err("Confirm deletion first".into());
        }
        let mut store = self.0.lock().map_err(|_| "Library is unavailable")?;
        let store = store.as_mut().map_err(|message| message.clone())?;
        let mut next = store.document.clone();
        let before = next.entries.len();
        next.entries.retain(|entry| entry.id != id);
        if before == next.entries.len() {
            return Err("Library item no longer exists".into());
        }
        store.commit(next)
    }
}

impl Store {
    fn commit(&mut self, next: Document) -> Result<()> {
        validate_document(&next)?;
        let bytes = serde_json::to_vec(&next).map_err(|_| "Could not encode library")?;
        if bytes.len() as u64 > MAX_FILE {
            return Err("Library exceeds its 2 MiB storage limit".into());
        }
        let directory = self.path.parent().ok_or("Invalid library location")?;
        // Same-volume atomic replacement. NamedTempFile uses owner-only mode on
        // Unix. Failed writes leave both the previous file and live state intact.
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|_| "Could not save library")?;
        temporary
            .write_all(&bytes)
            .map_err(|_| "Could not save library")?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "Could not save library")?;
        temporary
            .persist(&self.path)
            .map_err(|_| "Could not replace library")?;
        self.document = next;
        // A directory sync is best-effort: replacement has already committed.
        #[cfg(unix)]
        if let Ok(directory) = File::open(directory) {
            let _ = directory.sync_all();
        }
        Ok(())
    }
}

fn validate_document(document: &Document) -> Result<()> {
    if document.version != 1 || document.entries.len() > MAX_ITEMS {
        return Err("Unsupported or oversized library".into());
    }
    let mut ids = HashSet::new();
    for entry in &document.entries {
        if entry.id.len() != 40
            || !entry.id.starts_with("library:")
            || !entry.id[8..].bytes().all(|byte| byte.is_ascii_hexdigit())
            || !ids.insert(&entry.id)
        {
            return Err("Invalid library item ID".into());
        }
        validate_draft(&entry.draft)?;
    }
    Ok(())
}

fn validate_draft(draft: &LibraryDraft) -> Result<()> {
    if draft.name.trim().is_empty()
        || draft.name.len() > 120
        || draft.name.chars().any(char::is_control)
        || draft.keywords.len() > 512
        || draft.keywords.chars().any(char::is_control)
        || draft.content.is_empty()
        || draft.content.len() > MAX_TEXT
        || draft.content.contains('\0')
    {
        return Err("Use a name (120 bytes), keywords (512 bytes), and content (32 KiB)".into());
    }
    match draft.kind {
        LibraryKind::Quicklink => {
            let names = arguments(draft)?;
            let values = names
                .into_iter()
                .map(|name| (name, "example".into()))
                .collect();
            let rendered = render_quicklink(draft, &values)?;
            validate_target(&rendered)?;
        }
        LibraryKind::Snippet if draft.content.contains("{selection}") => {
            return Err("Selection capture is not supported; use {clipboard} explicitly".into());
        }
        LibraryKind::Snippet => {}
    }
    Ok(())
}

/// Quicklink braces are reserved for arguments. Snippets preserve ordinary
/// braces verbatim, replacing only the three documented built-in placeholders.
fn arguments(draft: &LibraryDraft) -> Result<Vec<String>> {
    if draft.kind == LibraryKind::Snippet {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let mut rest = draft.content.as_str();
    while let Some(start) = rest.find('{') {
        if rest[..start].contains('}') {
            return Err("Invalid quicklink placeholder".into());
        }
        let end = rest[start..]
            .find('}')
            .ok_or("Unclosed quicklink placeholder")?
            + start;
        let token = &rest[start + 1..end];
        let name = if token == "query" {
            "query"
        } else {
            token
                .strip_prefix("argument:")
                .ok_or("Use {query} or {argument:name}")?
        };
        if name.is_empty()
            || name.len() > 32
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(
                "Argument names use 1–32 ASCII letters, digits, underscores or hyphens".into(),
            );
        }
        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_owned());
        }
        rest = &rest[end + 1..];
    }
    if rest.contains('}') || names.len() > MAX_ARGUMENTS {
        return Err("Invalid or excessive quicklink arguments (maximum 16)".into());
    }
    Ok(names)
}

fn encode_argument(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn render_quicklink(draft: &LibraryDraft, values: &BTreeMap<String, String>) -> Result<String> {
    let names = arguments(draft)?;
    if values.len() != names.len() || names.iter().any(|name| !values.contains_key(name)) {
        return Err("Supply all named arguments and no extra arguments".into());
    }
    // Arguments must never choose the protocol, authority, or local file path.
    let scheme_end = draft
        .content
        .find(':')
        .ok_or("Use a complete URL or file URL")?;
    let scheme = &draft.content[..scheme_end];
    let protected_end = if draft.content[scheme_end + 1..].starts_with("//") {
        let start = scheme_end + 3;
        start
            + draft.content[start..]
                .find(['/', '?', '#'])
                .unwrap_or(draft.content.len() - start)
    } else {
        scheme_end + 1
    };
    if draft.content[..protected_end].contains('{')
        || (scheme.eq_ignore_ascii_case("file") && !names.is_empty())
    {
        return Err("Arguments cannot change the scheme, host, or local file path".into());
    }
    let mut rendered = draft.content.clone();
    for name in names {
        let value = &values[&name];
        if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
            return Err("Each argument must contain 1–4096 bytes".into());
        }
        let encoded = encode_argument(value);
        rendered = replace_bounded(&rendered, &format!("{{argument:{name}}}"), &encoded)?;
        if name == "query" {
            rendered = replace_bounded(&rendered, "{query}", &encoded)?;
        }
    }
    if rendered.len() > MAX_RENDERED {
        return Err("Expanded quicklink is too large".into());
    }
    validate_target(&rendered)?;
    Ok(rendered)
}

fn replace_bounded(content: &str, token: &str, value: &str) -> Result<String> {
    let count = content.matches(token).count();
    let size = content.len() - count * token.len() + count * value.len();
    if size > MAX_RENDERED {
        return Err("Expanded quicklink is too large".into());
    }
    Ok(content.replace(token, value))
}

enum Target {
    Url(String),
    File(PathBuf),
}

fn validate_target(value: &str) -> Result<Target> {
    if value.chars().any(char::is_control) || value.trim() != value {
        return Err("URLs cannot contain whitespace at their edges or control characters".into());
    }
    let url = url::Url::parse(value).map_err(|_| "Use a complete URL or file URL")?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Credentials in quicklink URLs are not supported".into());
    }
    match url.scheme() {
        "http" | "https" if url.host_str().is_some() => Ok(Target::Url(url.into())),
        // Deliberately narrow: no shell, javascript, data, command, vscode, or
        // arbitrary custom protocols. These schemes open a foreground handler.
        "mailto" | "tel" | "spotify" => Ok(Target::Url(url.into())),
        "file" => {
            if url.host_str().is_some_and(|host| host != "localhost")
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("Use a local file URL without a query or fragment".into());
            }
            let path = url.to_file_path().map_err(|_| "Invalid local file URL")?;
            if !path.is_absolute() {
                return Err("File paths must be absolute".into());
            }
            Ok(Target::File(path))
        }
        _ => Err("Allowed schemes: https, http, file, mailto, tel, spotify".into()),
    }
}

fn render_snippet(
    content: &str,
    date: &str,
    time: &str,
    clipboard: Option<&str>,
) -> Result<String> {
    let mut rendered = String::new();
    let mut rest = content;
    // Single pass: clipboard contents are never reinterpreted as placeholders.
    while let Some(start) = rest.find('{') {
        rendered.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(tail) = rest.strip_prefix("{date}") {
            rendered.push_str(date);
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("{time}") {
            rendered.push_str(time);
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("{clipboard}") {
            rendered.push_str(clipboard.ok_or("Allow clipboard access for this invocation")?);
            rest = tail;
        } else {
            rendered.push('{');
            rest = &rest[1..];
        }
        if rendered.len() > MAX_RENDERED {
            return Err("Expanded snippet exceeds 64 KiB".into());
        }
    }
    rendered.push_str(rest);
    if rendered.len() > MAX_RENDERED {
        return Err("Expanded snippet exceeds 64 KiB".into());
    }
    Ok(rendered)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum LibraryAction {
    Open,
    Copy,
    Paste,
}

#[tauri::command]
pub fn library_list(
    query: String,
    state: tauri::State<'_, LibraryState>,
) -> std::result::Result<Vec<LibraryItem>, String> {
    state.search_items(&query)
}

#[tauri::command]
pub fn library_get(
    id: String,
    state: tauri::State<'_, LibraryState>,
) -> std::result::Result<LibraryEntry, String> {
    state.get(&id)
}

#[tauri::command]
pub async fn library_save(
    id: Option<String>,
    draft: LibraryDraft,
    app: AppHandle,
) -> std::result::Result<LibraryEntry, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<LibraryEntry> {
        let entry = app.state::<LibraryState>().save(id.as_deref(), draft)?;
        // The write has committed. A missing event listener must not turn a
        // successful create into an error that invites a duplicate retry.
        let _ = app.emit("library-changed", ());
        Ok(entry)
    })
    .await
    .map_err(|_| "Library task failed".to_owned())?
}

#[tauri::command]
pub async fn library_delete(
    id: String,
    confirmed: bool,
    app: AppHandle,
) -> std::result::Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        app.state::<LibraryState>().delete(&id, confirmed)?;
        let _ = app.emit("library-changed", ());
        Ok(())
    })
    .await
    .map_err(|_| "Library task failed".to_owned())?
}

fn check_enabled(settings: &crate::settings::Settings, id: &str) -> Result<()> {
    if settings
        .item_preferences
        .get(id)
        .is_some_and(|preference| preference.disabled)
    {
        return Err("This library item is disabled in settings".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn library_execute(
    id: String,
    action: LibraryAction,
    arguments: BTreeMap<String, String>,
    allow_clipboard: bool,
    app: AppHandle,
) -> std::result::Result<(), String> {
    let paste_target = if matches!(action, LibraryAction::Paste) {
        Some(super::paste::prepare(&app)?)
    } else {
        None
    };
    let worker_app = app.clone();
    let paste = tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>> {
        let app = worker_app;
        check_enabled(&app.state::<super::LauncherState>().settings(), &id)?;
        let entry = app.state::<LibraryState>().get(&id)?;
        // State lock is released before any OS operation.
        validate_draft(&entry.draft)?;
        match (entry.draft.kind, action) {
            (LibraryKind::Quicklink, LibraryAction::Open) => {
                let rendered = render_quicklink(&entry.draft, &arguments)?;
                match validate_target(&rendered)? {
                    Target::Url(url) => app.opener().open_url(url, None::<&str>),
                    Target::File(path) => {
                        if !path.exists() {
                            return Err("The local file or folder no longer exists".into());
                        }
                        app.opener().open_path(path.to_string_lossy(), None::<&str>)
                    }
                }
                .map_err(|_| "Could not open quicklink".to_owned())?;
                Ok(None)
            }
            (LibraryKind::Snippet, LibraryAction::Copy | LibraryAction::Paste) => {
                if !arguments.is_empty() {
                    return Err("Snippets do not accept quicklink arguments".into());
                }
                let clipboard = if entry.draft.content.contains("{clipboard}") {
                    if !allow_clipboard {
                        return Err("Allow clipboard access for this invocation".into());
                    }
                    let text = app
                        .clipboard()
                        .read_text()
                        .map_err(|_| "Could not read clipboard text")?;
                    if text.len() > MAX_RENDERED {
                        return Err("Clipboard text exceeds 64 KiB".into());
                    }
                    Some(text)
                } else {
                    None
                };
                let now = Local::now();
                let text = render_snippet(
                    &entry.draft.content,
                    &now.format("%Y-%m-%d").to_string(),
                    &now.format("%H:%M").to_string(),
                    clipboard.as_deref(),
                )?;
                if matches!(action, LibraryAction::Paste) {
                    Ok(Some(text))
                } else {
                    super::clipboard::write_secret(&text)
                        .map_err(|_| "Could not copy snippet".to_owned())?;
                    Ok(None)
                }
            }
            _ => Err("This action is not supported for this library item".into()),
        }
    })
    .await
    .map_err(|_| "Library task failed".to_owned())??;
    if let Some(text) = paste {
        super::paste::paste_text(
            &app,
            text,
            paste_target.ok_or("No paste target was captured.")?,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(kind: LibraryKind, content: &str) -> LibraryDraft {
        LibraryDraft {
            kind,
            name: "Example".into(),
            keywords: "ex".into(),
            content: content.into(),
        }
    }

    #[test]
    fn disabled_preferences_block_execution_but_hidden_items_remain_invocable() {
        let id = "library:00000000000000000000000000000001";
        let mut settings = crate::settings::Settings::default();
        assert!(check_enabled(&settings, id).is_ok());
        settings.item_preferences.insert(
            id.into(),
            crate::settings::ItemPreference {
                hidden: true,
                disabled: false,
                ..Default::default()
            },
        );
        assert!(check_enabled(&settings, id).is_ok());
        settings.item_preferences.get_mut(id).unwrap().disabled = true;
        assert!(check_enabled(&settings, id).is_err());
        assert!(check_enabled(&settings, "library:another").is_ok());
    }

    #[test]
    fn direct_insertion_keeps_clipboard_consent_and_quicklink_input_explicit() {
        let mut entry = LibraryEntry {
            id: "library:fixture".into(),
            draft: draft(LibraryKind::Snippet, "Hello {date} at {time}"),
        };
        assert!(entry.inserts_directly());
        entry.draft.content = "Hello {clipboard}".into();
        assert!(!entry.inserts_directly());
        entry.draft = draft(LibraryKind::Quicklink, "https://example.com/{query}");
        assert!(!entry.inserts_directly());
    }

    #[test]
    fn persistence_confirmation_and_backend_ids() {
        let dir = tempfile::tempdir().unwrap();
        let library = LibraryState::open(dir.path().into()).unwrap();
        let entry = library
            .save(None, draft(LibraryKind::Snippet, "Hello {date}"))
            .unwrap();
        assert!(library.save(Some("forged"), entry.draft.clone()).is_err());
        assert!(library.delete(&entry.id, false).is_err());
        let loaded = LibraryState::open(dir.path().into()).unwrap();
        assert_eq!(loaded.get(&entry.id).unwrap().draft.content, "Hello {date}");
        assert_eq!(loaded.search_items("EX").unwrap().len(), 1);
        library
            .save(
                Some(&entry.id),
                draft(LibraryKind::Snippet, "Edited content"),
            )
            .unwrap();
        assert_eq!(
            LibraryState::open(dir.path().into())
                .unwrap()
                .get(&entry.id)
                .unwrap()
                .draft
                .content,
            "Edited content"
        );
        library.delete(&entry.id, true).unwrap();
        assert!(
            LibraryState::open(dir.path().into())
                .unwrap()
                .search_items("")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unavailable_state_cannot_read_write_or_reset_storage() {
        let library = LibraryState::unavailable("Library could not be read".into());
        assert_eq!(
            library.search_items("").unwrap_err(),
            "Library could not be read"
        );
        assert_eq!(
            library.get("library:unknown").unwrap_err(),
            "Library could not be read"
        );
        assert_eq!(
            library
                .save(None, draft(LibraryKind::Snippet, "x"))
                .unwrap_err(),
            "Library could not be read"
        );
        assert_eq!(
            library.delete("library:unknown", true).unwrap_err(),
            "Library could not be read"
        );
    }

    #[test]
    fn corrupt_storage_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, b"private broken content").unwrap();
        assert!(LibraryState::open(dir.path().into()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"private broken content");
    }

    #[test]
    fn failed_atomic_save_does_not_change_live_items() {
        let dir = tempfile::tempdir().unwrap();
        let library = LibraryState::open(dir.path().into()).unwrap();
        fs::create_dir(dir.path().join("library.json")).unwrap();
        assert!(
            library
                .save(None, draft(LibraryKind::Snippet, "content"))
                .is_err()
        );
        assert!(library.search_items("").unwrap().is_empty());
    }

    #[test]
    fn templates_encode_arguments_and_reject_unsafe_targets() {
        let link = draft(
            LibraryKind::Quicklink,
            "https://example.com/search?q={query}&scope={argument:scope}",
        );
        let values = BTreeMap::from([
            ("query".into(), "a &b={date}".into()),
            ("scope".into(), "a/b".into()),
        ]);
        assert_eq!(
            render_quicklink(&link, &values).unwrap(),
            "https://example.com/search?q=a%20%26b%3D%7Bdate%7D&scope=a%2Fb"
        );
        assert!(render_quicklink(&link, &BTreeMap::new()).is_err());
        let repeated = draft(
            LibraryKind::Quicklink,
            &format!("https://example.com/?{}", "{query}".repeat(200)),
        );
        assert!(
            render_quicklink(
                &repeated,
                &BTreeMap::from([("query".into(), "x".repeat(4096))])
            )
            .is_err()
        );
        for value in [
            "javascript:alert(1)",
            "data:text/plain,hi",
            "vscode://command/x",
            "https://user:secret@example.com",
            "https://{query}/",
            "{query}://example.com",
            "file:///tmp/{query}",
            "FILE:///tmp/{query}",
            "file://remote/share",
            "https://example.com/{unknown}",
        ] {
            assert!(
                validate_draft(&draft(LibraryKind::Quicklink, value)).is_err(),
                "{value}"
            );
        }
        for value in [
            "https://example.com",
            "spotify:track:123",
            "mailto:hello@example.com",
        ] {
            assert!(
                validate_draft(&draft(LibraryKind::Quicklink, value)).is_ok(),
                "{value}"
            );
        }
        let path = std::env::temp_dir().join("TinyDash local café example");
        let file_url = url::Url::from_file_path(&path).unwrap();
        assert!(validate_draft(&draft(LibraryKind::Quicklink, file_url.as_str())).is_ok());
        assert!(
            matches!(validate_target(file_url.as_str()).unwrap(), Target::File(found) if found == path)
        );
    }

    #[test]
    fn snippets_are_single_pass_and_clipboard_is_explicit() {
        let content = "{date} {time} {clipboard} {literal}";
        assert!(render_snippet(content, "2026-09-20", "09:30", None).is_err());
        assert_eq!(
            render_snippet(content, "2026-09-20", "09:30", Some("{date}")).unwrap(),
            "2026-09-20 09:30 {date} {literal}"
        );
        assert!(
            render_snippet(
                "{clipboard}{clipboard}",
                "",
                "",
                Some(&"x".repeat(MAX_RENDERED))
            )
            .is_err()
        );
        assert!(validate_draft(&draft(LibraryKind::Snippet, "{selection}")).is_err());
    }

    #[test]
    fn bounds_are_enforced() {
        assert!(validate_draft(&draft(LibraryKind::Snippet, &"x".repeat(MAX_TEXT + 1))).is_err());
        let mut document = Document {
            version: 1,
            entries: Vec::new(),
        };
        for n in 0..=MAX_ITEMS {
            document.entries.push(LibraryEntry {
                id: format!("library:{n:032x}"),
                draft: draft(LibraryKind::Snippet, "x"),
            });
        }
        assert!(validate_document(&document).is_err());
    }
}
