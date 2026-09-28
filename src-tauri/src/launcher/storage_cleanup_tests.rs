use super::*;

fn fixture() -> (
    tempfile::TempDir,
    Session,
    Mutex<SearchManager>,
    rusqlite::Connection,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.sqlite3");
    let search = Mutex::new(SearchManager::default());
    let mut session = Session::default();
    session
        .initialize(
            &path,
            &directory.path().join("settings.json"),
            &search,
            Some(100),
        )
        .unwrap()
        .unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    (directory, session, search, external)
}

fn observe(session: &mut Session, search: &Mutex<SearchManager>, text: &str) -> CaptureRevision {
    session.observe_text(search, Some(text.into()), 100, 100);
    session.observed.captured.unwrap().0
}

fn saved(external: &rusqlite::Connection, id: i64) -> bool {
    external
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_history WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .unwrap()
}

fn privacy_warning(session: &Session) -> LauncherWarning {
    let warning = session
        .warning
        .lock()
        .unwrap()
        .clone()
        .expect("privacy warning");
    assert_eq!(warning.code, WarningCode::StorageUnavailable);
    assert!(
        warning
            .message
            .contains("Sensitive clipboard cleanup is pending")
    );
    warning
}

#[test]
fn automatic_cleanup_survives_busy_clear_and_unrelated_success_then_quiet_tick() {
    let (_directory, mut session, search, external) = fixture();
    let capture = observe(&mut session, &search, "synthetic upstream secret");
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    // Same observation/clear path as the real monitor, not a DB-delete-only test.
    session.cleared(&search, 101);
    assert_eq!(session.health, Health::Busy);
    assert_eq!(session.cleanup, VecDeque::from([capture]));
    assert!(session.observed.captured.is_none());
    assert!(session.observed.text.is_none());
    assert!(saved(&external, capture.id));
    let warning = privacy_warning(&session);
    assert!(warning.retryable);
    let wire = serde_json::to_value(&warning).unwrap();
    assert_eq!(wire["code"], "storageUnavailable");
    assert_eq!(wire["retryable"], true);
    assert_eq!(wire["message"], warning.message);
    external.execute_batch("ROLLBACK").unwrap();

    session.record(&search, "app:unrelated", 102);
    assert_eq!(session.health, Health::Healthy);
    assert_eq!(session.cleanup.len(), 1);
    privacy_warning(&session); // An unrelated success cannot hide the obligation.
    assert!(session.retry_cleanup(&search)); // No new OS value needed.
    assert!(session.cleanup.is_empty());
    assert!(session.warning.lock().unwrap().is_none());
    assert!(!saved(&external, capture.id));
    assert!(
        search
            .lock()
            .unwrap()
            .clipboard_entry(&format!("clipboard:{}", capture.id))
            .is_err()
    );
}

#[test]
fn blocked_cleanup_never_deletes_a_later_deduplicated_capture() {
    let (_directory, mut session, search, external) = fixture();
    let first = observe(&mut session, &search, "synthetic returning value");
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    session.cleared(&search, 101);
    external.execute_batch("ROLLBACK").unwrap();
    // Same second, same id, same created_at: only the real revision differs.
    let second = observe(&mut session, &search, "synthetic returning value");
    assert_eq!(first.id, second.id);
    assert!(second.revision > first.revision);
    privacy_warning(&session);
    session.retry_cleanup(&search);
    assert!(saved(&external, second.id));
    assert!(session.cleanup.is_empty());
    assert!(session.warning.lock().unwrap().is_none());
    assert!(
        search
            .lock()
            .unwrap()
            .clipboard_entry(&format!("clipboard:{}", second.id))
            .is_ok()
    );
    // A subsequent upstream clear targets the new capture, not the old one.
    session.cleared(&search, 102);
    assert!(!saved(&external, second.id));
}

#[test]
fn external_recapture_revision_is_checked_atomically_too() {
    let (directory, mut session, search, external) = fixture();
    let first = observe(&mut session, &search, "synthetic external recapture");
    let mut other = Database::open(&directory.path().join("state.sqlite3")).unwrap();
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    session.cleared(&search, 101);
    external.execute_batch("ROLLBACK").unwrap();
    let (_, _, second) = other
        .capture_clipboard_revision("synthetic external recapture", 99, 100)
        .unwrap();
    assert_eq!(first.id, second.id);
    session.retry_cleanup(&search);
    assert!(saved(&external, second.id));
    assert!(session.cleanup.is_empty());
}

