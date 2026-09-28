use super::*;
use std::{sync::mpsc, time::Duration};

#[test]
fn settings_readers_are_not_held_behind_a_search_contender() {
    let state = Arc::new(LauncherState::new(Settings::default(), vec![]));
    let search = state.search.lock().unwrap();
    let (started, waiting) = mpsc::channel();
    let publisher = state.clone();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        publisher.replace_settings(Settings {
            hide_on_blur: false,
            ..Settings::default()
        });
    });
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    let reader = state.clone();
    let (done, read) = mpsc::channel();
    let event_thread = std::thread::spawn(move || {
        // Window/blur/clipboard callbacks use this same synchronous snapshot.
        for _ in 0..1000 {
            assert!(reader.settings().hide_on_blur);
            std::thread::yield_now();
        }
        done.send(()).unwrap();
    });
    let responsive = read.recv_timeout(Duration::from_secs(5));
    // Release before asserting so a regression cannot leave test threads stuck.
    drop(search);
    worker.join().unwrap();
    event_thread.join().unwrap();
    responsive.expect("settings readers must finish while search is still held");
    assert!(!state.settings().hide_on_blur);
}

#[test]
fn publication_acquires_search_before_waiting_for_settings() {
    let state = Arc::new(LauncherState::new(Settings::default(), vec![]));
    // Hold the *second* lock. Once try_lock observes search held by the
    // publisher, it must have reached the settings write wait. The old order
    // can never satisfy this condition: it waits here before taking search.
    let settings = state.settings.write().unwrap();
    let publisher = state.clone();
    let worker = std::thread::spawn(move || {
        publisher.replace_settings(Settings::default());
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let acquired_search = loop {
        if state.search.try_lock().is_err() {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::yield_now();
    };
    // Always release before joining/asserting, including a regression failure.
    drop(settings);
    worker.join().unwrap();
    assert!(
        acquired_search,
        "publisher must acquire search before settings"
    );
}

#[test]
fn reordered_publication_still_rejects_scans_for_previous_settings() {
    let previous = Settings::default();
    let state = LauncherState::new(previous.clone(), vec![]);
    let files = || {
        FileProvider::new(vec![crate::providers::files::FileEntry {
            id: "file:/fixture/document".into(),
            name: "document".into(),
            path: "/fixture/document".into(),
            folder: false,
        }])
    };
    assert!(state.accept_file_scan(&previous, files()).unwrap());
    let current = Settings {
        file_search_roots: Some(vec!["/different".into()]),
        ..previous.clone()
    };
    state.replace_settings(current.clone());
    assert_eq!(state.search.lock().unwrap().file_count(), 0);
    assert!(!state.accept_file_scan(&previous, files()).unwrap());
    assert_eq!(state.search.lock().unwrap().file_count(), 0);
    assert!(state.accept_file_scan(&current, files()).unwrap());
    assert_eq!(state.search.lock().unwrap().file_count(), 1);
}
