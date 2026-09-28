//! Synthetic in-process measurements, never native input-to-paint proof.
//! Run only this ignored test in release mode; fixtures contain no user data.
use super::*;

fn percentiles(label: &str, mut values: Vec<u64>) {
    values.sort_unstable();
    let at = |percent: usize| values[(values.len() * percent).div_ceil(100).saturating_sub(1)];
    println!(
        "  phase={label} unit=us n={} min={} p50={} p95={} p99={} max={}",
        values.len(),
        values[0],
        at(50),
        at(95),
        at(99),
        values.last().unwrap()
    );
}

fn measure(
    manager: &mut SearchManager,
    label: &str,
    mode: SearchMode,
    queries: &[&str],
    samples: usize,
) {
    let mut totals = Vec::with_capacity(samples);
    let mut pins = Vec::with_capacity(samples);
    let mut providers: [Vec<u64>; 7] = Default::default();
    let mut calls = [0usize; 7];
    let mut timeouts = 0;
    let mut rows = 0;
    for index in 0..samples + 3 {
        let budget = SearchBudget::new(None, Arc::default());
        let started = Instant::now();
        let outcome = manager
            .search_with_budget(queries[index % queries.len()], mode, &budget)
            .unwrap();
        let elapsed = started.elapsed().as_micros() as u64;
        assert!(outcome.results.len() <= RESULT_LIMIT);
        if index < 3 {
            continue;
        }
        timeouts += usize::from(budget.stopped());
        rows += outcome.results.len();
        totals.push(elapsed);
        pins.push(manager.timings.pin_us);
        for provider in 0..7 {
            providers[provider].push(manager.timings.provider_us[provider]);
            calls[provider] += manager.timings.calls[provider];
        }
    }
    println!(
        "case={label} samples={samples} warmups=3 timeouts={timeouts} returned_rows={rows} provider_calls={calls:?}"
    );
    percentiles("whole-search", totals);
    percentiles("pins", pins);
    for (label, values) in [
        "apps",
        "files",
        "clipboard",
        "system",
        "emoji",
        "calculator",
        "tools",
    ]
    .into_iter()
    .zip(providers)
    {
        percentiles(label, values);
    }
}

#[test]
#[ignore = "Synthetic release percentile benchmark; run --release synthetic_search_percentiles -- --ignored --nocapture --test-threads=1"]
#[allow(clippy::assertions_on_constants)] // Guard accidental execution of this ignored benchmark in debug mode.
fn synthetic_search_percentiles() {
    assert!(!cfg!(debug_assertions), "Run this benchmark with --release");
    let samples = std::env::var("TINYDASH_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(40)
        .clamp(20, 500);
    println!(
        "synthetic-search version=1 profile=release os={} arch={} request_budget_ms={} samples={samples}; NOT native input-to-paint; no worker/IPC/lock contention in this harness",
        std::env::consts::OS,
        std::env::consts::ARCH,
        SEARCH_TIME_LIMIT.as_millis()
    );
    for count in [1_000, 50_000, 100_000] {
        let mut manager = SearchManager::default();
        let started = Instant::now();
        let files = FileProvider::new(
            (0..count)
                .map(|index| {
                    let path = format!(
                        "/synthetic/Project-{:03}/Document-{index:06}.txt",
                        index % 100
                    );
                    FileEntry {
                        id: format!("file:{path}"),
                        name: format!("Document-{index:06}.txt"),
                        path,
                        folder: false,
                    }
                })
                .collect(),
        );
        println!(
            "files={count} fixture_prepare_us={} id_lookup_memory_lower_bound_bytes={} (excludes allocator/control/load-factor overhead; not RSS)",
            started.elapsed().as_micros(),
            files.lookup_memory_lower_bound()
        );
        manager.replace_files(files);
        measure(
            &mut manager,
            &format!("files-{count}"),
            SearchMode::Files,
            &["document", "dcm123", "Project-072", "no-such-filename"],
            samples,
        );
        for index in count - RESULT_LIMIT..count {
            manager.set_pinned(
                &format!(
                    "file:/synthetic/Project-{:03}/Document-{index:06}.txt",
                    index % 100
                ),
                SearchMode::All,
                true,
            );
        }
        measure(
            &mut manager,
            &format!("file-pins-{count}"),
            SearchMode::All,
            &[""],
            samples,
        );
    }
    let mut manager = SearchManager {
        clipboard: ClipboardProvider::new(
            (1..=1000)
                .rev()
                .map(|id| ClipboardEntry {
                    id,
                    content: format!(
                        "Synthetic history {id:04}\n{}",
                        "x".repeat(crate::providers::clipboard::MAX_TEXT_BYTES - 23)
                    ),
                    created_at: id,
                    last_used_at: None,
                })
                .collect(),
        ),
        ..SearchManager::default()
    };
    assert_eq!(manager.clipboard.len(), 1000);
    for id in 1..=RESULT_LIMIT {
        manager.set_pinned(&format!("clipboard:{id}"), SearchMode::Clipboard, true);
    }
    measure(
        &mut manager,
        "clipboard-1000-near-16KiB-plus-30-old-pins",
        SearchMode::Clipboard,
        &[""],
        samples,
    );
    measure(
        &mut manager,
        "clipboard-1000-near-16KiB-matching",
        SearchMode::Clipboard,
        &["synthetic", "no-such-value"],
        samples,
    );
    drop(manager);
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
    measure(
        &mut manager,
        "30-expensive-calculator-pins-shared-deadline",
        SearchMode::All,
        &[""],
        samples,
    );
}
