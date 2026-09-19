# Public release verification

Status: **Paused on 19 September 2026.** The maintainer set a $10 service-cost
limit and asked for a smaller process for this solo open source project.
See the [checkpoint](release-checkpoint.md). Existing results remain evidence
of the work already checked. The previous release gates are a reference until
the plan is reduced. No pending check is changed to a pass by this pause.

Use this record with the [release plan](release-plan.md). Both the public prerelease and stable launch include macOS, Windows, and Linux together.

Current release decision: **Not ready for public release.** Signing, updates, the download page, and release-candidate desktop checks remain incomplete.

## Status rules

- `PASS` means a check met its criteria and has linked evidence for the candidate under test.
- `FAIL` means the check ran and did not meet its criteria.
- `PENDING` means the check has not completed or has no evidence.
- `NOT APPLICABLE` requires a reason. It cannot remove one of the three required operating systems from the release.

Run each applicable check separately on each supported desktop. A workflow definition proves only that a check is configured. An older successful run does not verify a new commit. The supplied screenshot proves only its visible appearance.

Any unresolved data-loss, removal, signing/trust, or launch failure blocks stable promotion, regardless of the usability result. This includes an invalid signature, unexpected publisher, or policy block. An expected SmartScreen reputation warning remains a separate recorded observation. All other required technical checks must also pass.

## Initial evidence

Inspection date: 18 September 2026. Latest source commit observed: `bccaa245284cc22a7bc0c96d3083dace3a1950ca`. The workspace advanced from `b1c68e7` during this inspection. Keep the CI evidence for each commit separate.

