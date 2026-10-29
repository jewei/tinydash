//! Snippet expansion as you type. While the setting is on, the platform
//! layer reads the keys typed in other apps, `features/expansion.rs` finds a
//! trigger such as `;sig` in them, and a worker deletes the trigger and types
//! the snippet. Typed text stays in memory, at most
//! [`MAX_TYPED`](crate::features::expansion::MAX_TYPED) characters, and is
//! never logged or saved.

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
        expansion::{Expansion, Key, Keywords, Typed},
        library::Library,
    },
    platform,
    state::State,
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
    if watch
        .as_ref()
        .is_some_and(platform::TypingWatch::is_running)
    {
        return Ok(());
    }
    // A watch that ended on its own, as on macOS when access was taken
    // away, reads no keys: start a new one in its place.
    if let Some(ended) = watch.take() {
        ended.stop();
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
    let watcher = app.clone();
    let library = move || watcher.state::<State>().library.get();
    // If the watch cannot start, `found` drops with it, and the worker ends.
    *watch = Some(platform::watch_typing(find_triggers(
        library, found, typing,
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
/// keywords from `library`, and hand a trigger to the worker. The keywords
/// are built again only when a save swaps in a new library.
fn find_triggers(
    library: impl Fn() -> Arc<Library> + Send + 'static,
    found: mpsc::SyncSender<Expansion>,
    typing: Arc<AtomicBool>,
) -> impl FnMut(Key) + Send + 'static {
    let mut typed = Typed::default();
    let mut known: Option<Arc<Library>> = None;
    let mut keywords = Keywords::default();
    move |key| {
        let key = if typing.load(Ordering::Acquire) {
            Key::Reset
        } else {
            key
        };
        let current = library();
        if !known
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, &current))
        {
            keywords = Keywords::new(current.items());
            known = Some(current);
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

/// The worker: type each trigger's snippet.
fn type_snippets(app: &AppHandle, triggers: &mpsc::Receiver<Expansion>, typing: &AtomicBool) {
    for expansion in triggers {
        type_snippet(app, &expansion);
        std::thread::sleep(SETTLE);
        typing.store(false, Ordering::Release);
    }
}

fn type_snippet(app: &AppHandle, expansion: &Expansion) {
    let library = app.state::<State>().library.get();
    // Read the way quicklinks read it: a copy that its app marked secret,
    // such as a password, is never typed as keys that other apps can read.
    let clipboard = || platform::clipboard_text(app);
    let Some(text) = expansion.text(library.items(), chrono::Local::now(), clipboard) else {
        return;
    };
    if let Err(error) = platform::type_text(expansion.erase, &text) {
        tracing::warn!(%error, "Could not type a snippet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::library::{LibraryItem, LibraryKind};

    fn library(keyword: &str) -> Arc<Library> {
        Arc::new(Library::new(vec![LibraryItem {
            id: Some(1),
            kind: LibraryKind::Snippet,
            name: keyword.into(),
            keyword: keyword.into(),
            text: "text".into(),
        }]))
    }

    #[test]
    fn hands_each_trigger_to_the_worker_once() {
        let saved = Arc::new(Mutex::new(library("sig")));
        let current = Arc::clone(&saved);
        let typing = Arc::new(AtomicBool::new(false));
        let (found, triggers) = mpsc::sync_channel(1);
        let mut on_key = find_triggers(
            move || Arc::clone(&current.lock().unwrap()),
            found,
            Arc::clone(&typing),
        );
        let mut type_keys = |text: &str| text.chars().for_each(|c| on_key(Key::Char(c)));
        let keyword = |expansion: Expansion| expansion.keyword;

        // While a snippet types, the app shows text that the watch never
        // saw, so no key there starts a trigger.
        typing.store(true, Ordering::Release);
        type_keys(";sig");
        assert!(triggers.try_recv().is_err());

        typing.store(false, Ordering::Release);
        type_keys(";sig");
        assert_eq!(triggers.try_recv().map(keyword), Ok("sig".into()));
        assert!(typing.load(Ordering::Acquire));

        // A save swaps in a new library, and its keywords count at once.
        typing.store(false, Ordering::Release);
        *saved.lock().unwrap() = library("bye");
        type_keys(" ;sig ;bye");
        assert_eq!(triggers.try_recv().map(keyword), Ok("bye".into()));
        assert!(triggers.try_recv().is_err());

        // A worker that has stopped takes nothing, so the keys count again.
        drop(triggers);
        typing.store(false, Ordering::Release);
        type_keys(" ;bye");
        assert!(!typing.load(Ordering::Acquire));
    }
}
