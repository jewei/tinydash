# Memory diagnostic records

These files support the [memory investigation](../../../memory-investigation.md). They are debug tools and measured records. Do not apply `probe.patch` to a product build.

The probe uses an archived copy of commit `bccaa245284cc22a7bc0c96d3083dace3a1950ca`. It has a separate app identifier and an empty test profile. The installed TinyDash app and the active checkout are separate from this copy.

`probe.patch` adds a local command-file reader to the test app. The reader can hide, destroy, and recreate test windows. It can also navigate the main webview to a blank page. A test command records DOM counts. A marker file disables icon loading for one test. The patch prevents automatic process exit when the last test window closes.

The command reader polls every 100 ms. Do not use this build to compare idle CPU use or wakeups. The test uses a release build without an open inspector. All measurements sum the host and its attributed helper processes through the original `measure.py` reader.

| Record                   | Purpose                                                              |
| ------------------------ | -------------------------------------------------------------------- |
| `results/method.json`    | Build identity, test settings, sample rules, and limits              |
| `results/lifecycle.json` | Hidden launcher, Settings closure, blank page, and destroyed windows |
| `results/no-icons.json`  | A fresh process with icon loading disabled                           |
| `results/growth.json`    | A fresh normal process with repeated query and reopen cycles         |
| `results/summary.json`   | Process medians, check counts, and the recorded recreation failure   |

Each stage waits 30 seconds, then takes five samples one second apart. The test verifies that all app windows are hidden. It also verifies stable process membership during the five samples. This shorter diagnostic differs from the original comparison, which took 31 idle samples per process and repeated each app three times.

The lifecycle test recorded the memory states successfully. Its final recreation check failed to find an on-screen window with an accessible input within three seconds. The file retains that error. It must not be presented as a working memory-saving feature. The runner stops its own test process in the cleanup path.

The first lifecycle run included an attributed Metal compiler helper. Later processes can have different helpers. Compare process breakdowns before attributing a change in total memory to an intervention. These are exploratory samples from a desktop session with swap in use.

To repeat the probe, use macOS, Python 3.12 or newer, Rust, Bun, and the Swift compiler. Install this repository's frontend dependencies first. Accessibility and event-posting access are also required. The prepare script requires a new temporary directory and refuses to overwrite an existing test profile.

Run from the repository root:

```sh
python3 docs/benchmarks/2026-09-19/memory/prepare.py /tmp/tinydash-memory-check
cd /tmp/tinydash-memory-check
swiftc -O ui.swift -o ui
export TINYDASH_PROBE_DIR="$(pwd)"
cd source
bun run tauri build --bundles app
cd ..
python3 probe.py lifecycle
python3 probe.py no-icons
python3 probe.py growth
```

Each command starts a fresh process. The query checks cover Safari, Terminal, and two arithmetic expressions. No result is executed and the system clipboard is unchanged. The test profile has no file roots, clipboard capture, or currency updates. The `growth` mode adds three groups of 100 measured queries and 20 measured window reopen operations, plus warmup operations, after its baseline.

The recorded build reused a cloned Cargo release cache to shorten compilation. This does not change the source or test settings. `results/method.json` records its executable hash. After testing, stop the test app before removing the temporary directory and the profile named in `profile-path.txt`.
