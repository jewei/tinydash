# Memory optimization evidence

The review candidate contains result-copy reduction and on-demand icons.
Settings release and binary icon delivery are deferred. The final comparison
shows lower controlled host memory, but no demonstrated reduction in total
application memory. All latency acceptance results are inconclusive.

`selected-summary.json` and `selected-tables.md` contain the final 20-run
comparison. `diagnostics.json` contains the icon-format, repeated-use, and
capacity checks. Both selected native diagnostics are complete. The older
`summary.json` and `comparison-tables.md` describe the earlier experiment matrix.
They are not the final comparison.

`selected-source.tar.gz` contains the prepared `icon-final` build.
`icon-final.build-measured.json` records its source and executable hashes.
Apply the two patches in `selected-patches` in numerical order to the product
files from the frozen baseline. They separate result selection from icon loading.
Each patch was applied to a temporary tree and every resulting file hash checked.
The working product also matches the measured source after removal of the
diagnostic overlay.

`measured-sources.tar.gz` preserves the original source and the first five
comparison builds. The `sources` directory records their exact revisions.
The original source includes the uncommitted files present before this work.
The base commit alone is not its complete source identity.

`deferred-settings-source.tar.gz`, `deferred-settings-patches`, and
`final.build-measured.json` preserve the former Settings-release selection.
`ordered-settings-source.tar.gz` and `ordered.build-measured.json` preserve its
second close sequence. Both sequences failed native stress checks. Their binaries
are no longer retained. Their source hashes and measured binary hashes remain.

`tao-ownership.patch` shows the single changed source file in the published Tao
crate. It removed retained native windows in the prototype, but did not establish
safe Settings release. This patch is absent from the selected product.
The older `patches` directory also includes the deferred binary-icon trial.
Do not apply those experiment patches as the final selected implementation.

Memory values use macOS physical footprint, including all attributed helpers.
Host-only RSS, per-state samples, independent launches, and profiler diagnostics
are different measurements. Keep them separate. See the implementation report
for the implemented changes, failed experiments, and measurement limits.

`raw-records.tar.gz` preserves the raw measurements and failed attempts.
`logs.tar.gz` preserves build and validation output. `tools.tar.gz` preserves
the measurement scripts. `SHA256.json` records the evidence file hashes.

All temporary compiler output, completed benchmark app bundles, and generated
file and clipboard fixtures have been removed. The isolated benchmark profile
has also been removed. The three `disk-cleanup*.json` records document cleanup.
Source archives and binary hashes remain available for review.