#[test]
fn pins_added_during_contention_keep_cleanup_pending_until_all_categories_unpin() {
    let (_directory, mut session, search, external) = fixture();
    let capture = observe(&mut session, &search, "synthetic pinned secret");
    let key = format!("clipboard:{}", capture.id);
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    session.cleared(&search, 101);
    // Simulate a pin becoming durable before the next cleanup attempt.
    for category in ["all", "clipboard"] {
        external
            .execute(
                "INSERT INTO pinned_items (category, result_id) VALUES (?1, ?2)",
                rusqlite::params![category, key],
            )
            .unwrap();
    }
    external.execute_batch("COMMIT").unwrap();
    session.retry_cleanup(&search);
    assert!(saved(&external, capture.id));
    assert_eq!(session.cleanup, VecDeque::from([capture]));
    privacy_warning(&session);
    session
        .with_database(|db| db.set_pinned(&key, SearchMode::All, false))
        .unwrap();
    session.retry_cleanup(&search);
    assert!(saved(&external, capture.id));
    privacy_warning(&session);
    session
        .with_database(|db| db.set_pinned(&key, SearchMode::Clipboard, false))
        .unwrap();
    session.retry_cleanup(&search);
    assert!(!saved(&external, capture.id));
    assert!(session.cleanup.is_empty());
    assert!(session.warning.lock().unwrap().is_none());
}

#[test]
fn pinned_intents_do_not_starve_unpinned_cleanup_and_recapture_retires_old_intent() {
    let (_directory, mut session, search, external) = fixture();
    let pinned = observe(&mut session, &search, "synthetic pinned");
    session
        .with_database(|db| {
            db.set_pinned(
                &format!("clipboard:{}", pinned.id),
                SearchMode::Clipboard,
                true,
            )
        })
        .unwrap();
    session.cleared(&search, 101);
    let unpinned = observe(&mut session, &search, "synthetic unpinned");
    session.cleared(&search, 101); // Rotates the older pinned obligation.
    session.retry_cleanup(&search);
    assert!(!saved(&external, unpinned.id));
    assert!(saved(&external, pinned.id));
    let recaptured = observe(&mut session, &search, "synthetic pinned");
    assert_eq!(recaptured.id, pinned.id);
    session.retry_cleanup(&search);
    assert!(session.cleanup.is_empty());
    assert!(saved(&external, pinned.id));
}

#[test]
fn ordinary_failed_manual_deletes_and_clears_are_never_queued() {
    for deletion in [Deletion::All, Deletion::Unpinned, Deletion::Entry(1)] {
        let (_directory, mut session, search, external) = fixture();
        // Not an upstream clear; user actions must remain explicit retries.
        let capture = observe(&mut session, &search, "synthetic manual delete");
        assert_eq!(capture.id, 1);
        external.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(session.delete(&search, deletion).is_err());
        assert!(session.cleanup.is_empty());
        external.execute_batch("ROLLBACK").unwrap();
        session.record(&search, "app:unrelated", 102);
        session.retry_cleanup(&search);
        assert!(saved(&external, capture.id));
        assert!(session.warning.lock().unwrap().is_none());
    }
}

#[test]
fn successful_manual_delete_or_clear_resolves_pending_without_touching_new_capture() {
    for deletion in [Deletion::All, Deletion::Unpinned, Deletion::Entry(1)] {
        let (_directory, mut session, search, external) = fixture();
        let first = observe(&mut session, &search, "synthetic deleted then recaptured");
        external.execute_batch("BEGIN IMMEDIATE").unwrap();
        session.cleared(&search, 101);
        external.execute_batch("ROLLBACK").unwrap();
        session.delete(&search, deletion).unwrap();
        let next = observe(&mut session, &search, "synthetic deleted then recaptured");
        assert!(next.id > first.id);
        session.retry_cleanup(&search);
        assert!(saved(&external, next.id));
        assert!(session.cleanup.is_empty());
        assert!(session.warning.lock().unwrap().is_none());
    }
}

