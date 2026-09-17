# Install TinyDash test builds

Open the [Checks runs](https://github.com/jewei/tinydash/actions/workflows/check.yml) and select a successful run. Download the artifact for your operating system and CPU, then extract it. Read `build.txt` to confirm the commit and version.

These test builds have no publisher signature. They are not public releases. Mac builds use a local ad-hoc signature to check bundle integrity. They do not have Developer ID signing or notarization. Windows builds do not have Authenticode signing. The operating system can show a warning or block the app. Use these builds only if you trust the source and build run. Do not disable system security controls.

| Artifact               | Package      | Tested system             |
| ---------------------- | ------------ | ------------------------- |
| `TinyDash-macOS-ARM64` | `.dmg`       | Apple silicon Mac         |
| `TinyDash-Windows-X64` | `-setup.exe` | Windows x64 with WebView2 |
| `TinyDash-Linux-X64`   | `.deb`       | Ubuntu 24.04 x64 with X11 |

The Mac bundle declares macOS 12 as its minimum version. CI tests the current hosted Mac image, not each older version. Intel Mac, Windows ARM, and other Linux package formats are not part of this build matrix. Bun, Node.js, and Rust are not required to run an installed build.

## Check the download

Each artifact contains `SHA256SUMS`. These hashes detect damaged or changed files. They do not replace a publisher signature. Run the check from the extracted artifact directory.

On macOS:

```sh
shasum -a 256 -c SHA256SUMS
```

On Linux:

```sh
sha256sum -c SHA256SUMS
```

On Windows, use PowerShell:

```powershell
Get-Content SHA256SUMS | ForEach-Object {
    $hash, $name = $_ -split '  ', 2
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $name).Hash -ne $hash) {
        throw "Checksum mismatch: $name"
    }
}
```

## Install or replace a build

Quit the running TinyDash process through its tray menu before installation. Hiding the window does not stop the process. The bundle identifier remains `dev.tinydash.launcher`, so installed builds use the existing settings and database. The package version is currently `0.1.0`. Use the commit in `build.txt` to distinguish test builds of this version.

On macOS, open the DMG and drag `TinyDash.app` to Applications. Replace the previous copy if needed. Eject the disk image, then open TinyDash from Applications. The inner ZIP contains the same app for checks that do not need the disk image. The disk image check mounts it read-only and verifies the copied app. It does not test Gatekeeper approval or notarization.

On Windows, run the file that ends in `-setup.exe`. The installer adds TinyDash to the current user's Start menu and installs to `%LOCALAPPDATA%\TinyDash`. It does not need an administrator account. If WebView2 is missing, the installer downloads Microsoft's runtime. This step needs an internet connection. CI already has WebView2; installation without WebView2 still needs a separate desktop check. The other `.exe` in the artifact is a standalone file for diagnosis, not an installer.

On Ubuntu 24.04 x64, run this command in the extracted directory:

```sh
sudo apt install ./TinyDash_0.1.0_amd64.deb
tinydash
```

APT installs the declared libraries. The package adds `/usr/bin/tinydash`, a desktop entry, and icons. It uses the system GTK 3 and WebKitGTK 4.1 libraries. Do not assume this package works on older Ubuntu versions or every Debian-based distribution. The inner tar archive contains the standalone binary for diagnosis and needs the same system libraries.

Press Control + Shift + Space to show or hide TinyDash. On native Wayland, assign a desktop or compositor shortcut to `tinydash`. Focus and placement depend on the compositor. See [desktop-checks.md](desktop-checks.md) before using a build for daily work.

## Remove the app

Quit TinyDash first.

- On macOS, remove `TinyDash.app` from Applications.
- On Windows, use Settings, Apps, Installed apps, TinyDash, Uninstall. Leave the option to delete application data unselected if you want to keep it.
- On Ubuntu, run `sudo apt remove tiny-dash`.

Normal removal keeps settings and saved history. Reinstallation can use them again. Saved clipboard text remains on disk until you clear it in TinyDash or remove its data directory. See the settings and storage paths in the [repository README](https://github.com/jewei/tinydash#settings).

## Build and package checks

Run `bun run tauri build` on the target operating system. Tauri merges `tauri.macos.conf.json`, `tauri.windows.conf.json`, or `tauri.linux.conf.json` with the shared configuration. Output is in `src-tauri/target/release/bundle/`.

CI verifies each artifact's checksums. Mac CI mounts the DMG and checks the copied app and Applications link. Windows and Linux CI install the package, check its executable and desktop entry, reinstall it, and run the native application tests from the installed path. They then remove the package and check that test settings and user data remain. These installation scripts require disposable GitHub-hosted runners.

Installation logs are in `native-results-<OS>-<architecture>/installer.txt`. Mac disk image results are in `test-results-macOS-<architecture>/dmg-check.txt`. The native workflow can test an existing Checks run that contains these installer artifacts. Runs from before installer support contain only standalone builds.

Public distribution still needs Mac signing and notarization, Windows signing, and physical desktop checks. No automatic updater is included. Use the standard [Tauri distribution tools](https://v2.tauri.app/distribute/), [Mac signing configuration](https://v2.tauri.app/distribute/sign/macos/), and [Windows installer settings](https://v2.tauri.app/distribute/windows-installer/) when adding signing. Replace the Mac test signing identity `-` with a Developer ID identity for distribution.
