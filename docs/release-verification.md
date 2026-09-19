# Public release verification

Status: resumed on 19 September 2026. Use the existing tools and free services.
The accepted policy is Mac Developer ID signing and notarization, an unsigned
Windows preview, and Linux checksums with manual `.deb` updates. Mac and Windows
keep separate Tauri updater signatures. The [checkpoint](release-checkpoint.md)
preserves the earlier state. No pending package check becomes a pass because
its workflow exists.

Use this record with the [release plan](release-plan.md). Both the public prerelease and stable launch include macOS, Windows, and Linux together.

Current release decision: **Not ready for public release.** Installed version upgrades, key backup and recovery, public downloads, and desktop checks remain incomplete. License and price still need a decision.

## Status rules

- `PASS` means a check met its criteria and has linked evidence for the candidate under test.
- `FAIL` means the check ran and did not meet its criteria.
- `PENDING` means the check has not completed or has no evidence.
- `NOT APPLICABLE` requires a reason. It cannot remove one of the three required operating systems from the release.

Run each applicable check separately on each supported desktop. A workflow definition proves only that a check is configured. An older successful run does not verify a new commit. The supplied screenshot proves only its visible appearance.

Any unresolved data-loss, removal, update-integrity, or launch failure on a supported system blocks promotion. Mac publisher-signature or notarization failures also block it. Windows has no publisher signature by design. Record its warning and verify installation on the supported configuration. A policy block is not a successful installation; systems that prohibit unsigned software are outside this preview's support. All required technical checks must pass.

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
| W4            | PENDING for the final release page            | The page uses the supplied Mac screenshot with a Mac caption. Extra platform captures and a product recording are optional. Verify any images used for the final candidate.                                                                                                                                                                   |

The Rust library passed `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --locked -- -D warnings`. The local path test used `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib launcher::files::tests::changing_roots_removes_old_results_and_rejects_an_unfinished_old_scan -- --exact`. No Windows test result is inferred from this Mac run.

The [download site guide](download-site.md) gives the build, preview, and publication commands. Site test images use simulated release details. The default site build has no installer links. Site hosting and deployment are pending.

MIT, a free first release, and the public credit "TinyDash by Jewei" remain recommendations. The source license and price need a decision for S1. The local Mac certificate identity is recorded below. Windows has no certificate publisher in this preview.

## Signing strategy checks, 19 September 2026

The maintainer resumed this task and assigned it the release workflow and
scripts. No paid service is required. The [setup guide](release-setup.md) lists
the configuration and remaining key-recovery checks.

