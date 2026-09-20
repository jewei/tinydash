#![allow(dead_code)]

use nucleo_matcher::Matcher;
use serde::Serialize;
#[cfg(not(feature = "track-allocations"))]
use std::time::Instant;
use std::{collections::HashMap, hint::black_box};

include!(concat!(env!("OUT_DIR"), "/production.rs"));
pub use launcher::{currency, files};

// These aliases preserve the production module routes. Query parsing, scanning,
// platform APIs, Tauri, SQLite and UI code are not exercised by this harness.
mod launcher {
    pub use crate::result;
    pub mod files {
        #[derive(Debug, serde::Serialize)]
        pub struct FileStatus {
            pub total: usize,
            pub indexing: bool,
            pub warning: Option<String>,
        }
    }
    pub mod currency {
        #[derive(Debug, serde::Serialize)]
        pub struct CurrencyStatus {
            pub as_of: Option<String>,
            pub refreshing: bool,
            pub warning: Option<String>,
        }
    }
}
mod providers {
    pub mod tools {
        pub mod url_cleaner {
            pub fn is_candidate(_: &str) -> bool {
                panic!("query parsing is outside this harness")
            }
        }
    }
}
mod error {
    #[derive(Debug)]
    pub enum Error {
        FileNotFound,
        QueryTooLong,
        Io(std::io::Error),
    }
    pub type Result<T> = std::result::Result<T, Error>;
    impl From<std::io::Error> for Error {
        fn from(error: std::io::Error) -> Self {
            Self::Io(error)
        }
    }
}
mod platform {
    pub fn file_is_hidden(_: &walkdir::DirEntry) -> std::io::Result<bool> {
        panic!("filesystem scanning is outside this harness")
    }
}

#[cfg(feature = "track-allocations")]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    static BYTES: AtomicUsize = AtomicUsize::new(0);
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    pub struct Tracking;
    #[global_allocator]
    static ALLOCATOR: Tracking = Tracking;
    fn added(bytes: usize) {
        let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
        PEAK.fetch_max(live, Relaxed);
        BYTES.fetch_add(bytes, Relaxed);
        CALLS.fetch_add(1, Relaxed);
    }
    unsafe impl GlobalAlloc for Tracking {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() && ENABLED.load(Relaxed) {
                added(layout.size());
            }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() && ENABLED.load(Relaxed) {
                added(layout.size());
            }
            pointer
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            if ENABLED.load(Relaxed) {
                LIVE.fetch_sub(layout.size(), Relaxed);
            }
            unsafe { System.dealloc(pointer, layout) };
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let next = unsafe { System.realloc(pointer, layout, size) };
            if !next.is_null() && ENABLED.load(Relaxed) {
                // Realloc contributes the full requested new size to cumulative
                // traffic, but only its net change to live requested bytes.
                LIVE.fetch_sub(layout.size(), Relaxed);
                added(size);
            }
            next
        }
    }
    #[derive(Clone, Copy)]
    pub struct Snapshot {
        pub live: usize,
        pub peak: usize,
        pub bytes: usize,
        pub calls: usize,
    }
    pub fn snapshot() -> Snapshot {
        Snapshot {
            live: LIVE.load(Relaxed),
            peak: PEAK.load(Relaxed),
            bytes: BYTES.load(Relaxed),
            calls: CALLS.load(Relaxed),
        }
    }
    pub fn reset_peak() {
        PEAK.store(LIVE.load(Relaxed), Relaxed);
    }
    pub fn start() {
        LIVE.store(0, Relaxed);
        PEAK.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        CALLS.store(0, Relaxed);
        ENABLED.store(true, Relaxed);
    }
    pub fn stop() {
        ENABLED.store(false, Relaxed);
    }
}

const COUNT: usize = 50_000;
const LIMIT: usize = 30;
const QUERIES: &[&str] = &[
    "",
    "doc",
    "dcm123",
    "Project-72",
    "no-such-filename",
    "café",
    "東京",
    r"Project-72\Document",
];

fn fixture(corpus: &str, count: usize) -> Vec<file_provider::FileEntry> {
    (0..count)
        .map(|index| {
            let name = if corpus == "mixed" && index % 10 == 0 {
                format!("cafe\u{301}-東京-Document-{index:05}.txt")
            } else {
                format!("Document-{index:05}.txt")
            };
            let path = format!("/benchmark/Project-{}/{name}", index % 100);
            file_provider::FileEntry {
                id: format!("file:{path}"),
                name,
                path,
            }
        })
        .collect()
}

