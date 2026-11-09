# Golden corpus

What the Rust app (v0.9.0 line, commit in `meta.jsonl`) answered for each query. The Go port must match it.

- `queries/<feature>.txt`: the inputs, one per line. Harvested from the Rust test modules and the examples in `docs/features.md`, plus hand-written cases. Add cases here; never delete one to make a test pass.
- `search.jsonl`: what `search` returned per category and query in a fixed world (see `golden_search`); file paths start with `<root>`.
- `<feature>.jsonl`: one line per query, written by `src-tauri/src/golden.rs`. A `null` or `[]` answer is a case too: the Go port must also give no answer.
- `meta.jsonl`: the fixed clock (Monday 2026-11-09 10:30), time zone (`Asia/Kuala_Lumpur`), and currency rates used.

## Frozen

The Rust app is gone from this branch, so these files no longer change. `src-tauri/src/golden.rs` wrote them; to see it, or to write them again, check out commit `9f48452`, the last one that has the Rust code, and run there:

```sh
TZ=Asia/Kuala_Lumpur GOLDEN_OUT=$PWD/testdata/golden GOLDEN_COMMIT=$(git rev-parse --short HEAD) \
  cargo test --manifest-path src-tauri/Cargo.toml --locked golden -- --ignored
```

The output is deterministic: two runs give identical files.

## Allowed differences

The port drops some features on purpose (for example fend's long tail of units). A query where Go differs from Rust must be listed in `divergences/<feature>.tsv`: one line per query, then a tab, then the reason. The first line is a header. A difference that is not listed fails the Go tests.

## Fixtures

`fixtures/` pins how the Rust app reads and writes the user's data, written by `golden_fixtures` in the same file.

- `settings.jsonl`: each input `settings.json` (or none), the settings Rust loaded from it, its warning (`<dir>` in place of the folder), and whether it copied the file to `settings.invalid.json`.
- `tinydash.db`: a database that the Rust `Store` filled by a fixed sequence of calls (see `golden_fixtures`). `store.jsonl` dumps every table of it, its `user_version`, and the clipboard hash of sample contents. The Go store must read this file and, given the same calls, write the same rows.
