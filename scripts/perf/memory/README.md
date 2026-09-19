# Memory checks

These tools use a separate macOS app and profile. They do not change the installed TinyDash app.

Run `snapshot.py DIRECTORY` from the repository to copy the current source. The copy includes uncommitted files. The tool records the base commit, file hashes, and tracked changes beside the copy. It does not change the Git index.

Run `prepare.py DIRECTORY` once on that copy. It adds a local control socket, a minimal page, and measurement hooks. It uses the identifier `dev.tinydash.memoryoptimization.20260919`. Build the copy with `bun run tauri build --bundles app`.

The control directory must have a short path, such as `/tmp/td-memory`. Unix socket paths have a length limit. `run.py` creates its Swift accessibility helper there. The app profile must have the `.memory-benchmark-owner` marker. The first run creates this marker only when the profile is new. A process lock prevents two measurement jobs from using the same profile.

```sh
python3 scripts/perf/memory/run.py /path/to/source-copy \
  --label baseline --control /tmp/td-memory --output /path/to/results \
  --pages real,minimal --profiles controlled --runs 5
```

Use `--smoke --runs 1` to check setup. Smoke results are not memory measurements. The smoke workload also records serialized response length in UTF-16 code units. The historic trace field is named `bytes`; it is not a UTF-8 byte count. Measured workloads omit that extra serialization. Native latency checks disable all response tracing.

The controlled profile has no file roots, clipboard capture, or currency updates. The daily profile adds 50,000 test files, file watching, 100 clipboard entries, pins, and cached exchange rates. Its clipboard monitor reads synthetic text through the diagnostic overlay. It does not read or change the system clipboard.

Each measured state has a 30-second settling period and 31 samples at one-second intervals. Physical footprint includes the host and every helper attributed to it. RSS remains a separate field. A background sampler records workload peaks. Those peaks can miss allocations between samples.

`matrix.py ROOT` expects the `baseline2`, `p1`, `p2`, `p3a`, and `p3b` source copies below `ROOT`. It alternates build order across five fresh launches. It compares all five controlled builds, plus the original and combined builds with the daily profile. P2 and P3a omit the Settings cycle phase. Their shared workload and latency checks remain the same.

For a follow-up comparison, use `--builds baseline2,icon-final`. To repeat a disturbed
pair, use a new output directory with `--first-trial N --runs 1`. Keep the old
pair and record why it was repeated. Do not remove a run because its result is
unfavorable.

```sh
python3 scripts/perf/memory/matrix.py /path/to/builds \
  --control /tmp/td-memory --output /path/to/results
python3 scripts/perf/memory/summarize.py /path/to/results /path/to/summary.json
```

`summarize.py` keeps all valid runs and lists failed runs. It checks the application corpus, executable hashes, sample counts, and query results. It reports each run, medians, ranges, and the plan's latency budgets. It does not remove Metal or other helpers from a result.

`icon_workload.py ROOT` compares P3a and P3b with empty caches, warm caches, and enough unique images to exceed the cache budget. It reports text readiness and decoded image arrival separately. The delayed-image check verifies that text can appear first. The native checks use the real Tauri image path.

`lifecycle.py DIRECTORY` performs 20 varied Settings cycles. It checks actual window destruction, draft restoration, shortcuts, Save, imports, errors, cancellation, Discard, and scroll position. For native shortcut failure testing, make another isolated copy of the prepared source. Apply `inject_shortcut_failure.py` to that copy, build it, then pass `--inject-shortcut-failure` to `lifecycle.py`. Do not use the failure-injection build for performance measurements.

`visual.py DIRECTORY` saves a screenshot and a short recording of the test window during keyboard use and warm reopening. This is a visual check. The accessibility timing does not measure the exact time that pixels reach the display.

`repeated.py ROOT` runs ten groups of 100 queries and 20 warm reopen operations in one process per build. It saves each hidden state and its process group. This is a growth diagnostic, not a five-launch savings estimate.

`capacity.py ROOT` uses separate fixtures with 100,000 files and 500 unpinned clipboard entries. It adds 100 pins, checks the retained database rows, and measures a file refresh. Both tools use the same profile lock. Run them after the comparison and icon workloads finish.

`settings_growth.py SOURCE --control DIRECTORY --output FILE` checks 100 Settings recreations in one process. It records host and helper memory after each group of 20. This separate diagnostic checks retention seen during the first comparison runs.

`settings_allocations.py SOURCE --control DIRECTORY --output DIRECTORY` adds
native heap, VM, and allocation graph captures at zero, 20, and 40 Settings
visits. Use `--assert-bounded` with the corrected build to require at most two
native Tao windows after each group. These profiler runs are separate from the
production footprint and latency measurements.

The selected build uses small data URLs. Settings release and its Tao patch are
deferred after native close failures. The original retained Settings window is
in the selected build. `settings_stress.py SOURCE --retain-settings --cycles 1000`
checks that path; omit `--retain-settings` only for a window-release prototype.
The binary PNG trial remains separate from the selected implementation.

`record_build.py SOURCE OUTPUT` records the actual prepared source and executable
after a successful build. `archive_sources.py` and `archive_selected.py` preserve
the original comparison and the failed Settings release selection.
`archive_icons.py ROOT OUTPUT` preserves those records, then archives the final
icon source and creates two selected review patches. It checks that both patches
reproduce their complete product-file trees.

The final comparison uses `--builds baseline2,icon-final` and alternates both builds
for five fresh launches per profile after disk cleanup. Use
`summarize.py --final-comparison --candidate icon-final` for this dataset.
The earlier `final-comparison` folder belongs to the failed Settings prototype;
its baseline was measured earlier and its daily trial 2 overlapped disk cleanup.
Do not report that folder as the final selected result.

`repeated.py` and `capacity.py` accept `--builds baseline2,icon-final`.
`diagnostics.py ROOT OUTPUT --candidate icon-final` summarizes those results and
the five trials of each icon format. The older Settings growth records are
separate diagnostics of the deferred release experiment.

`archive_sources.py ROOT OUTPUT` preserves the exact measured source and the original source. It removes the diagnostic overlay in separate review copies and creates ordered product patches. It applies each patch to a temporary copy and verifies every resulting file hash. The final patch contains later test additions. It does not modify the repository index.

Keep raw runs, failed setup attempts, source manifests, patches, executable hashes, and the build logs with the report. Do not run builds, tests, profilers, or another benchmark app during memory and latency collection.

Remove completed compiler caches when no compiler is active. A measurement
needs only its `.app` bundle at the recorded path. Source archives, build
manifests, and logs preserve the evidence without retaining `target/debug`,
`target/release/deps`, or incremental compiler data. Delete obsolete test app
bundles after their last diagnostic. Do not remove the installed app.