// Explicit algorithm makes signatures independent of randomized HashMap seeds.
fn signature(results: &[result::SearchResult]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for result in results {
        for byte in (result.id.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(result.id.bytes())
            .chain(result.score.to_le_bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn query_results(
    provider: &file_provider::FileProvider,
    matcher: &mut Matcher,
) -> Vec<serde_json::Value> {
    QUERIES.iter().map(|query| {
        let results = provider.search(query, matcher, &HashMap::new(), 0, LIMIT);
        serde_json::json!({"query": query, "signature": signature(&results), "results": results.iter().map(|result| (&result.id, result.score)).collect::<Vec<_>>()})
    }).collect()
}

#[cfg(feature = "track-allocations")]
fn allocation_run(corpus: &str, count: usize) -> serde_json::Value {
    #[derive(Serialize)]
    struct Measurement {
        query: &'static str,
        allocations: usize,
        allocated_bytes: usize,
        peak_extra_bytes: usize,
        result_live_bytes: usize,
        count: usize,
    }
    // Output bookkeeping exists before tracking so it is not provider work.
    let mut measurements = Vec::with_capacity(QUERIES.len());
    allocations::start();
    let entries = fixture(corpus, count);
    let fixture_snapshot = allocations::snapshot();
    allocations::reset_peak();
    let provider = file_provider::FileProvider::new(entries);
    let indexed = allocations::snapshot();
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let usage = HashMap::new();
    for _ in 0..2 {
        for query in QUERIES {
            black_box(provider.search(query, &mut matcher, &usage, 0, LIMIT));
        }
    }
    for &query in QUERIES {
        allocations::reset_peak();
        let before = allocations::snapshot();
        let results = provider.search(query, &mut matcher, &usage, 0, LIMIT);
        let after = allocations::snapshot();
        measurements.push(Measurement {
            query,
            allocations: after.calls - before.calls,
            allocated_bytes: after.bytes - before.bytes,
            peak_extra_bytes: after.peak - before.live,
            result_live_bytes: after.live - before.live,
            count: results.len(),
        });
        drop(results);
    }
    allocations::stop();
    serde_json::json!({
        "mode": "requested_heap_allocations", "corpus": corpus, "files": provider.len(),
        "fixture_live_bytes": fixture_snapshot.live,
        "index_retained_bytes": indexed.live,
        "index_peak_bytes": indexed.peak,
        "index_allocated_bytes": indexed.bytes - fixture_snapshot.bytes,
        "index_allocations": indexed.calls - fixture_snapshot.calls,
        "utf32_string_size": std::mem::size_of::<nucleo_matcher::Utf32String>(),
        "optional_utf32_string_size": std::mem::size_of::<Option<nucleo_matcher::Utf32String>>(),
        "queries": measurements,
        "result_equivalence": query_results(&provider, &mut matcher),
        "scope": "Production FileProvider::new/search; requested Rust heap only; excludes filesystem scan, Tauri, UI, native RSS, allocator overhead and fragmentation"
    })
}

#[cfg(not(feature = "track-allocations"))]
fn latency_run(corpus: &str, rounds: usize, count: usize) -> serde_json::Value {
    assert!(rounds > 0, "rounds must be positive");
    let entries = fixture(corpus, count);
    let started = Instant::now();
    let provider = file_provider::FileProvider::new(entries);
    let index_ns = started.elapsed().as_nanos() as u64;
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let usage = HashMap::new();
    for _ in 0..5 {
        for query in QUERIES {
            black_box(provider.search(query, &mut matcher, &usage, 0, LIMIT));
        }
    }
    let mut samples: Vec<Vec<u64>> = QUERIES.iter().map(|_| Vec::with_capacity(rounds)).collect();
    for round in 0..rounds {
        // Rotate order to avoid always giving one query the same thermal slot.
        for position in 0..QUERIES.len() {
            let index = (position + round) % QUERIES.len();
            let started = Instant::now();
            let results = black_box(provider.search(
                black_box(QUERIES[index]),
                &mut matcher,
                &usage,
                0,
                LIMIT,
            ));
            let ns = started.elapsed().as_nanos() as u64;
            black_box(&results);
            drop(results);
            samples[index].push(ns);
        }
    }
    let queries: Vec<_> = QUERIES.iter().zip(samples).map(|(query, samples)| {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        serde_json::json!({"query": query, "p50_ns": sorted[(rounds - 1) / 2], "p95_ns": sorted[(rounds * 95).div_ceil(100) - 1], "samples_ns": samples})
    }).collect();
    serde_json::json!({
        "mode": "latency_uninstrumented", "corpus": corpus, "files": provider.len(),
        "rounds": rounds, "index_ns": index_ns, "queries": queries,
        "result_equivalence": query_results(&provider, &mut matcher),
        "scope": "Production FileProvider::new/search in a separate executable without tracking allocator; synthetic fixture; excludes filesystem scan, Tauri and UI"
    })
}

fn equivalence_run() -> serde_json::Value {
    let paths = [
        "/Documents/README.md",
        "/Documents/readme.md",
        "/Documents/café.txt",
        "/Documents/cafe\u{301}.txt",
        "/東京/報告書.txt",
        "/Work/Report  2026.txt",
        "/Work/tab\tname.txt",
        "/Work/line\nname.txt",
        "/Work/crlf\r\nname.txt",
        "C:\\Work\\Report.txt",
        "/Work/İstanbul.txt",
        "/Work/emoji-🚀.txt",
    ];
    let entries = paths
        .iter()
        .map(|path| file_provider::FileEntry {
            id: format!("file:{path}"),
            path: (*path).into(),
            name: path.rsplit(['/', '\\']).next().expect("name").into(),
        })
        .collect();
    let provider = file_provider::FileProvider::new(entries);
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
    let mut records = Vec::new();
    for used in [false, true] {
        let usage = if used {
            HashMap::from([(
                "file:/Documents/readme.md".into(),
                ranking::Usage {
                    count: 10,
                    last_used_at: 1000,
                },
            )])
        } else {
            HashMap::new()
        };
        for query in [
            "",
            "read",
            "README.md",
            "café",
            "cafe\u{301}",
            "東京",
            "報告",
            "Report 2026",
            "tab name",
            "line\nname",
            "crlf\r\nname",
            "C:/Work/Report",
            r"C:\Work\Report",
            "istanbul",
            "🚀",
            "no-such-file",
        ] {
            for limit in [0, 1, 3, 30] {
                let results = provider.search(query, &mut matcher, &usage, 1000, limit);
                records.push(serde_json::json!({"query":query, "limit":limit, "usage":used, "signature":signature(&results), "results":results.iter().map(|result| (&result.id,result.score)).collect::<Vec<_>>()}));
            }
        }
    }
    serde_json::json!({"mode":"edge_equivalence", "records":records})
}

#[cfg(feature = "track-allocations")]
fn clipboard_run() -> serde_json::Value {
    let mut records = Vec::new();
    for (name, bytes) in [
        ("valid_100_entries", 160usize),
        ("oversized_100_entries", 16_384),
    ] {
        let ids: Vec<i64> = (1..=100).rev().collect();
        allocations::start();
        let provider = clipboard_provider::ClipboardProvider::new(
            (1..=100)
                .map(|id| clipboard_provider::ClipboardEntry {
                    id,
                    content: format!("{id:03}{}", "x".repeat(bytes - 3)),
                    created_at: id,
                    last_used_at: None,
                })
                .collect(),
        );
        allocations::reset_peak();
        let before = allocations::snapshot();
        let selected = provider.entries_for_ids(&ids).expect("selected entries");
        let combined = clipboard_provider::combine_entries(&selected, "\n");
        drop(selected);
        let after = allocations::snapshot();
        allocations::stop();
        records.push(serde_json::json!({
            "case":name, "selected":ids.len(), "source_bytes":bytes * ids.len(),
            "allocated_bytes":after.bytes-before.bytes, "allocations":after.calls-before.calls,
            "peak_extra_bytes":after.peak-before.live,
            "result_bytes":combined.as_ref().ok().map(String::len),
            "result":combined,
        }));
    }
    serde_json::json!({"mode":"clipboard_selection_requested_heap", "records":records,
        "scope":"Production ClipboardProvider::entries_for_ids followed by combine_entries; excludes storage facade, Tauri, platform clipboard and native RSS"})
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let corpus = args.get(1).map(String::as_str).unwrap_or("ascii");
    if corpus == "equivalence" {
        println!("{}", equivalence_run());
        return;
    }
    #[cfg(feature = "track-allocations")]
    if corpus == "clipboard" {
        println!("{}", clipboard_run());
        return;
    }
    assert!(
        matches!(corpus, "ascii" | "mixed"),
        "corpus must be ascii or mixed"
    );
    let count = args
        .get(3)
        .map(|value| value.parse().expect("file count"))
        .unwrap_or(COUNT);
    #[cfg(feature = "track-allocations")]
    let output = allocation_run(corpus, count);
    #[cfg(not(feature = "track-allocations"))]
    let output = latency_run(
        corpus,
        args.get(2)
            .map(|value| value.parse().expect("rounds"))
            .unwrap_or(100),
        count,
    );
    println!(
        "{}",
        serde_json::to_string(&output).expect("serialize result")
    );
}