| Observation                                                          | Result  | Evidence and limit                                                                                                                                                                                  |
| -------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Package configurations exist for all three systems.                  | PASS    | [Mac](../src-tauri/tauri.macos.conf.json), [Windows](../src-tauri/tauri.windows.conf.json), and [Linux](../src-tauri/tauri.linux.conf.json). This is a source check.                                |
| Automated build and installer checks are configured.                 | PASS    | [Checks](../.github/workflows/check.yml), [native checks](../.github/workflows/native.yml), and [DMG check](../scripts/ci/check-dmg.sh). This is a source check.                                    |
| The Windows runtime policy matches the configuration.                | PASS    | [Windows configuration](../src-tauri/tauri.windows.conf.json) selects `downloadBootstrapper`. This is a source check; offline and missing-runtime installation tests remain pending.                |
| Database migration rollback checks exist.                            | PASS    | [Migration code and tests](../src-tauri/src/db/migrations.rs) use a transaction, test rollback, and reject newer schemas. This does not verify installed-app backup or interruption recovery.       |
| CI for the latest observed source commit has completed successfully. | FAIL    | [Run 35324561816](https://github.com/jewei/tinydash/actions/runs/35324561816) for `bccaa245` failed its Windows Rust tests. Mac and Linux build jobs passed; native checks were skipped.            |
| Windows Rust tests passed for the previous source commit.            | FAIL    | The [Windows job](https://github.com/jewei/tinydash/actions/runs/35324129930/job/105533139693) for `b1c68e7` failed. The other build jobs were cancelled and native checks were skipped.            |
| A previous complete CI run passed.                                   | PASS    | [Run 35197297573](https://github.com/jewei/tinydash/actions/runs/35197297573) passed for `21e3d54bba6cd27fc17d819272c2aa6b1f20dae9`. This is historical evidence only.                              |
| A public release exists.                                             | PENDING | `gh release list --repo jewei/tinydash --limit 5` returned no releases.                                                                                                                             |
| Public signing, updater, and website are implemented.                | PENDING | The release workflow now has credential-backed signing and updater paths, but no protected-credential run, public candidate, or public website has been verified. GitHub's homepage field is empty. |
| The user's screenshot is stored without changes.                     | PASS    | [Mac reference image](assets/launcher-macos-light.png), 2052 by 1332 pixels. The source commit of the image was not supplied.                                                                       |

The screenshot SHA-256 is `97951412af3808fff46867135c019667bf8436070db02760fc6fce7b9bd63f9b`. It is a visual reference, not installation or runtime test evidence.

## Local implementation checks, 19 September 2026

These results cover the working tree based on `bccaa245284cc22a7bc0c96d3083dace3a1950ca`. The changes are not a signed release candidate. These local results do not change the public release decision.

| Related check | Local result                                  | Evidence and limit                                                                                                                                                                                                                                                                                                                            |
| ------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1            | PASS on the local Mac                         | The file-index test now compares resolved paths. The exact test passed locally. A Windows run for the final candidate is still required.                                                                                                                                                                                                      |
| U5            | PASS for automated database checks            | All 19 database tests passed, including three new backup checks in [backup.rs](../src-tauri/src/db/backup.rs). They cover a readable snapshot, exact settings preservation, restore reads, private Unix permissions, failed backup writes, and newer or unreadable databases. Installed-app migration and interruption checks remain pending. |
| W1, W3        | PASS for the local site                       | All 25 [site tests](../site/tests/downloads.spec.ts) and [publication tests](../site/tests/publication.spec.ts) passed. The tests include a fresh server run, downloads with JavaScript off, keyboard use, contrast, and responsive layouts. TypeScript and the static site build also passed.                                                |
| W2, L1        | PASS for the verification script's test cases | [Public download preparation](../scripts/site/release.ts) checks all three anonymous downloads against the reviewed local files. Tests reject changed bytes, a missing platform, inaccessible assets, drafts, prereleases, and an older release. These tests use simulated GitHub responses. Actual public downloads are still pending.       |
| W4            | PENDING                                       | The page uses the supplied Mac screenshot. Native Windows and Ubuntu captures and a product recording are still required.                                                                                                                                                                                                                     |

The Rust library passed `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --locked -- -D warnings`. The local path test used `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib launcher::files::tests::changing_roots_removes_old_results_and_rejects_an_unfinished_old_scan -- --exact`. No Windows test result is inferred from this Mac run.

The [download site guide](download-site.md) gives the build, preview, and publication commands. Site test images use simulated release details. The default site build has no installer links. Site hosting and deployment are pending.

The user confirmed that Mac signing is ready and Windows signing is not ready. This is a readiness report, not signature evidence. MIT, a free first release, and the public credit "TinyDash by Jewei" were recommended. The license, price, and signing publisher identity still need a recorded decision for S1.

## Windows signing setup review, 19 September 2026

The [Windows signing guide](windows-signing.md) records provider eligibility,
costs, setup steps, and required verification. The maintainer confirmed the
publisher type as an individual in Malaysia. The service and verified
certificate name remain open. Microsoft Artifact Signing does not accept this
publisher type and country under its current public signing rules. The earlier
SSL.com recommendation is withdrawn because it exceeds the maintainer's budget.
The guide now records free options. No signing service has been purchased or
configured.

The current release workflow imports a PFX file. It does not yet integrate with
a cloud signing service. The workflow owner must replace that path after
service selection. This source review does not pass B4. The signed app,
installer, installed files, timestamps, and actual Windows trust behavior still
need verification.

## Open CI issue

The Windows test `launcher::files::tests::changing_roots_removes_old_results_and_rejects_an_unfinished_old_scan` failed at line 300 of [the file-index tests](../src-tauri/src/launcher/files.rs).

The assertion compared a regular Windows path with a path that included the `\\?\` prefix. The failure also occurred in the [Windows job for `bccaa245`](https://github.com/jewei/tinydash/actions/runs/35324561816/job/105534607159). Both Windows jobs reported 106 passed tests, one failed test, and two ignored tests. Diagnose the path comparison and obtain a passing Windows run for the release commit.

## Required release checks

All checks in this section start as `PENDING`. Add a result record for each check and platform. The owner must supply the evidence before changing a result to `PASS`.

Use the publication stages below. B6 requires a published prerelease, P1 through P3 require testers, and L1 requires stable promotion. Those checks cannot run while all downloads remain in a draft release.

### Scope and source

| ID  | Pass criteria                                                                                                                                                                                                                                                           | Evidence                                                                                                   |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| S1  | Release notes name the supported OS versions, processors, package types, and Linux session limits. The license, price, and publisher are recorded.                                                                                                                      | Release notes and support matrix.                                                                          |
| S2  | All version files and the release tag agree. Every package uses the selected source commit. The application identifier remains unchanged.                                                                                                                               | Tag, source SHA, package metadata, and workflow run.                                                       |
| S3  | The updater key has a named custodian and an encrypted offline backup outside CI. A restored key signs a test file that verifies with the public key from the candidate installer. Recovery secrets remain private.                                                     | Custodian, public-key fingerprint, protected backup reference, and successful restore/sign/verify report.  |
| S4  | Loss, compromise, planned key changes, and publisher-certificate renewal have documented procedures. Test recovery with disposable keys, including manual reinstall if the old key is unavailable or untrusted. OS certificate renewal keeps the updater key unchanged. | Expiry/renewal record and recovery exercise. Record how affected access and certificates would be revoked. |

### Build and package integrity

| ID  | Pass criteria                                                                                                                                                                                                                      | Evidence                                                                       |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| B1  | Formatting, type checks, Rust checks, browser tests, all three builds, and Windows/Linux native checks pass for the selected source commit and candidate packages.                                                                 | Completed CI run and job links.                                                |
| B2  | The release contains all three installers. Their versions and processor types match their labels. Checksums match the final signed files.                                                                                          | Asset inventory, build metadata, and SHA-256 results.                          |
| B3  | The Mac app has the expected Developer ID signature. Notarization succeeds, the ticket validates, and Gatekeeper accepts a browser-downloaded copy.                                                                                | Signing, notarization, ticket, and Gatekeeper logs.                            |
| B4  | The Windows app and installer have valid signatures, the expected publisher, and trusted timestamps. Record any SmartScreen prompt separately.                                                                                     | Authenticode output and screenshots from a clean Windows desktop.              |
| B5  | A clean Ubuntu 24.04 installation resolves the package's declared libraries. Its executable, desktop entry, and icons are installed correctly.                                                                                     | APT log, package metadata, and desktop screenshot.                             |
| B6  | All three published prerelease assets download without authentication and match the final candidate checksums. The prerelease is not latest, and the stable feed remains unchanged. The preview page labels the release candidate. | Release metadata, anonymous download logs, checksums, and a stable-feed check. |

A SmartScreen reputation warning alone does not invalidate a valid publisher signature. Record its exact text and effect on installation. An invalid signature or policy block fails the applicable installation check. Do not describe signing as a guarantee of no warnings. See [Microsoft's SmartScreen guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).

### Installation and removal

| ID  | Pass criteria                                                                                                                                                                                                                 | Evidence                                                                                                       |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| I1  | A user can install and open a supplied candidate file with the preview instructions. Bun, Node.js, Rust, and a GitHub account are unnecessary. B6 checks public download access.                                              | Screen recording and OS version from each system.                                                              |
| I2  | Windows installs for a standard user. Test WebView2 present while offline, absent while online, and absent with offline or blocked downloads. Failed bootstrapper downloads show a clear error and permit a successful retry. | Logs for every runtime/network case. Confirm the page states the internet requirement when WebView2 is absent. |
| I3  | Reinstallation and replacement while TinyDash is running produce one installed version and one resident process. Required quit or restart steps are clear.                                                                    | Process check and installer logs.                                                                              |
| I4  | A second launch opens the existing window. The shortcut, focus return, app launch, copy, settings, and tray actions work.                                                                                                     | [Desktop check](desktop-checks.md) results for each system.                                                    |
| I5  | Removal deletes TinyDash executables, desktop links, and startup registration. Only documented data and backups remain. Reinstallation restores saved settings. Explicit data removal also covers managed backups.            | Uninstall, residue inspection, reinstall, and data-removal results using a disposable profile.                 |
| I6  | A conflicting global shortcut produces a clear failure or supported setup guidance. A replacement shortcut works and persists. Focus returns to the prior app. Test the documented compositor shortcut on Wayland.            | Shortcut-conflict, replacement, restart, and focus tests on each supported session.                            |

### First use and desktop behavior

| ID  | Pass criteria                                                                                                                                                         | Evidence                                                                    |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| F1  | A fresh installation shows the welcome screen and a usable shortcut. The user can launch an app, find a sample file, and copy `96` from `12 * 8`.                     | Recording from each system.                                                 |
| F2  | A fresh installation saves no clipboard history before the user chooses. Declining stops capture. Enabling capture works. Upgrades retain the user's saved choice.    | Separate fresh-install and upgrade test results.                            |
| F3  | Start at login is optional. When enabled, TinyDash starts once after login. When disabled, it does not start. Repeated changes create no duplicate startup entries.   | Login tests on each system.                                                 |
| F4  | Ubuntu X11 and GNOME Wayland pass the applicable desktop checks. Wayland instructions explain the custom shortcut, focus behavior, clipboard limits, and tray access. | Separate X11 and Wayland reports, including desktop and extension versions. |

Use [desktop-checks.md](desktop-checks.md) for the full app checks. Use test data for clipboard, file, and settings checks. Run power-transition checks only in a disposable session after saving work. Mark untested behavior as pending.

### Updates and recovery

After v1, the required upgrade source is the previous public version. Before v1, use an earlier signed test version A and the candidate B. Mark only the unavailable previous-public-version case as `NOT APPLICABLE`, with the reason. The A-to-B upgrade test is still required. An A-to-A reinstall does not prove that upgrades work.

Use the same application identifier and exact candidate bytes. Select a separate test feed through runtime configuration in disposable profiles. Record both versions, schema versions, and checksums. Populate settings, pins, sample history, and both clipboard-history choices. Verify the backup and restoration procedure before running interruption or migration-failure cases.

| ID  | Pass criteria                                                                                                                                                                                                                                                                                           | Evidence                                                                                                                                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| U1  | Mac and Windows update the previous public version to the candidate. For v1, use an earlier signed test version. Settings, pins, history, and clipboard-history choices remain. The current version shows no update to itself.                                                                          | Source and target versions, checksums, and before-and-after data checks.                                                                   |
| U2  | The update flow rejects changed files and invalid signatures. It handles offline checks, missing assets, invalid metadata, and interrupted downloads without replacing the working app.                                                                                                                 | Failure-case logs and a successful launch of the retained version.                                                                         |
| U3  | Each updater selects the correct OS and processor package. The user can defer installation. Candidate builds use the compile-time `TINYDASH_UPDATE_ENDPOINT`; public builds use the stable feed. The stable feed excludes public prereleases.                                                           | Per-system feed and update tests using the same candidate bytes.                                                                           |
| U4  | Ubuntu uses the supported manual procedure: quit, install the new .deb with APT, and reopen. The required source version follows U1. Data and preferences remain, with one package, desktop entry, startup entry if enabled, and resident process.                                                      | APT log, version and data checks, registration inventory, and process count. Instructions state that v1 has no APT update feed.            |
| U5  | Before migration, a readable backup preserves database and settings with recorded schema versions. Backup failure stops migration. Migration failure rolls back schema/version and preserves data and clipboard choice. Restore to a compatible app succeeds. Newer schemas are retained without reset. | Disk-backed migration/failure results, backup/restore checks, and existing migration unit tests. Include full disk and denied-write cases. |
| U6  | Interrupt installation during replacement and interrupt first startup separately. Recovery or documented repair restores one usable installation with retained data. It preserves the pre-migration backup and requires an explicit choice before restoring older data.                                 | Interruption-point logs, repair steps, version/data checks, and duplicate-registration/process checks for each system.                     |

Tauri requires signed updater files. Those signatures do not replace Mac or Windows publisher signing. Its Linux updater package is AppImage, not `.deb`. See the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/).

### Download page and release images

| ID  | Pass criteria                                                                                                                                                                                                                   | Evidence                                                                                |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| W1  | The main button selects the correct OS. All downloads remain available. Unknown systems and mobile browsers show manual choices. Processor support is explicit.                                                                 | Browser tests for Mac, Windows, Linux, mobile, and unknown OS values.                   |
| W2  | Before prerelease publication, staged mappings select the correct installer and version for every button. Unsupported systems receive clear instructions. B6 and L1 separately verify downloads after their publication stages. | Staged link mappings and package checks. Public download evidence belongs to B6 and L1. |
| W3  | At 375, 768, and 1440 CSS pixels, content has no horizontal overflow. Keyboard focus, links, contrast, and image descriptions are usable.                                                                                       | Browser screenshots and keyboard checks.                                                |
| W4  | The Mac reference image has a Mac caption. Windows and Linux images come from their own candidate builds. Images use sample data and match the advertised features.                                                             | Image inventory with source version and OS.                                             |
| W5  | The page states price, system limits, network use, clipboard storage, manual Ubuntu updates, the Windows runtime download requirement, backup/removal steps, and support accurately. Performance claims have measurements.      | Copy review against the candidate, update policy, and installation instructions.        |

Use [the supplied image](assets/launcher-macos-light.png) to check the intended appearance. Do not require other platforms to reproduce Mac key symbols or font rendering. Confirm the page works at the deployed GitHub Pages base path as well as in local preview.

### User verification

| ID  | Pass criteria                                                                                                                                                                                           | Evidence                                                                 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| P1  | Five users per system join the initial check. Record each OS, processor, version, and installation route. Keep all attempts and any additional tester results.                                          | Anonymous tester IDs and complete task records.                          |
| P2  | At least four of the initial five users on each system install, open, search, and remove the app without live help. Record time to first success and each failure.                                      | Raw task counts per system. Do not present a general success percentage. |
| P3  | All required technical failures are fixed and retested. No unresolved data-loss, removal, signing/trust, or launch failure remains, regardless of P2. Supported limitations appear in the instructions. | Issue links, retest evidence, and updated instructions.                  |

Four successful users out of five is a small first-use check, not an estimate of a general success rate. Keep the initial group and all additional results in the record. Do not remove failed attempts to meet the target. An unresolved critical failure from any tester blocks promotion.

Ask testers after seven days whether they still use TinyDash and why. This follow-up informs later work; it is not a release requirement. Download counts do not establish continued use.

### Publication

| ID  | Pass criteria                                                                                                                                                                                                                                | Evidence                                                                                           |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| L1  | All earlier promotion checks pass. After stable promotion, anonymous downloads return all three installers. The release ID, tag, versions, assets, signatures, and SHA-256 values match the tested prerelease. No rebuild occurred.          | Candidate and stable release inventories, before-and-after checksums, and anonymous download logs. |
| L2  | The stable website and update feed change only after L1 passes. All three systems are available before announcements. The test feed remains separate.                                                                                        | Website deployment, stable manifest, and final link checks.                                        |
| L3  | A rehearsal demonstrates stopping update promotion and recovering a usable installation with retained data. It covers the verified backup, compatible app/schema pairs, and key-incident procedures. It does not force a database downgrade. | Recovery procedure, U5/U6 reports, S3/S4 evidence, and a rehearsal on the test feed.               |

Use these stages to avoid circular requirements:

| Stage               | Checks required to proceed                                                                             | Action                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| Internal candidate  | S1 through S4, B1 through B5, I1 through I6, F1 through F4, U1 through U6, W1 through W5, and L3 pass. | Publish all three packages together as a prerelease. Leave the stable feed unchanged.                  |
| Public candidate    | B6 passes on the published prerelease.                                                                 | Give anonymous download links to the test users. Run P1 through P3.                                    |
| Stable promotion    | All earlier required checks and P1 through P3 pass, with no unresolved critical failure.               | Promote the same release by changing its prerelease/latest metadata. Preserve the tag and every asset. |
| Stable verification | L1 passes after promotion.                                                                             | Update the stable website links and update manifest, then complete L2 before announcements.            |

A required `FAIL` or `PENDING` result prevents its stage from proceeding. Additional package-manager channels do not hold the direct-download release. GitHub supports [changing prerelease status while retaining release assets](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository#editing-a-release).

## Additional installation channels

Run these checks before advertising a Homebrew or WinGet command. They start as `PENDING` and apply only when that channel is offered.

| ID  | Pass criteria                                                                                                                                            | Evidence                                            |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| C1  | The published command installs the expected signed release on a clean system. Its manifest uses the correct versioned URL, processor type, and checksum. | Package review or publication link and install log. |
| C2  | The documented update and removal commands work. App updates and package-manager updates do not cause a downgrade or remove saved data unexpectedly.     | Update, removal, and retained-data checks.          |

## Test result template

Save one completed report per candidate and platform. Attach logs and screenshots to a durable release record or issue. CI artifacts expire after 14 days, so preserve required evidence before they expire.

```text
Candidate version:
Previous version and package SHA-256:
Source commit:
Test-code commit:
Build run URL:
GitHub release ID, tag, and prerelease/stable state:
Package name and SHA-256:
Publisher identity, where applicable:
Updater public-key fingerprint, where applicable:
Database schema versions before and after:
Backup and restore evidence, without private data:
OS version and processor:
Desktop environment and X11/Wayland session:
Tester and date:

Check ID:
Result: PASS / FAIL / PENDING / NOT APPLICABLE
Steps performed:
Expected result:
Observed result:
Log or screenshot:
Issue and retest evidence:
Reason, if not applicable:
```

## Repeat the source and CI inspection

These commands inspect the baseline. They do not install or remove the app.

```sh
git rev-parse HEAD
gh release list --repo jewei/tinydash --limit 5
gh run list --repo jewei/tinydash --workflow check.yml --limit 5
gh run view 35324561816 --repo jewei/tinydash
bunx --bun --no-install prettier --check README.md docs/release-plan.md docs/release-verification.md
git diff --check
```

For a new candidate, use its own run ID. Verify downloaded build hashes with `bun scripts/ci/artifacts.ts verify <directory>`. Use the [native workflow](../.github/workflows/native.yml) for Windows and Linux installer checks on disposable runners. Preserve manual Mac and Wayland reports separately.
