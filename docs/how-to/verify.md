# Verify a change

Run commands from the repository root. Install dependencies with `bun install --frozen-lockfile`. Install the browser once with `bunx --bun --no-install playwright install chromium`. Linux may need `--with-deps`.

## Run the fast check

```sh
bun run verify
```

This checks public file paths, local Markdown links and anchors, Prettier and Rust formatting, and TypeScript. It then runs the browser tests tagged `@smoke`. These cover welcome search, category navigation, password actions, and the repository guards.

The command selects a free loopback port. Playwright starts its own Vite process and refuses to reuse another server. Each run uses a separate browser context and output directory. The app backend is mocked in browser tests.

## Check repository paths and history

Run `bun run check:repo` to check tracked and untracked public files. Use `bun scripts/verify/repository.ts --staged` to check only the staged tree.

The separate history checker reads a ref name and commit ID from each line of standard input. To check the history of the current commit:

```sh
printf 'HEAD %s\n' "$(git rev-parse HEAD)" | bun scripts/verify/push.ts
```

This checks all commits reachable from the supplied commit, including merge changes and paths that later commits deleted. It requests separate merge diffs so the result does not depend on `log.diffMerges`. It permits the former public guide paths. Regular verification and CI check the current files; they do not run this history check.

## Run all source checks

```sh
bun run verify:full
```

This adds the frontend build, Clippy, Rust tests, and all browser tests. A warm Rust cache reduces the time. Compilation is part of the first run. The fast check does not replace these checks before a merge.

For a focused browser check with retained successful traces, use a recipe from the [feature map](../reference/features/README.md):

```sh
bun run verify:browser tests/pins.spec.ts
bun run verify:browser tests/launcher.spec.ts --grep "copies calculation results"
```

The focused wrapper selects its own port and output directory. It accepts test-file paths and `--grep` or `-g`. Use direct Playwright commands for test discovery. Direct `bun run test:ui` runs use port 1421 and do not produce the wrapper's source record. No test attaches to an existing Vite server.

Use focused tests during diagnosis. After the final relevant edit, repeat affected proofs and run `verify:full` for source changes. For each task, record expected behavior, required platforms, outcomes, and evidence paths under `.local/` or `test-results/`. A command pass does not establish behavior outside that command's coverage. Missing required evidence prevents a verified result.

## CI triggers

The Checks workflow skips branch pushes and pull requests when all changed files are Markdown files with the `.md` or `.markdown` extension, or files under `docs/`. This also skips the desktop builds and native app checks. If any other file changes, the workflow runs. The separate Verification tools workflow checks changes to the verification skill, procedures, feature map, and tools, including Markdown-only changes. It does not replace the agent exercises in [test the verification procedure](verify-verification.md).

Manual runs and tag pushes still run the checks. The Release candidate workflow also runs for version tags or manual requests. See [GitHub path filters](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushpull_requestpull_request_targetpathspaths-ignore) for the trigger rules.

## Drive the real desktop app

Use Windows or Linux X11 in a test session. Quit an existing TinyDash process first. The native suite replaces clipboard contents and clears test clipboard history. On Windows, use a separate test user with no personal TinyDash data. On Linux, the suite creates isolated application data and configuration folders, but the session clipboard is shared.

Install `tauri-driver` 2.0.6. Linux also needs `WebKitWebDriver`, `xclip`, `xdg-utils`, `desktop-file-utils`, and `scrot`, with an active X11 session. Windows needs Edge WebDriver matching WebView2. Run without administrator privileges; the CI wrapper handles its disposable elevated runner separately.

```sh
cargo install tauri-driver --version 2.0.6 --locked
bun run verify:native
```

On a separate local Windows test account, set `$env:TINYDASH_NATIVE_TEST_PROFILE = '1'` in PowerShell before the command. Run only one native suite in a desktop session, including suites from other checkouts.

By default, `verify:native` runs `bun scripts/verify/build.ts --no-bundle` before driving the app. This builds the frontend and native executable, fingerprints the source before and after the build, and records the executable hash. A build that changes its source cannot produce a valid record. The build helper uses the default release target directory and removes the previous executable before building. Its lock prevents concurrent verification builds in that checkout.

The helper accepts `--config` with a file path or inline JSON. It records inline settings and hashes settings files. A changed settings file invalidates a local build record. If cancellation leaves `src-tauri/target/verification-build.lock`, inspect its process ID and confirm that the owned build processes have stopped before removing it.

To reuse that build, set `TINYDASH_NATIVE_MANIFEST` to `src-tauri/target/verification-build.json`. A local record must match the current source and executable. Set `TINYDASH_NATIVE_BINARY` as well when selecting another executable path. An executable path alone is insufficient. The initial check rejects non-native files, missing execute permission, mismatched platforms, stale local source, and changed executable bytes.

