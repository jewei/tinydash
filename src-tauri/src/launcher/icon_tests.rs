use super::*;
use std::sync::{
    Condvar,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

fn store(
    loader: impl Fn(&Path, u16) -> Option<Vec<u8>> + Send + Sync + 'static,
    budget: usize,
) -> (IconStore, Vec<String>) {
    let store = IconStore(Arc::new(Inner {
        state: Mutex::new(State::default()),
        loader: Arc::new(loader),
        budget,
        enabled: true,
    }));
    let mut entries: Vec<_> = (0..100)
        .map(|id| AppEntry::new(format!("App {id}"), format!("/fixture/{id}").into(), vec![]))
        .collect();
    store.replace_catalog(&mut entries, || {});
    (
        store,
        entries
            .into_iter()
            .map(|entry| entry.icon.unwrap())
            .collect(),
    )
}

fn request(
    store: &IconStore,
    key: &str,
    pixels: u16,
    id: usize,
) -> Result<oneshot::Receiver<Reply>, IconError> {
    store.request(
        key.into(),
        pixels,
        Request {
            window: "main".into(),
            id: id.to_string(),
        },
        Arc::new(|| {}),
    )
}

fn finish(receiver: oneshot::Receiver<Reply>) -> Reply {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(5), receiver)
            .await
            .expect("icon completion")
            .unwrap()
    })
}

#[test]
fn cache_hits_do_not_send_capacity_notifications() {
    let (store, keys) = store(|_, _| Some(b"image".to_vec()), 16);
    finish(request(&store, &keys[0], 72, 0).unwrap()).unwrap();
    let notifications = Arc::new(AtomicUsize::new(0));
    let count = notifications.clone();
    let ready: Arc<Notify> = Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
    });
    for id in 1..=10 {
        assert_eq!(
            tauri::async_runtime::block_on(store.image(
                keys[0].clone(),
                72,
                Request {
                    window: "main".into(),
                    id: id.to_string()
                },
                ready.clone(),
            )),
            Ok("image".into())
        );
    }
    assert_eq!(notifications.load(Ordering::SeqCst), 0);
}

#[test]
fn cancelling_unknown_requests_does_not_send_capacity_notifications() {
    let (store, _) = store(|_, _| None, 16);
    let notifications = AtomicUsize::new(0);
    store.cancel_request(
        &Request {
            window: "main".into(),
            id: "settled".into(),
        },
        || {
            notifications.fetch_add(1, Ordering::SeqCst);
        },
    );
    assert_eq!(notifications.load(Ordering::SeqCst), 0);
}

#[derive(Default)]
struct LoadGate(Arc<(Mutex<bool>, Condvar)>);

impl LoadGate {
    fn open(&self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}

impl Drop for LoadGate {
    fn drop(&mut self) {
        self.open();
    }
}

fn wait_for_gate(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let _guard = gate
        .1
        .wait_while(gate.0.lock().unwrap(), |open| !*open)
        .unwrap();
}

fn notification_counter() -> (Arc<AtomicUsize>, Arc<Notify>, mpsc::Receiver<()>) {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let (send, receive) = mpsc::channel();
    let notify = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        let _ = send.send(());
    });
    (count, notify, receive)
}

fn tracked_request(
    store: &IconStore,
    key: &str,
    window: &str,
    id: usize,
    ready: &Arc<Notify>,
) -> Result<oneshot::Receiver<Reply>, IconError> {
    store.request(
        key.into(),
        72,
        Request {
            window: window.into(),
            id: id.to_string(),
        },
        ready.clone(),
    )
}

