# Resume performance optimization

The selected changes reduce work without replacing icon delivery or window management. They retain result rows by ID, take independent dialog snapshots, copy application icons after result selection, and stop lower-priority providers when the response is full. The category order puts applications ahead of files, clipboard, system commands, and emoji. Calculations and explicit tools retain priority.

The direct tests check these outcomes:

- A broad application query copies 30 icon payloads instead of 99. The Settings catalog copies none instead of 100.
- With 40 matching applications and 50,000 files, only the application provider runs. The other four providers do not run. Returned results match the eager reference.
- Unchanged refreshes and a narrower query retain the row and image nodes. Reordering updates the row. Open dialogs keep independent command and clipboard data.

These are work reductions. No native memory or latency improvement is claimed for the selected combined build.

Keep [draft PR #3](https://github.com/jewei/tinydash/pull/3) open for the remaining work. Its branch is `codex/search-and-icon-optimization`. It contains the on-demand icon cache, icon cancellation and notification changes, measurement tools, raw records, source snapshots, and review evidence. Its [follow-up report](https://github.com/jewei/tinydash/blob/codex/search-and-icon-optimization/docs/performance-follow-up.md) describes the tests. Its [measurement report](https://github.com/jewei/tinydash/blob/codex/search-and-icon-optimization/docs/memory-optimization-results.md) describes the earlier native results and failures.

The on-demand cache has no established total-memory benefit. Some earlier hidden-memory observations were higher. Those observations belong to the archived source snapshots, not the current main branch. Settings release failed native close tests. Binary icon transport showed no useful memory benefit. Main-window destruction did not pass the recreation check. Keep those changes out of a release until their own evidence supports them.

Start the next session here:

1. Fetch `main` and PR #3. Check their commits, working changes, and latest CI results. Preserve unrelated work. Use separate test data and an app identifier that does not replace the installed app.
2. Use the new `main` as the baseline. Check that the draft differs only in the remaining experiment and evidence. Resolve changes from main before measuring it.
3. Start with a small causal comparison. Count mounts and icon requests for unchanged visible results. Count provider calls with a full application response and a large file corpus. Record text-result latency separately from icon arrival.
4. For a memory decision, use fresh-launch blocks with randomized A/B order. Keep cold, post-first-use, post-Settings, and sustained-use states separate. Record host and all attributed helpers. Preserve high and low observations.
5. Set the useful saving, latency margins, fixed sample count, and confidence method before acceptance runs. Use launch-level uncertainty. The old rule based on the largest within-build range is withdrawn. The draft plan proposes a 10 MiB useful hidden-memory saving. Its latency margins are the greater of 2 ms or 5% for p50, and 5 ms or 10% for p95.
6. Diagnose retained JavaScript, images, and layers in a separate profiler run. Do not mix forced garbage collection with release measurements. Stop if no valuable unused allocation is found. File-index storage and direct clipboard lookup remain separate candidates.

Before merging a later experiment, rerun the affected tests and both controlled and daily-use native profiles. Do not add savings from separate experiments. Record the exact source and executable hashes. Remove completed build caches after preserving the small evidence files.