CI builds use the same build helper and include `build.json` in the package checksums. The installer checks compare the installed executable with the extracted package after replacement. They create `installed-build.json` with the package hash, executable hash, and build source. The native wrapper uses this record without rebuilding the installed package. Its evidence records the package source and test-code source separately. Older packages without a build record need a new identified build.

The initial check verifies the platform and executable and refuses to drive an existing TinyDash process. A lock prevents two native wrapper runs in the same checkout. On cancellation, the wrapper asks the suite to stop through a process message. The suite stops pending requests and waits, stops its owned processes, then restores fixtures and settings. Repeated cancellation requests do not interrupt cleanup.

The wrapper removes `test-results/verification/native.lock` only after confirmed cleanup. A failed test can still complete cleanup. Failed or unconfirmed cleanup retains the lock and records the state in the wrapper's `result.json`. Inspect the native `owned-resources.json`, `cleanup.json`, and `cleanup-failure.txt` before removing a retained lock. If the suite was forcibly terminated, its cleanup record can be missing. Confirm that the wrapper, suite, and owned processes have stopped, and restore any remaining fixtures first. Failed Windows settings restoration retains `settings-backup.json` in the temporary fixture directory.

Run `bun run test:ui tests/native-cancellation.spec.ts` for cancellation regression tests. These run the real wrapper and suite with a dummy driver and temporary settings. They cover setup, compilation, pending requests, repeated cancellation, forced process termination, and missing or failed cleanup records. They do not replace Windows and Linux desktop checks.

The suite checks application launch markers, calculator and emoji clipboard values, clipboard history, file watching, and canceled system commands. It starts and stops its own driver and fixture processes. Native evidence is separate from browser evidence.

Use the Native app checks workflow to verify an existing CI package without rebuilding it. Set its build run ID. Use `release_artifacts=true` for a release candidate. The workflow records both the build commit and test-code commit.

An agent with GitHub access can use an existing controlled CI session. First select the Checks run for the required source commit:

```sh
gh run list --workflow check.yml --commit "$(git rev-parse HEAD)" --json databaseId,headSha,status,conclusion
```

Inspect that run and its native jobs with `gh run view RUN_ID` and wait with `gh run watch RUN_ID --exit-status`. Download its evidence with `gh run download RUN_ID --dir test-results/ci/RUN_ID`. Replace `RUN_ID` with the selected numeric ID. A local uncommitted change is not part of a CI build. Use the same source in a controlled VM or an authorized pushed revision before using CI as proof of that change.

For an explicitly selected existing package, dispatch `native.yml` with `build_run_id` and the intended test-code ref. For example, `gh workflow run native.yml --ref TEST_REF -f build_run_id=BUILD_RUN_ID -f release_artifacts=false`. Inspect the resulting run's IDs and source records before attributing its evidence to the task. The workflow checks Windows and Linux X11; it does not establish macOS or Wayland behavior.

macOS has no WebDriver adapter in this repository. Use [desktop checks](desktop-checks.md). `tests/native/focus-macos.swift` is an optional interactive focus check that needs Accessibility access; it is not an isolated replacement for the native suite. Adding an embedded macOS driver is separate work.

Tauri documents an [embedded WebDriver option](https://v2.tauri.app/develop/tests/webdriver/) for macOS. Evaluate it in an isolated test build before adopting it. Keep the driver out of release builds, disable IPC mocks, and identify the instrumented build in evidence. Start with a real calculator search, then test teardown and data isolation. This route still needs separate proof for physical shortcuts, focus return, tray actions, and installed packages. See the [plugin setup](https://webdriver.io/docs/desktop-testing/tauri/plugin-setup/) for its dependency and permissions requirements.

## Inspect the evidence

Each wrapper run writes `test-results/verification/<timestamp>-<pid>-<mode>/`. It contains numbered command logs and `result.json` with the source commit, working-tree state, source fingerprint, platform, command results, and times. The fingerprint covers tracked files, tracked deletions, and untracked files that Git does not ignore. Generated output and private evidence remain excluded by the repository's ignore rules. The wrapper checks the fingerprint again at the end and fails if the source changed during the run. Native results go in its `native/` subfolder; `build.json` identifies the executable and build source.

Fast and focused browser tests retain screenshots and traces in the run's `browser/` folder. Explicit screenshots use each test's output directory, so later runs and retries cannot replace earlier evidence. A trace records actions and their results. Full browser runs retain traces on failure; use a focused run to retain a successful trace for each changed user journey. Open a trace with:

```sh
bunx --bun --no-install playwright show-trace path/to/trace.zip
```

Check the user action, the resulting state, and relevant side effects. A browser pass does not prove OS integration. A native copy check needs the exact clipboard value. A launch check needs the marker from the selected executable. A canceled power dialog must not execute the power action.

Playwright and the native suite stop the processes they own. Cleanup retains the run directory. Keep local evidence private. CI uploads only results from its synthetic test environment with a short retention period.

Update the feature page when an entry point, expected result, test, or platform limit changes.