#[test]
fn window_cancellation_wakes_another_window_once() {
    let gate = LoadGate::default();
    let worker_gate = gate.0.clone();
    let (store, keys) = store(
        move |_, _| {
            wait_for_gate(&worker_gate);
            Some(b"image".to_vec())
        },
        1024,
    );
    let (count, ready, _) = notification_counter();
    let replies: Vec<_> = keys
        .iter()
        .take(ACTIVE_LOADS + QUEUED_LOADS)
        .enumerate()
        .map(|(id, key)| tracked_request(&store, key, "main", id, &ready).unwrap())
        .collect();
    assert_eq!(
        tracked_request(&store, &keys[66], "settings", 100, &ready).unwrap_err(),
        IconError::Busy
    );
    store.cancel_window("main", || ready());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for reply in replies {
        assert_eq!(finish(reply), Err(IconError::Cancelled));
    }
    store.cancel_window("main", || ready());
    store.cancel_request(
        &Request {
            window: "main".into(),
            id: "0".into(),
        },
        || ready(),
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let resumed = tracked_request(&store, &keys[66], "settings", 100, &ready).unwrap();
    gate.open();
    finish(resumed).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn a_waiter_can_retry_a_duplicate_even_when_the_queue_is_full() {
    let gate = LoadGate::default();
    let worker_gate = gate.0.clone();
    let (store, keys) = store(
        move |_, _| {
            wait_for_gate(&worker_gate);
            Some(b"image".to_vec())
        },
        1024,
    );
    let (count, ready, _) = notification_counter();
    let mut replies: Vec<_> = (0..WAITERS)
        .map(|id| {
            tracked_request(
                &store,
                &keys[if id < 66 { id } else { 0 }],
                "main",
                id,
                &ready,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(
        tracked_request(&store, &keys[0], "settings", 200, &ready).unwrap_err(),
        IconError::Busy
    );
    store.cancel_request(
        &Request {
            window: "main".into(),
            id: "0".into(),
        },
        || ready(),
    );
    assert_eq!(finish(replies.remove(0)), Err(IconError::Cancelled));
    assert_eq!(store.0.state.lock().unwrap().queue.len(), QUEUED_LOADS);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let resumed = tracked_request(&store, &keys[0], "settings", 200, &ready).unwrap();
    gate.open();
    for reply in replies {
        finish(reply).unwrap();
    }
    finish(resumed).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn completion_without_consumers_still_wakes_a_blocked_window() {
    let first = LoadGate::default();
    let others = LoadGate::default();
    let first_worker = first.0.clone();
    let other_workers = others.0.clone();
    let (store, keys) = store(
        move |path, _| {
            wait_for_gate(if path.ends_with("0") {
                &first_worker
            } else {
                &other_workers
            });
            Some(b"image".to_vec())
        },
        1024,
    );
    let (count, ready, notifications) = notification_counter();
    let mut replies: Vec<_> = keys
        .iter()
        .take(ACTIVE_LOADS + QUEUED_LOADS)
        .enumerate()
        .map(|(id, key)| tracked_request(&store, key, "main", id, &ready).unwrap())
        .collect();
    assert_eq!(
        tracked_request(&store, &keys[66], "settings", 100, &ready).unwrap_err(),
        IconError::Busy
    );
    store.cancel_request(
        &Request {
            window: "main".into(),
            id: "0".into(),
        },
        || ready(),
    );
    assert_eq!(finish(replies.remove(0)), Err(IconError::Cancelled));
    assert_eq!(count.load(Ordering::SeqCst), 0, "the queue is still full");
    first.open();
    notifications
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("orphaned completion must release a queue slot");
    let resumed = tracked_request(&store, &keys[66], "settings", 100, &ready).unwrap();
    others.open();
    for reply in replies {
        finish(reply).unwrap();
    }
    finish(resumed).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn catalog_replacement_notifies_blocked_consumers_and_invalidates_old_requests() {
    let gate = LoadGate::default();
    let worker_gate = gate.0.clone();
    let (store, keys) = store(
        move |_, _| {
            wait_for_gate(&worker_gate);
            Some(b"image".to_vec())
        },
        1024,
    );
    let (count, ready, _) = notification_counter();
    let replies: Vec<_> = (0..WAITERS)
        .map(|id| tracked_request(&store, &keys[0], "main", id, &ready).unwrap())
        .collect();
    assert_eq!(
        tracked_request(&store, &keys[0], "settings", 200, &ready).unwrap_err(),
        IconError::Busy
    );
    let mut entries = vec![AppEntry::new(
        "New app".into(),
        "/fixture/new".into(),
        vec![],
    )];
    store.replace_catalog(&mut entries, || ready());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for reply in replies {
        assert_eq!(finish(reply), Err(IconError::Cancelled));
    }
    assert_eq!(
        tracked_request(&store, &keys[0], "settings", 200, &ready).unwrap_err(),
        IconError::Unavailable
    );
    let resumed = tracked_request(
        &store,
        entries[0].icon.as_ref().unwrap(),
        "settings",
        200,
        &ready,
    )
    .unwrap();
    gate.open();
    finish(resumed).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn cold_warm_and_eviction_use_the_byte_budget() {
    let loads = Arc::new(AtomicUsize::new(0));
    let count = loads.clone();
    let (store, keys) = store(
        move |_, size| {
            count.fetch_add(1, Ordering::SeqCst);
            Some(vec![size as u8; 8])
        },
        16,
    );
    assert_eq!(
        &**finish(request(&store, &keys[0], 72, 0).unwrap()).unwrap(),
        &[72; 8]
    );
    finish(request(&store, &keys[1], 72, 1).unwrap()).unwrap();
    finish(request(&store, &keys[0], 72, 2).unwrap()).unwrap();
    assert_eq!(loads.load(Ordering::SeqCst), 2);
    finish(request(&store, &keys[2], 128, 3).unwrap()).unwrap();
    finish(request(&store, &keys[0], 72, 4).unwrap()).unwrap();
    assert_eq!(
        loads.load(Ordering::SeqCst),
        3,
        "recently used entry stays cached"
    );
    finish(request(&store, &keys[1], 72, 5).unwrap()).unwrap();
    assert_eq!(loads.load(Ordering::SeqCst), 4, "oldest entry was evicted");
    assert_eq!(store.0.state.lock().unwrap().bytes, 16);
}

#[test]
fn duplicate_requests_share_work_and_obsolete_work_leaves_the_queue() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_gate = gate.clone();
    let (started, starts) = mpsc::channel();
    let (store, keys) = store(
        move |_, size| {
            started.send(size).unwrap();
            let (lock, wake) = &*worker_gate;
            let _guard = wake
                .wait_while(lock.lock().unwrap(), |ready| !*ready)
                .unwrap();
            Some(vec![1; 8])
        },
        16,
    );
    let first = request(&store, &keys[0], 72, 0).unwrap();
    let duplicate = request(&store, &keys[0], 72, 1).unwrap();
    let second = request(&store, &keys[1], 128, 2).unwrap();
    starts
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    starts
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let mut queued = Vec::new();
    for (index, key) in keys.iter().enumerate().take(66).skip(2) {
        queued.push(request(&store, key, 72, index + 1).unwrap());
    }
    assert_eq!(
        request(&store, &keys[66], 72, 99).unwrap_err(),
        IconError::Busy
    );
    {
        let state = store.0.state.lock().unwrap();
        assert_eq!(state.active, 2);
        assert_eq!(state.queue.len(), 64);
        assert_eq!(state.jobs.len(), 66);
    }
    store.cancel_window("main", || {});
    assert!(store.0.state.lock().unwrap().queue.is_empty());
    assert_eq!(finish(duplicate), Err(IconError::Cancelled));
    assert_eq!(finish(first), Err(IconError::Cancelled));
    assert_eq!(finish(second), Err(IconError::Cancelled));
    for receiver in queued {
        assert_eq!(finish(receiver), Err(IconError::Cancelled));
    }
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    assert!(
        starts.try_recv().is_err(),
        "cancelled queued jobs never start"
    );
}

#[test]
fn oversized_and_missing_images_do_not_grow_the_cache() {
    let (store, keys) = store(
        |path, _| {
            if path.ends_with("0") {
                None
            } else {
                Some(vec![0; 32])
            }
        },
        16,
    );
    assert_eq!(
        finish(request(&store, &keys[0], 72, 0).unwrap()),
        Err(IconError::Unavailable)
    );
    assert_eq!(
        finish(request(&store, &keys[1], 72, 1).unwrap())
            .unwrap()
            .len(),
        32
    );
    assert_eq!(store.0.state.lock().unwrap().bytes, 0);
    assert_eq!(
        store.0.state.lock().unwrap().cache.len(),
        1,
        "only the negative entry is cached"
    );
    assert_eq!(
        request(&store, "/arbitrary/file", 72, 2).unwrap_err(),
        IconError::Unavailable
    );
    assert_eq!(
        request(&store, &keys[1], 257, 3).unwrap_err(),
        IconError::InvalidSize
    );
}

#[test]
fn replacing_an_application_invalidates_old_identity_and_cached_image() {
    let (store, keys) = store(|_, _| Some(vec![1; 8]), 16);
    finish(request(&store, &keys[0], 72, 0).unwrap()).unwrap();
    let mut entries = vec![AppEntry::new(
        "Replaced app".into(),
        "/fixture/0".into(),
        vec![],
    )];
    store.replace_catalog(&mut entries, || {});
    assert_ne!(entries[0].icon.as_ref().unwrap(), &keys[0]);
    assert_eq!(
        request(&store, &keys[0], 72, 1).unwrap_err(),
        IconError::Unavailable
    );
    assert_eq!(store.0.state.lock().unwrap().bytes, 0);
    finish(request(&store, entries[0].icon.as_ref().unwrap(), 128, 2).unwrap()).unwrap();
}

#[test]
fn negative_entries_and_duplicate_consumers_have_separate_limits() {
    let (missing, keys) = store(|_, _| None, 16);
    for pixels in [36, 72, 128] {
        for (id, key) in keys.iter().enumerate() {
            assert_eq!(
                finish(request(&missing, key, pixels, id).unwrap()),
                Err(IconError::Unavailable)
            );
            let state = missing.0.state.lock().unwrap();
            assert!(state.cache.len() <= CACHE_ENTRIES);
            assert_eq!(state.bytes, 0);
        }
    }
    assert_eq!(missing.0.state.lock().unwrap().cache.len(), CACHE_ENTRIES);

    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_gate = gate.clone();
    let (store, keys) = store(
        move |_, _| {
            let (lock, wake) = &*worker_gate;
            let _guard = wake
                .wait_while(lock.lock().unwrap(), |ready| !*ready)
                .unwrap();
            Some(vec![1; 8])
        },
        16,
    );
    let waiting: Vec<_> = (0..WAITERS)
        .map(|id| request(&store, &keys[0], 72, id).unwrap())
        .collect();
    let full = request(&store, &keys[0], 72, WAITERS).unwrap_err();
    let jobs = store.0.state.lock().unwrap().jobs.len();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    assert_eq!(full, IconError::Busy);
    assert_eq!(jobs, 1, "all consumers share one extraction");
    for reply in waiting {
        finish(reply).unwrap();
    }
}

#[test]
fn a_failed_loader_releases_capacity_and_source_changes_change_revision() {
    let (store, keys) = store(
        |path, _| {
            if path.ends_with("0") {
                panic!("injected native failure")
            }
            Some(vec![1; 8])
        },
        16,
    );
    assert_eq!(
        finish(request(&store, &keys[0], 72, 0).unwrap()),
        Err(IconError::Unavailable)
    );
    finish(request(&store, &keys[1], 72, 1).unwrap()).unwrap();
    assert_eq!(store.0.state.lock().unwrap().active, 0);

    let app = tempfile::tempdir().unwrap();
    let before = source_revision(app.path());
    std::fs::write(app.path().join("Icon\r"), b"custom Finder icon").unwrap();
    let custom = source_revision(app.path());
    assert_ne!(before, custom);
    let resources = app.path().join("Contents/Resources");
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::write(resources.join("AppIcon.icns"), b"replacement icon").unwrap();
    assert_ne!(custom, source_revision(app.path()));
}