#[test]
fn cleanup_capacity_pauses_capture_and_overflow_requires_successful_full_clear() {
    let (_directory, mut session, search, external) = fixture();
    for index in 0..MAX_CLEANUP_INTENTS {
        let capture = observe(&mut session, &search, &format!("synthetic pinned {index}"));
        session
            .with_database(|db| {
                db.set_pinned(
                    &format!("clipboard:{}", capture.id),
                    SearchMode::Clipboard,
                    true,
                )
            })
            .unwrap();
        session.cleared(&search, 101);
    }
    assert_eq!(session.cleanup.len(), MAX_CLEANUP_INTENTS);
    assert!(session.capture_paused());
    assert!(
        privacy_warning(&session)
            .message
            .contains("capacity is full")
    );
    session.observe_text(&search, Some("must not be captured".into()), 102, 100);
    assert!(session.observed.captured.is_none());
    assert_eq!(
        session
            .database
            .as_ref()
            .unwrap()
            .load_clipboard()
            .unwrap()
            .len(),
        MAX_CLEANUP_INTENTS
    );

    let next = session.cleanup[0];
    session
        .with_database(|db| {
            db.set_pinned(
                &format!("clipboard:{}", next.id),
                SearchMode::Clipboard,
                false,
            )
        })
        .unwrap();
    session.retry_cleanup(&search);
    assert!(!session.capture_paused());
    // Admission resumes for new values, without replaying a skipped value.
    let admitted = observe(&mut session, &search, "new value after capacity released");
    session
        .with_database(|db| {
            db.set_pinned(
                &format!("clipboard:{}", admitted.id),
                SearchMode::Clipboard,
                true,
            )
        })
        .unwrap();
    session.cleared(&search, 103);
    assert_eq!(session.cleanup.len(), MAX_CLEANUP_INTENTS);
    // Defensive overflow: preserve an explicit obligation even if admission
    // invariants are violated, rather than silently dropping an ID.
    session.enqueue_cleanup(CaptureRevision {
        id: i64::MAX,
        revision: 1,
    });
    assert_eq!(session.cleanup.len(), MAX_CLEANUP_INTENTS);
    assert!(session.cleanup_overflow);
    assert!(!privacy_warning(&session).retryable);
    session.record(&search, "app:unrelated", 104);
    assert!(
        privacy_warning(&session)
            .message
            .contains("capacity was exceeded")
    );
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert!(session.delete(&search, Deletion::All).is_err());
    assert!(session.cleanup_overflow);
    external.execute_batch("ROLLBACK").unwrap();
    session.delete(&search, Deletion::Unpinned).unwrap();
    assert!(session.cleanup_overflow);
    session.delete(&search, Deletion::All).unwrap();
    assert!(session.cleanup.is_empty());
    assert!(!session.capture_paused());
    assert!(session.warning.lock().unwrap().is_none());
}

#[test]
fn permanent_cleanup_failure_keeps_privacy_warning_without_reopening_database() {
    let (_directory, mut session, search, external) = fixture();
    let capture = observe(&mut session, &search, "synthetic permanent failure");
    external.execute_batch("CREATE TRIGGER fail_cleanup BEFORE DELETE ON clipboard_history BEGIN SELECT RAISE(FAIL, 'synthetic failure'); END;").unwrap();
    session.cleared(&search, 101);
    assert_eq!(session.health, Health::RecoveryRequired);
    assert!(session.database.is_none());
    assert_eq!(session.cleanup, VecDeque::from([capture]));
    let warning = privacy_warning(&session);
    assert!(!warning.retryable);
    assert!(warning.message.contains("Storage needs recovery"));
    external.execute_batch("DROP TRIGGER fail_cleanup").unwrap();
    session.record(&search, "app:unrelated", 102);
    assert!(!session.retry_cleanup(&search));
    assert!(saved(&external, capture.id));
    privacy_warning(&session);
}

#[test]
fn pending_cleanup_is_session_only_and_restart_does_not_replay_old_deletes() {
    let (directory, mut session, search, external) = fixture();
    let capture = observe(&mut session, &search, "synthetic shutdown limit");
    external.execute_batch("BEGIN IMMEDIATE").unwrap();
    session.cleared(&search, 101);
    assert!(
        privacy_warning(&session)
            .message
            .contains("quitting loses pending cleanup")
    );
    drop(session);
    external.execute_batch("ROLLBACK").unwrap();
    let mut restarted = Session::default();
    restarted
        .initialize(
            &directory.path().join("state.sqlite3"),
            &directory.path().join("settings.json"),
            &search,
            Some(100),
        )
        .unwrap()
        .unwrap();
    assert!(restarted.cleanup.is_empty());
    assert!(!restarted.retry_cleanup(&search));
    assert!(saved(&external, capture.id));
}
