# File index and clipboard evidence, 20 September 2026

The patch reduces provider allocation. The native behavior checks passed. Three native latency comparisons are inconclusive, so the PR remains a draft. See the [full report](../../file-index-clipboard-performance.md) for the decision, results, and limits.

## Contents

| File                                               | Contents                                                                                                                                                  |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [manifest.json](manifest.json)                     | Baseline and product commits, host details, SHA-256 hashes for measured sources, executables, scripts, archives, summaries, and raw records               |
| [provider-summary.json](provider-summary.json)     | Allocation results and descriptive timing results from the second local provider run                                                                      |
| [native-summary.json](native-summary.json)         | All 56 latency comparisons and all ten memory comparisons                                                                                                 |
| [raw-results.tar.gz](raw-results.tar.gz)           | Supplied Linux records, initial local review, second provider run, two native behavior runs, 20 native measurement runs, build/test logs, and CI evidence |
| [measured-sources.tar.gz](measured-sources.tar.gz) | Exact diagnostic source overlays, native test and analysis scripts, and the provider reproduction script                                                  |
| [verify.py](verify.py)                             | Archive, raw-record, script, and reconstructed-source hash checks                                                                                         |

The baseline commit is `06974f411483f5d254d80b617d9fcbe4dc114a71`. Product commit `7784a47861a8db68ab5cfded9c431452111f8fc9` contains the five Rust changes. Later report commits do not change those product sources.

Native executable hashes:

```text
baseline   3ea8c6df7fd2ec15023dd5fb591438f774e0c20663bc928d8c1539db68a67443
candidate  e1f9a1ec940c41c1eebb5489b3096c1bddf31aa12d4b5dcb05de9a7a4b695dda
```

The archives contain sources and measurements, not executable bundles or build caches. A new compiler or build environment can produce a different executable hash. Record the new hash with new results; do not label it as the measured executable above.

## Verify and inspect

From the repository root:

```bash
python3 docs/benchmarks/2026-09-20-file-index/verify.py
```

The check needs the pinned baseline commit in local Git history. It verifies both archives, the two summaries, every raw record, the measurement scripts, and all 176 recorded source files per native build. It reconstructs source bytes from Git plus the archived overlays without changing the checkout.

To inspect the original records, extract both archives into a new temporary directory. The raw archive uses these prefixes:

- `supplied-linux/`: the original benchmark records and manifests. The supplied package did not contain its executables.
- `initial-review/`: the first local review and provider run. Timing was inconclusive and is retained.
- `provider/`: the second local provider run, with allocation and timing records.
- `native/smoke/`: separate behavior checks for both builds.
- `native/native/`: all 20 planned measurement runs, including environment observations, raw samples, result arrays, and app logs.
- `ci-linux-first-failure/`: artifacts from the first Linux native timeout. No failure screenshot was produced; the archive includes the available HTML and driver log.
- `logs/`: local validation, build, and measurement logs.

Root files include `builds.json`, `native-plan.json`, and the CI run records. The manifest maps each raw archive member to its SHA-256 hash.

To repeat the native analysis on the saved observations, extract the source archive into a new directory. Create `results/native` there, copy all JSON files from the raw archive's `native/native` directory into it, and copy the raw `builds.json` beside `summarize_native.py`. Run `python3 summarize_native.py`. Compare the resulting JSON values with the committed native summary. Whitespace can differ because the committed JSON uses repository formatting.

## Repeat the native experiment

Use macOS on Apple silicon with the project toolchain and a logged-in desktop session. The measured host used an Apple M2, macOS 27.0, Rust 1.98.1, and Bun 1.4.2. The native helper needs Accessibility access to send the test shortcut. Close other heavy workloads before measurement. Complete builds and tests first.

The commands below reconstruct the exact prepared sources. Set `repo` to this checkout. Use a short temporary path because the diagnostic control socket has a path-length limit.

```bash
repo=/path/to/tinydash
evidence="$repo/docs/benchmarks/2026-09-20-file-index"
experiment=$(mktemp -d /tmp/td-file-perf.XXXXXX)
chmod 700 "$experiment"
tar -xzf "$evidence/measured-sources.tar.gz" -C "$experiment"
mkdir -p "$experiment/control" "$experiment/results"

for variant in baseline candidate; do
  mkdir -p "$experiment/$variant" "$experiment/apps/$variant"
  git -C "$repo" archive 06974f411483f5d254d80b617d9fcbe4dc114a71 \
    | tar -xf - -C "$experiment/$variant"
  cp -R "$experiment/overlays/$variant/." "$experiment/$variant/"
  (
    cd "$experiment/$variant"
    bun install --frozen-lockfile
    CARGO_TARGET_DIR="$experiment/native-target" bun run tauri build --bundles app
  )
  cp -R "$experiment/native-target/release/bundle/macos/TinyDashFileIndexPerf.app" \
    "$experiment/apps/$variant/"
done

swiftc "$experiment/tools/docs/benchmarks/2026-09-19/ui.swift" \
  -o "$experiment/control/ui"
swiftc "$experiment/clipboard.swift" -o "$experiment/clipboard"
python3 "$experiment/native.py" prepare
python3 "$experiment/native.py" smoke
python3 "$experiment/native.py" measure
```

The source overlays already include the diagnostic setup. Do not run `prepare.py` on them again. Both builds use the isolated identifier `dev.tinydash.fileindexperf.20260920`. The runner requires an ownership marker before resetting that test profile. It creates 50,000 synthetic fixture files. It does not use the installed TinyDash database.

The smoke phase briefly uses the OS clipboard. Its helper holds the previous contents in memory and restores them if no external clipboard change occurs. The raw fields ending in `without_clipboard_write` mean that the text stayed unchanged after rejection. They do not prove a zero write count. Only synthetic clipboard text appears in the records.

The measurement phase writes its fixed order before launching the apps and refuses to overwrite results. It records five pairs for each profile. It uses real IPC, webview input events, and native shortcut events. Webview readiness is not display-paint timing. The daily profile is synthetic; it is not a human usage trial.

Before analysis of a new run, write a new `builds.json` beside `native.py`. For each of `baseline` and `candidate`, set `executable_sha256` to the SHA-256 of that variant's `apps/<variant>/TinyDashFileIndexPerf.app/Contents/MacOS/tinydash`. Record source hashes and toolchain versions with the new results. Then run `python3 "$experiment/summarize_native.py"`. The analysis verifies the executable hashes, sample counts, query sets, application catalogs, result equivalence, and process exit before it computes the intervals.

For provider-only reproduction, use the [committed benchmark](../../../scripts/perf/file-index/README.md). Its allocation tracker measures requested Rust heap bytes. It excludes native app overhead. The archived `provider/reproduce.py` records the original local paths used for the second run; the benchmark README gives portable commands.
