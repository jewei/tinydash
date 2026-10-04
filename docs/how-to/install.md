# Install TinyDash candidates and test builds

Use the prepared release path only when the maintainer gives you a specific
release candidate or public release. Use the existing Checks path for unsigned
development builds. These paths use different trust and update expectations.

## Prepared release candidate

The release workflow can build a candidate from one version tag and stage it as
a draft GitHub Release. A draft can require GitHub access and is not an
anonymous tester download. Do not treat a draft as a public release.

Before installing a prepared candidate, confirm the version, tag, source
commit, package type, processor, and release verification record supplied by
the maintainer. The signing policy differs by system:

- macOS: Developer ID signing and Apple notarization.
- Windows: an unsigned preview. Windows can warn or block installation.
- Ubuntu: a `.deb` with SHA-256 checksums and no publisher signature.

Download the package for your system and
verify its `SHA256SUMS` file. Follow the package steps below. Keep the old
installation until the update and removal checks are complete.

The repository does not currently provide a verified public candidate. Manual
update checks and installs are available only in release builds with the
compiled updater endpoint and public key. Development builds report that updates
are not configured. The Windows preview has separate Tauri update signatures;
these do not establish a trusted Windows publisher. Do not invent a download URL or bypass an operating system
trust warning. See [release preparation](https://github.com/jewei/tinydash/blob/main/docs/how-to/release.md) for signing and updater
requirements.

## Existing unsigned test builds

Open the [Checks runs](https://github.com/jewei/tinydash/actions/workflows/check.yml) and select a successful run. Download the artifact for your operating system and CPU, then extract it. Read `build.txt` to confirm the commit and version.

These test builds have no publisher signature. They are not public releases. Mac builds use a local ad-hoc signature to check bundle integrity. They do not have Developer ID signing or notarization. Windows builds do not have Authenticode signing. The operating system can show a warning or block the app. Use these builds only if you trust the source and build run. Do not disable system security controls.

| Artifact               | Package      | Tested system             |
| ---------------------- | ------------ | ------------------------- |
| `TinyDash-macOS-ARM64` | `.dmg`       | Apple silicon Mac         |
| `TinyDash-Windows-X64` | `-setup.exe` | Windows x64 with WebView2 |
| `TinyDash-Linux-X64`   | `.deb`       | Ubuntu 24.04 x64 with X11 |

The Mac bundle declares macOS 12 as its minimum version. CI tests the current hosted Mac image, not each older version. Intel Mac, Windows ARM, and other Linux package formats are not part of this build matrix. Bun, Node.js, and Rust are not required to run an installed build.

## Check the download

Download `SHA256SUMS` with the package. These hashes detect damaged or changed files. They do not replace a publisher signature. Run the command for your system from the download directory. Compare its hash with the entry for the same filename in `SHA256SUMS`. The hashes must match exactly.

On macOS:

```sh
shasum -a 256 TinyDash_0.1.3_aarch64.dmg
```

On Linux:

```sh
sha256sum TinyDash_0.1.3_amd64.deb
```

On Windows, use PowerShell:

```powershell
(Get-FileHash -Algorithm SHA256 -LiteralPath .\TinyDash_0.1.3_x64-setup.exe).Hash.ToLowerInvariant()
```

## Install or replace a build

Quit the running TinyDash process through its tray menu before installation. Hiding the window does not stop the process. The bundle identifier remains `dev.tinydash.launcher`, so installed builds use the existing settings and database. Read the version and commit in `build.txt` to identify the build. The commands below use version `0.1.3`; use your downloaded filename when installing another version.

On macOS, open the DMG and drag `TinyDash.app` to Applications. Replace the previous copy if needed. Eject the disk image, then open TinyDash from Applications. The disk image check mounts it read-only and verifies the copied app. It does not test Gatekeeper approval or notarization.

On Windows, run the file that ends in `-setup.exe`. The installer adds TinyDash to the current user's Start menu and installs to `%LOCALAPPDATA%\TinyDash`. It does not need an administrator account. If WebView2 is missing, the installer downloads Microsoft's runtime. This step needs an internet connection. CI already has WebView2; installation without WebView2 still needs a separate desktop check.

The Windows preview has no Authenticode signature. An unknown-publisher or
SmartScreen warning is possible. Smart App Control or organization policy can
block it. Such systems are not supported by this preview. Do not disable
security controls or install a custom root certificate to run TinyDash.

On Ubuntu 24.04 x64, run this command in the extracted directory:

```sh
sudo apt install ./TinyDash_0.1.3_amd64.deb
tinydash
```

APT installs the declared libraries. The package adds `/usr/bin/tinydash`, a desktop entry, and icons. It uses the system GTK 3 and WebKitGTK 4.1 libraries. Do not assume this package works on older Ubuntu versions or every Debian-based distribution. The release artifact contains the installer and its checksum.

To update Ubuntu v1, quit TinyDash, download the new `.deb`, check its SHA-256
hash, and repeat the APT install command with the new filename. This preserves
settings and data. There is no TinyDash APT repository or automatic Linux
updater in v1. SHA-256 checksums detect changed bytes; they do not authenticate
the publisher.

Press Control + Shift + Space to show or hide TinyDash. On native Wayland, assign a desktop or compositor shortcut to `tinydash`. Focus and placement depend on the compositor. See [desktop-checks.md](desktop-checks.md) before using a build for daily work.

## Remove the app

Quit TinyDash first.

- On macOS, remove `TinyDash.app` from Applications.
- On Windows, use Settings, Apps, Installed apps, TinyDash, Uninstall. Leave the option to delete application data unselected if you want to keep it.
- On Ubuntu, run `sudo apt remove tiny-dash`.

Normal removal keeps settings and saved history. Reinstallation can use them again. Saved clipboard text remains on disk until you clear it in TinyDash or remove its data directory. See the settings and storage paths in the [repository README](https://github.com/jewei/tinydash/blob/main/docs/explanation/data-and-privacy.md).

## Build and package checks

Run `bun run tauri build` on the target operating system. Tauri merges `tauri.macos.conf.json`, `tauri.windows.conf.json`, or `tauri.linux.conf.json` with the shared configuration. Output is in `src-tauri/target/release/bundle/`.

CI verifies each artifact's checksums. Mac CI mounts the DMG and checks the copied app and Applications link. Windows and Linux CI install the package, check its executable and desktop entry, reinstall it, and run the native application tests from the installed path. They then remove the package and check that test settings and user data remain. These installation scripts require disposable GitHub-hosted runners.

Installation logs are in `native-results-<OS>-<architecture>/installer.txt`. Mac disk image results are in `test-results-macOS-<architecture>/dmg-check.txt`. The native workflow can test an existing Checks run that contains these installer artifacts. Runs from before installer support contain only standalone builds.

Public distribution still needs a verified Mac signature and notarization,
update tests, and desktop checks on all three systems. Windows publisher
signing is deferred. See [release preparation](https://github.com/jewei/tinydash/blob/main/docs/how-to/release.md) for the current
signing and update policy. Mac and Windows release builds let the user check
for an update and choose when to install it.
