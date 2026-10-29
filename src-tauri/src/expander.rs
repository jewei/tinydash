//! Snippet expansion as you type. While the setting is on, the platform
//! layer reads the keys typed in other apps, `features/expansion.rs` finds a
//! trigger such as `;sig` in them, and a worker deletes the trigger and types
//! the snippet. Typed text stays in memory, at most
//! [`expansion::MAX_TYPED`] characters, and is never logged or saved.

use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use tauri::{AppHandle, Manager};

use crate::{
    error::{Error, Result},
    features::{
        expansion::{self, Expansion, Key, Keywords, Typed},
        library::{self, Library},
    },
    platform,
    state::State,
    system_clipboard,
};

/// Triggers that wait while the worker types. A trigger typed while the
/// queue is full stays as the user typed it.
const QUEUE: usize = 4;

/// The key watch while snippets expand. Changed under the settings lock, at
/// startup, and at exit.
static WATCH: Mutex<Option<platform::TypingWatch>> = Mutex::new(None);

fn watch() -> MutexGuard<'static, Option<platform::TypingWatch>> {
    WATCH.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start or stop to match the setting. Turning it on lets the OS ask for
/// access (macOS), and fails with what to allow when it is missing.
pub fn apply(app: &AppHandle, on: bool) -> Result<()> {
    if on {
        start(app, true)
    } else {
        stop();
        Ok(())
    }
}

/// Start reading keys, unless that runs already. `ask` lets the OS show its
/// access prompts.
pub fn start(app: &AppHandle, ask: bool) -> Result<()> {
    let mut watch = watch();
    if watch.is_some() {
        return Ok(());
    }
    if let Some(reason) = platform::typing_unavailable() {
        return Err(Error::msg(reason));
    }
    platform::typing_access(ask)?;
    let (found, triggers) = mpsc::sync_channel(QUEUE);
    let typist = app.clone();
    std::thread::Builder::new()
        .name("snippet-typing".into())
        .spawn(move || type_snippets(&typist, &triggers))?;
    // If the watch cannot start, `found` drops with it, and the worker ends.
    *watch = Some(platform::watch_typing(find_triggers(app.clone(), found))?);
    Ok(())
}

/// Stop reading keys, which forgets the typed text. The worker ends once it
/// has typed the triggers it holds.
pub fn stop() {
    // Out of the lock first; the watch thread never takes it.
    let running = watch().take();
    if let Some(running) = running {
        running.stop();
    }
}

/// What the watch thread runs for each key: match it against the snippet
/// keywords, and hand a trigger to the worker. The keywords are built again
/// only when a save swaps in a new library.
fn find_triggers(
    app: AppHandle,
    found: mpsc::SyncSender<Expansion>,
) -> impl FnMut(Key) + Send + 'static {
    let mut typed = Typed::default();
    let mut library: Option<Arc<Library>> = None;
    let mut keywords = Keywords::default();
    move |key| {
        let current = app.state::<State>().library.get();
        if !library
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, &current))
        {
            keywords = Keywords::new(current.items());
            library = Some(current);
        }
        let Some(expansion) = typed.press(key, &keywords) else {
            return;
        };
        if found.try_send(expansion).is_err() {
            tracing::warn!("A snippet did not expand: TinyDash was still typing others");
        }
    }
}

/// The worker: type each trigger's snippet, filled as when it is copied
/// from the launcher, with no text after the keyword, so `{query}` is empty.
fn type_snippets(app: &AppHandle, triggers: &mpsc::Receiver<Expansion>) {
    for expansion in triggers {
        let library = app.state::<State>().library.get();
        // The snippet was deleted or lost its keyword since.
        let Some(snippet) = expansion::snippet(library.items(), &expansion.keyword) else {
            continue;
        };
        let text = library::render_snippet(
            &snippet.text,
            "",
            chrono::Local::now(),
            system_clipboard::read_text,
        );
        if let Err(error) = platform::type_text(expansion.erase, &expansion.replacement(text)) {
            tracing::warn!(%error, "Could not type a snippet");
        }
    }
}
