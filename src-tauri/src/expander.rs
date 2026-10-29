//! Snippet expansion as you type. While the setting is on, the platform
//! layer reads the keys typed in other apps, `features/expansion.rs` finds a
//! trigger such as `;sig` in them, and a worker deletes the trigger and types
//! the snippet. Typed text stays in memory, at most
//! [`expansion::MAX_TYPED`] characters, and is never logged or saved.

use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

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

/// How long keys still count as a reset after a snippet is typed: the OS
/// may pass the last typed keys to the watch a moment later.
const SETTLE: Duration = Duration::from_millis(50);

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
    // Set from a trigger until its snippet is typed. Meanwhile the app shows
    // text that the typed buffer lacks, so each key is a reset: no trigger
    // starts inside the snippet, and none waits to erase the wrong text.
    let typing = Arc::new(AtomicBool::new(false));
    let (found, triggers) = mpsc::sync_channel(1);
    let typist = app.clone();
    let typist_typing = Arc::clone(&typing);
    std::thread::Builder::new()
        .name("snippet-typing".into())
        .spawn(move || type_snippets(&typist, &triggers, &typist_typing))?;
    // If the watch cannot start, `found` drops with it, and the worker ends.
    *watch = Some(platform::watch_typing(find_triggers(
        app.clone(),
        found,
        typing,
    ))?);
    Ok(())
}

/// Stop reading keys, which forgets the typed text. The worker ends once it
/// has typed the trigger it holds.
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
    typing: Arc<AtomicBool>,
) -> impl FnMut(Key) + Send + 'static {
    let mut typed = Typed::default();
    let mut library: Option<Arc<Library>> = None;
    let mut keywords = Keywords::default();
    move |key| {
        let key = if typing.load(Ordering::Acquire) {
            Key::Reset
        } else {
            key
        };
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
        typing.store(true, Ordering::Release);
        if found.try_send(expansion).is_err() {
            typing.store(false, Ordering::Release);
            tracing::warn!("A snippet did not expand: its typing worker has stopped");
        }
    }
}

/// The worker: type each trigger's snippet, filled as when it is copied
/// from the launcher, with no text after the keyword, so `{query}` is empty.
fn type_snippets(app: &AppHandle, triggers: &mpsc::Receiver<Expansion>, typing: &AtomicBool) {
    for expansion in triggers {
        type_snippet(app, &expansion);
        std::thread::sleep(SETTLE);
        typing.store(false, Ordering::Release);
    }
}

fn type_snippet(app: &AppHandle, expansion: &Expansion) {
    let library = app.state::<State>().library.get();
    // The snippet was deleted or lost its keyword since.
    let Some(snippet) = expansion::snippet(library.items(), &expansion.keyword) else {
        return;
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
