# Recorded launcher measurements

These files support the [comparison report](../../performance-comparison.md).
The `results` directory contains all nine runs, the summary, build identities, and test settings.
The JSON records retain all measured latency samples and all memory samples. Warmup samples have `warmup: true`.

The tools use Python's standard library and a small Swift program. They require macOS and Accessibility access.
`measure.py` controls the apps. `ui.swift` sends input and checks results. `summarize.py` calculates the tables.
The memory reader uses the macOS `proc_pid_rusage` interface and the system's process-responsibility function.
It was tested on macOS 27.0. Other macOS releases can change those interfaces.

To repeat the measurements, copy the three source files into an empty temporary directory.
Create a `results` directory there. Build the recorded commits with the commands in `results/method.json`.
Keep the following bundle paths relative to the temporary directory.

| App      | Bundle path                                          | Bundle identifier                           |
| -------- | ---------------------------------------------------- | ------------------------------------------- |
| TinyDash | `TinyDashPerf.app`                                   | `dev.tinydash.performance.20260919`         |
| Bopop    | `BopopPerf.app`                                      | `com.oneone.bopop.performance.20260919.dev` |
| Tinycast | `tinycast-build/Build/Products/Release/Tinycast.app` | `com.tinycast.app.performance.20260919`     |

Use `git archive` to copy each recorded commit. Do not change an active checkout to prepare these builds.
For TinyDash, merge this configuration with the normal Tauri configuration.

```json
{
  "productName": "TinyDashPerf",
  "identifier": "dev.tinydash.performance.20260919"
}
```

Build Bopop with `swift build -c release --arch arm64`.
Get its product directory with `swift build -c release --arch arm64 --show-bin-path`.
Pass that directory to `Support/assemble-app.sh`, with `Support/Info.plist` and the output bundle path.
Set its bundle identifier and bundle name in the assembled `Info.plist`.
In the temporary source copy, change the dev folder name in `Storage.swift` to `Bopop Perf 20260919`.
Keep the `.dev` suffix on the bundle identifier. Bopop uses that suffix to select its dev data folder.
This prevents access to the existing `Bopop` and `Bopop Dev` folders.

Build Tinycast with the release command in the method record. Include its clipboard helper target.
Sign each assembled app with `codesign --force --deep --sign -`.
The main executables must be arm64. Use `file` to check them.

Use the profile settings in the method record. These profiles have no clipboard data or file index.
TinyDash reads `settings.json` from its bundle identifier's Application Support directory.
Bopop and Tinycast read their own UserDefaults domains.
Create the `onboarded` marker in Tinycast's test Application Support directory.
The global shortcut must be Control+Option+Shift+Space in every test app.
Bopop stores key code `49` and modifier value `917504`.
Tinycast stores the following JSON string under `hotkey.togglePalette`.

```json
{ "combo": { "_0": { "carbonKeyCode": 49, "carbonModifiers": 6656 } } }
```

Make sure that no test app is running. Leave installed app identities separate from these test identities.
Stop unrelated builds before a new run if you need less variation in the latency samples.
Then run these commands in the temporary directory.

```sh
swiftc -O ui.swift -o ui
python3 measure.py
python3 summarize.py
```

The program starts one test app at a time through LaunchServices.
It uses a different app order in each round. It stops only the process that it started.
It checks every query result, the hidden-window state, and stable process membership during each idle interval.
A failed check stops the run and records the error. It does not remove a slow sample.
`summarize.py` reports the median and nearest-rank 95th percentile.
It does not mix warmup samples with measured samples.

The `process_discovery_ms` field is a diagnostic. It is not a startup-time measurement.
The benchmark does not time the first painted pixel or the end of a window animation.
The file-search test is separate. Its command and results are in `results/file-search.json`.

The recorded comparison used a running desktop session. Background work included an unrelated Rust build.
Use the report's limits when you interpret the timing results.
