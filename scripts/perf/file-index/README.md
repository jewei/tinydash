# TinyDash core allocation and search comparison

This harness compiles the checkout's actual `providers/files.rs`,
`providers/clipboard.rs`, `ranking/mod.rs`, and launcher result/query/pin types.
It does not copy the matching or ranking implementation. Tiny stubs satisfy
unrelated module references; calling the platform scanner or URL parser panics.

This is **requested Rust heap allocation**, not native application RSS or physical
footprint. Allocator metadata, fragmentation, Tauri, WebView helpers, SQLite, IPC,
filesystem scanning, and the operating system clipboard are outside its scope.

## Build and reproduce

Use Rust 1.98.1 (the project's declared version). Set these to two source checkouts:

```bash
export BASELINE_ROOT=/path/to/baseline
export CANDIDATE_ROOT=/path/to/candidate
mkdir -p bin results

for variant in baseline candidate; do
  if [ "$variant" = baseline ]; then
    source_root="$BASELINE_ROOT"
  else
    source_root="$CANDIDATE_ROOT"
  fi
  TINYDASH_SOURCE_ROOT="$source_root" CARGO_TARGET_DIR="target-$variant" \
    cargo build --locked --release --features track-allocations
  cp "target-$variant/release/tinydash-file-provider-profile" "bin/$variant-alloc"
  for corpus in ascii mixed clipboard equivalence; do
    "bin/$variant-alloc" "$corpus" > "results/$variant-$corpus-alloc.json"
  done
  TINYDASH_SOURCE_ROOT="$source_root" CARGO_TARGET_DIR="target-$variant" \
    cargo build --locked --release
  cp "target-$variant/release/tinydash-file-provider-profile" "bin/$variant-timing"
done

for block in 1 2 3 4 5; do
  if [ $((block % 2)) -eq 1 ]; then
    order='baseline candidate'
  else
    order='candidate baseline'
  fi
  for variant in $order; do
    for corpus in ascii mixed; do
      "bin/$variant-timing" "$corpus" 100 50000 \
        > "results/$variant-$corpus-timing-$block.json"
    done
  done
done
```

Run from this harness directory. On Windows, executable names have `.exe`.
The source root is explicit above; if omitted, the build expects the harness at
`scripts/perf/file-index` inside a TinyDash checkout.

## Method and output

- Fixed 50,000-file corpus: `/benchmark/Project-{i % 100}/Document-{i:05}.txt`.
  In `mixed`, every tenth name is `cafe\u{301}-東京-Document-{i:05}.txt`.
- `index_retained_bytes` includes the provider and its owned entries after index
  construction. `index_peak_bytes` is peak live requested heap during construction,
  including the input entries. Index allocation traffic excludes fixture creation.
- Query allocation snapshots follow two warmup passes. They include production
  search result construction; peaks are relative to the already-loaded index and
  warmed matcher. A realloc counts its full requested new size as allocation traffic
  and its net size change as live heap. No allocator size-class or RSS claim is made.
- Timing uses a **separate executable without the tracking allocator**. Each fresh
  process builds the index, warms all queries five times, then records 100 searches
  per query. Query order rotates each round. Timings include result construction and
  exclude dropping returned results. Each record includes all nanosecond samples.
  These five alternating blocks are descriptive; they are not a native application
  latency acceptance study or evidence of a statistically established speedup.
- File results include every ordered ID/score pair and deterministic FNV-1a
  signatures. `equivalence` checks 128 combinations of 16 queries, four limits, and
  two usage maps against composed/decomposed Unicode, CJK, emoji, ASCII controls,
  case variants, and Windows path separators. Compare complete records across builds.
- Clipboard cases call actual `entries_for_ids` and then `combine_entries`.
  The valid case joins 100 entries of 160 bytes; the oversized case rejects 100
  entries of 16,384 bytes. The resulting text/error is included for exact comparison.
  Storage facade validation and platform clipboard access are not exercised.
- Optional file arguments: `<ascii|mixed> [rounds=100] [files=50000]`. Allocation
  builds ignore `rounds`. `clipboard` requires the allocation build; `equivalence`
  works in either build. The archived raw records use defaults shown above.
