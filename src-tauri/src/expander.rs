//! Snippet expansion as you type. While the setting is on, the platform
//! layer reads the keys typed in other apps, `features/expansion.rs` finds a
//! trigger such as `;sig` in them, and a worker deletes the trigger and types
//! the snippet into the app or window where the trigger was typed, unless
//! the user pressed a key or clicked since. Typed text stays in memory, at
//! most [`MAX_TYPED`](crate::features::expansion::MAX_TYPED) characters, and
//! is never logged or saved.

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

/// What the key watch and the worker share about the trigger that the
/// worker holds.
#[derive(Default)]
struct Handover {
    /// Set from a trigger until its snippet is typed, plus [`SETTLE`].
    /// Meanwhile the app shows text that the typed buffer lacks, so each key
    /// is a reset: no trigger starts inside the snippet, and none waits to
    /// erase the wrong text.
    typing: AtomicBool,
    /// Set by each key or click that the watch passes while `typing` is
    /// set, and cleared when the next trigger is handed over. The user typed
    /// on, clicked elsewhere, or switched tabs, so the text before the cursor
    /// may no longer end with the trigger: a Backspace would erase their
    /// text, and the snippet could land in the wrong place. On Linux, the
    /// keys that TinyDash types set it too, so `type_text` there checks it
    /// before the first key only, and then stops only at a click.
    interrupted: AtomicBool,
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
    let handover = Arc::new(Handover::default());
    let (found, triggers) = mpsc::sync_channel(1);
    let typist = app.clone();
    let typist_handover = Arc::clone(&handover);
    std::thread::Builder::new()
        .name("snippet-typing".into())
        .spawn(move || {
            type_snippets(
                &triggers,
                &typist_handover,
                |expansion| fill(&typist, expansion),
                platform::type_text,
            );
        })?;
    let watcher = app.clone();
    let library = move || watcher.state::<State>().library.get();
    // If the watch cannot start, `found` drops with it, and the worker ends.
    *watch = Some(platform::watch_typing(find_triggers(
        library, found, handover,
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

/// What the watch thread runs for each key or click, with the app or
/// window that got it: match the key against the snippet keywords from
/// `library`, and hand a trigger to the worker with where it was typed. The
/// worker types there only, so focus that moves after the trigger, even
/// while the clipboard is read, stops it before the first Backspace. A key
/// or click after the trigger sets `interrupted`, which stops it too. The
/// keywords are built again only when a save swaps in a new library.
fn find_triggers<Focus: Send + 'static>(
    library: impl Fn() -> Arc<Library> + Send + 'static,
    found: mpsc::SyncSender<(Expansion, Focus)>,
    handover: Arc<Handover>,
) -> impl FnMut(Key, Focus) + Send + 'static {
    let mut typed = Typed::default();
    let mut known: Option<Arc<Library>> = None;
    let mut keywords = Keywords::default();
    move |key, focus| {
        let key = if handover.typing.load(Ordering::Acquire) {
            handover.interrupted.store(true, Ordering::Release);
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
        handover.interrupted.store(false, Ordering::Release);
        handover.typing.store(true, Ordering::Release);
        if found.try_send((expansion, focus)).is_err() {
            handover.typing.store(false, Ordering::Release);
            tracing::warn!("A snippet did not expand: its typing worker has stopped");
        }
    }
}

/// The worker: fill each trigger's snippet with `fill`, then type it with
/// `type_text` where the trigger was typed. A key or click since the
/// trigger, also while `fill` read the clipboard, leaves the trigger as
/// typed.
fn type_snippets<Focus>(
    triggers: &mpsc::Receiver<(Expansion, Focus)>,
    handover: &Handover,
    fill: impl Fn(&Expansion) -> Option<String>,
    type_text: impl Fn(Focus, usize, &str, &AtomicBool) -> Result<()>,
) {
    for (expansion, focus) in triggers {
        if let Some(text) = fill(&expansion) {
            let interrupted = &handover.interrupted;
            let typed = platform::no_input_since_trigger(interrupted)
                .and_then(|()| type_text(focus, expansion.erase, &text, interrupted));
            if let Err(error) = typed {
                tracing::warn!(%error, "Could not type a snippet");
            }
        }
        std::thread::sleep(SETTLE);
        handover.typing.store(false, Ordering::Release);
    }
}

/// The snippet of a trigger, filled as when it is copied. `{clipboard}` is
/// read the way quicklinks read it: a copy that its app marked secret, such
/// as a password, is never typed as keys that other apps can read.
fn fill(app: &AppHandle, expansion: &Expansion) -> Option<String> {
    let library = app.state::<State>().library.get();
    let clipboard = || platform::clipboard_text(app);
    expansion.text(library.items(), chrono::Local::now(), clipboard)
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
            text: format!("Text of {keyword}"),
        }]))
    }

    /// The window that got a key, as the watch passes it.
    type Window = &'static str;

    /// The snippet text of the next trigger that the worker gets, filled
    /// from `library`, and the window to type it in.
    fn next_trigger(
        triggers: &mpsc::Receiver<(Expansion, Window)>,
        library: &Mutex<Arc<Library>>,
    ) -> Option<(String, Window)> {
        let (expansion, window) = triggers.try_recv().ok()?;
        let library = Arc::clone(&library.lock().unwrap());
        let text = expansion.text(library.items(), chrono::Local::now(), || None)?;
        Some((text, window))
    }

    #[test]
    fn hands_each_trigger_to_the_worker_once() {
        let saved = Arc::new(Mutex::new(library("sig")));
        let current = Arc::clone(&saved);
        let handover = Arc::new(Handover::default());
        let (found, triggers) = mpsc::sync_channel(1);
        let mut on_key = find_triggers(
            move || Arc::clone(&current.lock().unwrap()),
            found,
            Arc::clone(&handover),
        );
        let mut type_keys = |text: &str, window: Window| {
            text.chars().for_each(|c| on_key(Key::Char(c), window));
        };
        let typing = &handover.typing;

        // While a snippet types, the app shows text that the watch never
        // saw, so no key there starts a trigger.
        typing.store(true, Ordering::Release);
        type_keys(";sig", "editor");
        assert!(triggers.try_recv().is_err());

        typing.store(false, Ordering::Release);
        type_keys(";sig", "editor");
        assert_eq!(
            next_trigger(&triggers, &saved),
            Some(("Text of sig".into(), "editor"))
        );
        assert!(typing.load(Ordering::Acquire));

        // The worker gets the window where the trigger was typed, not the
        // one in front when it starts to type.
        typing.store(false, Ordering::Release);
        type_keys(" ;sig", "chat");
        assert_eq!(
            next_trigger(&triggers, &saved),
            Some(("Text of sig".into(), "chat"))
        );

        // A save swaps in a new library, and its keywords count at once.
        typing.store(false, Ordering::Release);
        *saved.lock().unwrap() = library("bye");
        type_keys(" ;sig ;bye", "editor");
        assert_eq!(
            next_trigger(&triggers, &saved),
            Some(("Text of bye".into(), "editor"))
        );
        assert!(triggers.try_recv().is_err());

        // A worker that has stopped takes nothing, so the keys count again.
        drop(triggers);
        typing.store(false, Ordering::Release);
        type_keys(" ;bye", "editor");
        assert!(!typing.load(Ordering::Acquire));
    }

    #[test]
    fn marks_a_key_or_click_after_the_trigger() {
        let saved = library("sig");
        let handover = Arc::new(Handover::default());
        let (found, triggers) = mpsc::sync_channel(1);
        let mut on_key = find_triggers(move || Arc::clone(&saved), found, Arc::clone(&handover));
        let interrupted = || handover.interrupted.load(Ordering::Acquire);

        // A key and a click before the trigger do not count.
        on_key(Key::Char('x'), "editor");
        on_key(Key::Reset, "editor");
        ";sig".chars().for_each(|c| on_key(Key::Char(c), "editor"));
        assert!(triggers.try_recv().is_ok());
        assert!(!interrupted());

        // The user types on before the snippet is typed.
        on_key(Key::Char(' '), "editor");
        assert!(interrupted());

        // The next trigger starts afresh, and a click after it counts.
        handover.typing.store(false, Ordering::Release);
        ";sig".chars().for_each(|c| on_key(Key::Char(c), "editor"));
        assert!(triggers.try_recv().is_ok());
        assert!(!interrupted());
        on_key(Key::Reset, "editor");
        assert!(interrupted());
    }

    /// Type `;sig` in the editor, after a key and a click, and press
    /// `keys_while_filling` while the worker fills the snippet, as when the
    /// clipboard is slow to read. What the worker typed: the window, the
    /// Backspaces, and the text.
    fn expand(keys_while_filling: &[Key]) -> Vec<(Window, usize, String)> {
        let saved = library("sig");
        let items = Arc::clone(&saved);
        let handover = Arc::new(Handover::default());
        let (found, triggers) = mpsc::sync_channel(1);
        let mut on_key = find_triggers(move || Arc::clone(&saved), found, Arc::clone(&handover));
        on_key(Key::Char('x'), "editor");
        on_key(Key::Reset, "editor");
        ";sig".chars().for_each(|c| on_key(Key::Char(c), "editor"));
        // The watch stops while the snippet fills, so the worker ends after
        // this trigger.
        let watch = std::cell::RefCell::new(Some(on_key));
        let typed = std::cell::RefCell::new(Vec::new());
        type_snippets(
            &triggers,
            &handover,
            |expansion| {
                if let Some(mut on_key) = watch.borrow_mut().take() {
                    for &key in keys_while_filling {
                        on_key(key, "editor");
                    }
                }
                expansion.text(items.items(), chrono::Local::now(), || None)
            },
            |window, erase, text, _| {
                typed.borrow_mut().push((window, erase, text.to_owned()));
                Ok(())
            },
        );
        assert!(!handover.typing.load(Ordering::Acquire));
        typed.into_inner()
    }

    #[test]
    fn types_nothing_after_a_key_or_click_that_follows_the_trigger() {
        assert_eq!(expand(&[]), [("editor", 4, "Text of sig".to_owned())]);
        // The user types on, or clicks elsewhere, before typing starts.
        assert!(expand(&[Key::Char(' ')]).is_empty());
        assert!(expand(&[Key::Reset]).is_empty());
    }
}
