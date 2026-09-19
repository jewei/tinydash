# Original payload copy counter

Apply these files to the `original` tree in `measured-sources.tar.gz`. They add a test-only counter to the frozen implementation. Then run this command from `src-tauri`:

```sh
cargo test --lib record_old_payload_copy_count -- --nocapture
```

This overlay is only for the filtered diagnostic test. Do not use it for a release build. The selected search fixture in the product review patches contains the recorded original results. `files.json` records the exact counter source, and `result.log` contains its output.
