use super::*;
use std::sync::mpsc;

#[test]
fn production_request_deadline_is_enabled_except_for_the_finite_selection_fixture() {
    let mut budget = SearchBudget::new(Some(7), Arc::default());
    assert!(!budget.deterministic_selection_test);
    budget.deadline = Instant::now();
    assert!(budget.stopped());
    budget.deterministic_selection_test = true;
    assert!(!budget.stopped());
    // The fixture bypasses scheduling, not cooperative cancellation.
    budget.cancelled.store(7, Ordering::Release);
    assert!(budget.is_cancelled());
    assert!(budget.stopped());
}

#[test]
fn expired_or_cancelled_requests_do_no_provider_or_pin_work() {
    for cancelled in [false, true] {
        let mut manager = SearchManager::default();
        let key = QueryPin {
            mode: SearchMode::Calculator,
            index: 0,
            text: "12 * 8".into(),
            web_keyword: None,
        }
        .key();
        manager.set_pinned(&key, SearchMode::All, true);
        let mut budget = SearchBudget::new(Some(41), Arc::default());
        if cancelled {
            budget.cancelled.store(41, Ordering::Release);
        } else {
            budget.deadline = Instant::now();
        }
        let outcome = manager
            .search_with_budget("", SearchMode::All, &budget)
            .unwrap();
        assert!(outcome.results.is_empty());
        assert!(outcome.notice.is_some());
        assert_eq!(manager.timings.calls, [0; 7]);
        assert_eq!(manager.timings.pin_attempts, 0);
        assert!(manager.issued_pins.is_empty());
    }
}

#[test]
fn cancellation_does_not_wait_for_the_search_mutex_and_targets_only_its_request() {
    let index = Arc::new(Mutex::new(()));
    let held = index.lock().unwrap();
    let cancelled = Arc::new(AtomicU64::new(0));
    let budget = SearchBudget::new(Some(41), cancelled.clone());
    let next = SearchBudget::new(Some(42), cancelled.clone());
    let (started, waiting) = mpsc::channel();
    let (done, completed) = mpsc::channel();
    let worker_index = index.clone();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        done.send(budget.lock(&worker_index).map(|_| ())).unwrap();
    });
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    cancelled.store(41, Ordering::Release);
    // Completion is received while the competing index lock is still held.
    let result = completed.recv_timeout(Duration::from_secs(5));
    drop(held);
    worker.join().unwrap();
    assert_eq!(result.unwrap().unwrap_err(), "Search canceled.");
    assert!(!next.is_cancelled());
}

#[test]
fn worker_queue_and_lock_wait_spend_the_same_request_deadline() {
    let index = Mutex::new(());
    let _held = index.lock().unwrap();
    let mut budget = SearchBudget::new(None, Arc::default());
    // Simulate a request that spent all its budget queued for the worker.
    budget.deadline = Instant::now();
    assert!(budget.lock(&index).unwrap_err().contains("too long"));
    assert!(budget.stopped());
}

#[test]
fn expensive_pins_share_one_deadline_instead_of_thirty_calculator_deadlines() {
    let mut manager = SearchManager::default();
    for index in 0..RESULT_LIMIT {
        manager.set_pinned(
            &QueryPin {
                mode: SearchMode::Calculator,
                index: 0,
                text: format!("100000000! + {index}"),
                web_keyword: None,
            }
            .key(),
            SearchMode::All,
            true,
        );
    }
    let mut budget = SearchBudget::new(None, Arc::default());
    budget.deadline = Instant::now() + Duration::from_millis(10);
    let outcome = manager
        .search_with_budget("", SearchMode::All, &budget)
        .unwrap();
    assert!(budget.stopped());
    assert!(outcome.results.is_empty());
    assert!(outcome.notice.unwrap().contains("too long"));
    assert!(manager.timings.calls[5] < RESULT_LIMIT);
    assert!(manager.issued_pins.is_empty());
}

#[test]
fn stale_cancellation_never_changes_a_previous_issued_copy_value() {
    let mut manager = SearchManager::default();
    let result = manager
        .search("12 * 8", SearchMode::Calculator)
        .unwrap()
        .results
        .remove(0);
    let cancelled = Arc::new(AtomicU64::new(11));
    let old = SearchBudget::new(Some(11), cancelled.clone());
    assert!(
        manager
            .search_with_budget("3 + 4", SearchMode::Calculator, &old)
            .unwrap()
            .results
            .is_empty()
    );
    let newest = SearchBudget::new(Some(12), cancelled);
    assert_eq!(
        manager
            .search_with_budget("3 + 4", SearchMode::Calculator, &newest)
            .unwrap()
            .results[0]
            .title,
        "7"
    );
    assert!(
        matches!(manager.resolve_action(&result.id, Action::Copy).unwrap(), ResolvedAction::Copy(value) if value == "96")
    );
}