| Check                              | Result                      | Evidence and limit                                                                                                                                                                                                                                                                                         |
| ---------------------------------- | --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Local Mac identity                 | PASS for identity discovery | `security find-identity -v -p codesigning` lists `Developer ID Application: Jewei Mak (4L4SS26L9J)`. Its public certificate expires on 1 February 2027 at 22:12:15 UTC. No private key was exported. This is not package or notarization evidence.                                                         |
| Repository signing configuration   | PASS for CI use             | [Release run 35430862006](https://github.com/jewei/tinydash/actions/runs/35430862006) used the Apple credentials and updater key successfully. The downloaded Mac and Windows update files verify with the local public key. No private keys or passwords were displayed or copied into this record.       |
| Release configuration and metadata | PASS locally                | 26 tests passed in `tests/release-config.spec.ts` and `tests/release-artifacts.spec.ts`. They cover per-system requirements, missing updater keys, HTTPS, Developer ID, publisher labels, and empty or mismatched updater signatures. Artifact tests use temporary fixtures, not real installers.          |
| Type and shell checks              | PASS locally                | App and site TypeScript checks passed. `bash -n` passed for the changed preflight/staging scripts and the Mac verification/notarization scripts.                                                                                                                                                           |
| Workflow validation                | PASS locally                | Actionlint v1.7.12 accepted `.github/workflows/release.yml`. ShellCheck and Pyflakes were disabled because they are not installed. This does not execute the workflow.                                                                                                                                     |
| Download page                      | PASS locally                | All 25 site tests passed after adding the Windows preview warning. The Windows main download button has the warning beside it. The static site build passed and still contains no public installer links.                                                                                                  |
| Actual signing and trust           | PASS for CI package checks  | The Mac app and DMG passed Developer ID, notarization-ticket, and Gatekeeper checks. The Windows installer and extracted app passed the unsigned-preview checks. All three package sets passed metadata and checksum checks. Physical desktop trust prompts and installed version upgrades remain pending. |

The workflow no longer imports a Windows PFX. It checks that the installer and
its extracted TinyDash executable are unsigned. It also requires a matching,
nonempty updater signature file. The package and signature checks are recorded
below. B4 still needs a Windows 11 desktop report with the actual trust prompt
and installation result.

## CI checks, 19 September 2026

The release setup was merged in [PR 1](https://github.com/jewei/tinydash/pull/1).
Tag `v0.1.0` selects `9cdbf9e484541d85b4ef8fef0e3b46e5dd522456`.
The three version files agree with this tag. No GitHub Release was published.

| Check                              | Result | Evidence and limit                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Three desktop builds               | PASS   | The Mac, Windows, and Ubuntu build jobs in [run 35428796476](https://github.com/jewei/tinydash/actions/runs/35428796476) passed for `4c445b1490b7336517bf355a1ac71d8df5c75507`. The Windows path regression, Rust checks, package checks, and Linux UI tests passed. These are development packages.                                                                                                                                       |
| Native installation and app tests  | PASS   | [Run 35430680136](https://github.com/jewei/tinydash/actions/runs/35430680136) passed 25 Windows and 26 Ubuntu app checks, plus installation, same-version reinstallation, and removal. Test code `a3ead830cc8256e01cf18ce0b90ac96b811af570` used unchanged installers from `4c445b1`. These hosted-runner checks do not cover Windows security warnings, Wayland, or a version upgrade.                                                    |
| Download site                      | PASS   | [Run 35428796397](https://github.com/jewei/tinydash/actions/runs/35428796397) passed all 25 site tests and the static build. Public installer links remain disabled.                                                                                                                                                                                                                                                                       |
| Internal signing build             | PASS   | [Run 35430862006](https://github.com/jewei/tinydash/actions/runs/35430862006) built all three systems from `v0.1.0`, with `release_mode=true` and `stage_draft=false`. It passed source, package, Mac signing/notarization, and Windows unsigned-preview checks. No release was staged or published.                                                                                                                                       |
| Release package installation tests | PASS   | [Run 35431612597](https://github.com/jewei/tinydash/actions/runs/35431612597) passed 25 Windows and 26 Ubuntu app checks, plus installation, same-version reinstallation, and removal. Test code `3ca70823ae2fe853db71aca3e7e680015b47bb2e` used the exact release packages from `35430862006`, built from `9cdbf9e`. It verified retained settings and the test data marker. Version upgrades and physical desktop checks remain pending. |

The earlier native failures exposed old test selectors, an asynchronous
first-use check, overlapping clipboard samples, and a query-clear keyboard
sequence. The test now uses category buttons, the welcome screen, bounded
readiness waits, distinct samples, and the visible clear control. It retains
the clipboard consent and deletion checks. No application changes were needed
for these test corrections. The earlier Windows path assertion is resolved.

Downloaded Mac and Windows update files passed cryptographic verification with
`minisign-verify` 0.2.5, the library in the Tauri updater. Changing one byte in
memory caused rejection for each file. The generated public-key file SHA-256 is
`a159a8a49018cd9d848551ca10b34374cb3fdd4ba7e20c458344651acba6c207`.
The extracted Mac update app contains this public key and the stable update URL.
Its code signature and stapled notarization ticket also passed local checks.
These checks do not replace an installed version-upgrade or key-restoration test.

The downloaded installers have these SHA-256 values. Keep these files unchanged
when performing the remaining checks.

| Package                        | SHA-256                                                            |
| ------------------------------ | ------------------------------------------------------------------ |
| `TinyDash_0.1.0_aarch64.dmg`   | `660a85cdbd1fa8f98d17120927d6e752362be270829a23d8da1e368345da7ee7` |
| `TinyDash_0.1.0_x64-setup.exe` | `4b5f0e90e72a98c9a1996ce9493e735b2f9c090d2863ac0c6fa78cc7d0d9510d` |
| `TinyDash_0.1.0_amd64.deb`     | `1f72d0b31f327fddafcaa9277ed78a135672e96999b509798202dc31fff79682` |

## Required release checks

All checks in this section start as `PENDING`. Add a result record for each check and platform. The owner must supply the evidence before changing a result to `PASS`.

Use the publication stages below. B6 requires a published prerelease, P1 through P3 record desktop checks and available tester feedback, and L1 requires stable promotion. Those checks cannot run while all downloads remain in a draft release.

### Scope and source

| ID  | Pass criteria                                                                                                                                                                                                       | Evidence                                                                                                  |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| S1  | Release notes name the supported OS versions, processors, package types, and Linux session limits. The license, price, and publisher are recorded.                                                                  | Release notes and support matrix.                                                                         |
| S2  | All version files and the release tag agree. Every package uses the selected source commit. The application identifier remains unchanged.                                                                           | Tag, source SHA, package metadata, and workflow run.                                                      |
| S3  | The updater key has a named custodian and an encrypted offline backup outside CI. A restored key signs a test file that verifies with the public key from the candidate installer. Recovery secrets remain private. | Custodian, public-key fingerprint, protected backup reference, and successful restore/sign/verify report. |
| S4  | Loss, compromise, planned key changes, and Mac certificate renewal have documented procedures. Mac certificate renewal keeps the updater key unchanged.                                                             | Procedure in release setup, certificate expiry, and the S3 backup recovery result.                        |

### Build and package integrity

| ID  | Pass criteria                                                                                                                                                                                                                      | Evidence                                                                                                              |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| B1  | Formatting, type checks, Rust checks, browser tests, all three builds, and Windows/Linux native checks pass for the selected source commit and candidate packages.                                                                 | Completed CI run and job links.                                                                                       |
| B2  | The release contains all three installers. Their versions and processor types match their labels. Checksums match the final files after Mac signing and notarization. Publisher metadata follows the platform policy.              | Asset inventory, build metadata, and SHA-256 results.                                                                 |
| B3  | The Mac app has the expected Developer ID signature. Notarization succeeds, the ticket validates, and Gatekeeper accepts a browser-downloaded copy.                                                                                | Signing, notarization, ticket, and Gatekeeper logs.                                                                   |
| B4  | The Windows installer and extracted TinyDash executable report NotSigned. The matching updater signature is present. The page labels the unsigned preview. Record installation, launch, and the actual warning on Windows 11.      | PowerShell output and a clean Windows desktop report. Policy-blocked systems are recorded as unsupported, not passed. |
| B5  | A clean Ubuntu 24.04 installation resolves the package's declared libraries. Its executable, desktop entry, and icons are installed correctly.                                                                                     | APT log, package metadata, and desktop screenshot.                                                                    |
| B6  | All three published prerelease assets download without authentication and match the final candidate checksums. The prerelease is not latest, and the stable feed remains unchanged. The preview page labels the release candidate. | Release metadata, anonymous download logs, checksums, and a stable-feed check.                                        |

The Windows preview can show unknown-publisher or SmartScreen warnings. Record the exact prompt and installation result. Do not report a trusted publisher. A security-policy block is an unsupported configuration, not a pass. Do not disable security controls for the test. See [Microsoft's signing guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).

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

After v1, the required upgrade source is the previous public version. Before v1, use an earlier test version A and candidate B with the same updater key. A Mac candidate also needs Developer ID signing; Windows remains an unsigned preview. Mark only the unavailable previous-public-version case as `NOT APPLICABLE`, with the reason. The A-to-B upgrade test is still required. An A-to-A reinstall does not prove that upgrades work.

Use the same application identifier and exact candidate bytes. Compile a separate HTTPS test feed into the older disposable test version. Point it at the exact candidate artifacts. The candidate keeps its compiled stable endpoint. There is no runtime feed override. Record both versions, schema versions, and checksums. Populate settings, pins, sample history, and both clipboard-history choices. Verify the backup and restoration procedure before running interruption or migration-failure cases.

| ID  | Pass criteria                                                                                                                                                                                                                                                                                           | Evidence                                                                                                                                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| U1  | Mac and Windows update the previous public version to the candidate. For v1, use an older test version with the same updater key. Settings, pins, history, and clipboard choice remain. The current version does not update to itself.                                                                  | Source and target versions, checksums, and before-and-after data checks.                                                                   |
| U2  | The update flow rejects changed files and invalid signatures. It handles offline checks, missing assets, invalid metadata, and interrupted downloads without replacing the working app.                                                                                                                 | Failure-case logs and a successful launch of the retained version.                                                                         |
| U3  | Each updater selects the correct OS and processor. The user can defer installation. The older test build may use a separate compiled HTTPS feed. The exact candidate uses the stable feed, which excludes prereleases.                                                                                  | Feed and update checks for each system. Do not rebuild the candidate for promotion.                                                        |
| U4  | Ubuntu uses the supported manual procedure: quit, install the new .deb with APT, and reopen. The required source version follows U1. Data and preferences remain, with one package, desktop entry, startup entry if enabled, and resident process.                                                      | APT log, version and data checks, registration inventory, and process count. Instructions state that v1 has no APT update feed.            |
| U5  | Before migration, a readable backup preserves database and settings with recorded schema versions. Backup failure stops migration. Migration failure rolls back schema/version and preserves data and clipboard choice. Restore to a compatible app succeeds. Newer schemas are retained without reset. | Disk-backed migration/failure results, backup/restore checks, and existing migration unit tests. Include full disk and denied-write cases. |
| U6  | Interrupt installation during replacement and interrupt first startup separately. Recovery or documented repair restores one usable installation with retained data. It preserves the pre-migration backup and requires an explicit choice before restoring older data.                                 | Interruption-point logs, repair steps, version/data checks, and duplicate-registration/process checks for each system.                     |

Tauri requires signed updater files. Those signatures do not establish OS publisher trust. Mac also uses Developer ID; the Windows preview has no publisher signature. Its Linux updater package is AppImage, not `.deb`. See the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/).

### Download page and release images

| ID  | Pass criteria                                                                                                                                                                                                                   | Evidence                                                                                |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| W1  | The main button selects the correct OS. All downloads remain available. Unknown systems and mobile browsers show manual choices. Processor support is explicit.                                                                 | Browser tests for Mac, Windows, Linux, mobile, and unknown OS values.                   |
| W2  | Before prerelease publication, staged mappings select the correct installer and version for every button. Unsupported systems receive clear instructions. B6 and L1 separately verify downloads after their publication stages. | Staged link mappings and package checks. Public download evidence belongs to B6 and L1. |
| W3  | At 375, 768, and 1440 CSS pixels, content has no horizontal overflow. Keyboard focus, links, contrast, and image descriptions are usable.                                                                                       | Browser screenshots and keyboard checks.                                                |
| W4  | The supplied Mac image has a Mac caption. Any extra images identify their actual platform and contain sample data. Extra platform images and a recording are optional.                                                          | Review of the images used on the page.                                                  |
| W5  | The page states price, system limits, network use, clipboard storage, manual Ubuntu updates, the unsigned Windows preview, the WebView2 internet requirement, removal, and support accurately.                                  | Copy review against the candidate and install instructions.                             |

Use [the supplied image](assets/launcher-macos-light.png) to check the intended appearance. Do not require other platforms to reproduce Mac key symbols or font rendering. Confirm the page works at the deployed GitHub Pages base path as well as in local preview.

### User verification

| ID  | Pass criteria                                                                                                                                                                                                    | Evidence                                                                   |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| P1  | Keep one completed desktop report for each supported system. The maintainer can run these checks. Record OS, processor, version, and installation route.                                                         | Three platform reports, with X11 and Wayland coverage in the Linux report. |
| P2  | Record available tester attempts to install, launch, search, and remove the app. No quota or percentage applies. If no external testers are available, mark this optional check NOT APPLICABLE with that reason. | Task results and feedback, or the explicit reason no external check ran.   |
| P3  | Fix and retest technical failures on supported systems. Keep installation warnings and unsupported security-policy configurations in the instructions.                                                           | Issues, retest evidence, and accurate support limits.                      |

There is no tester quota or success percentage. One maintainer can complete the desktop checks. Record all external attempts when testers are available. A critical failure on a supported system must be fixed and retested before promotion.

Ask testers after seven days whether they still use TinyDash and why. This follow-up informs later work; it is not a release requirement. Download counts do not establish continued use.

### Publication

| ID  | Pass criteria                                                                                                                                                                                                                       | Evidence                                                                                           |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| L1  | All earlier promotion checks pass. After stable promotion, anonymous downloads return all three installers. The release ID, tag, versions, assets, signatures, and SHA-256 values match the tested prerelease. No rebuild occurred. | Candidate and stable release inventories, before-and-after checksums, and anonymous download logs. |
| L2  | Marking the release latest makes its existing latest.json available through the stable feed. L1 verifies that feed and all three installers. Publish the website links after L1, before announcements. Keep the test feed separate. | Release metadata, stable feed, website deployment, and final link checks.                          |
| L3  | The recovery instructions match the backup and interrupted-update checks. They explain restoring data with a compatible app and recovering a lost updater key. They do not force a database downgrade.                              | U5/U6 results and the S3/S4 recovery instructions. No separate incident exercise is required.      |

Use these stages to avoid circular requirements:

| Stage               | Checks required to proceed                                                                                                        | Action                                                                                                                        |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Internal candidate  | S1 through S4, B1 through B5, I1 through I6, F1 through F4, U1 through U6, W1 through W5, and L3 pass.                            | Publish all three packages together as a prerelease. Leave the stable feed unchanged.                                         |
| Public candidate    | B6 passes on the published prerelease.                                                                                            | Give anonymous download links to the test users. Run P1 through P3.                                                           |
| Stable promotion    | All earlier required checks, P1, and P3 pass. P2 feedback is recorded or marked optional. No unresolved critical failure remains. | Promote the same release by changing its prerelease/latest metadata. Preserve the tag and every asset, including latest.json. |
| Stable verification | L1 passes after promotion, including the stable feed.                                                                             | Update the stable website links, then complete L2 before announcements.                                                       |

A required `FAIL` or `PENDING` result prevents its stage from proceeding. P2 external feedback is optional. Complete P1 platform reports and P3 failure review. Additional package-manager channels do not hold the direct-download release. GitHub supports [changing prerelease status while retaining release assets](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository#editing-a-release).

## Additional installation channels

Run these checks before advertising a Homebrew or WinGet command. They start as `PENDING` and apply only when that channel is offered.

| ID  | Pass criteria                                                                                                                                                             | Evidence                                            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| C1  | The advertised command installs the expected release on a clean system. Its manifest uses the correct versioned URL, processor, checksum, and signing-policy description. | Package review or publication link and install log. |
| C2  | The documented update and removal commands work. App updates and package-manager updates do not cause a downgrade or remove saved data unexpectedly.                      | Update, removal, and retained-data checks.          |

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
