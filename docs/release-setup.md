# Release setup

This document describes the draft release workflow. It does not publish a
release and it does not create signing credentials.

## Workflow use

Push a `v<version>` tag to build the three packages. The workflow checks out
that tag on all runners, checks the version files, checks the CPU architecture,
and verifies package hashes. A tag push creates Actions artifacts only.

Use **Run workflow** with an existing tag to repeat the build. Leave
`stage_draft` and `release_mode` clear for an unsigned verification build.
Unsigned builds stay in Actions artifacts.

The workflow stages a draft GitHub Release only when both inputs are set:

- `stage_draft=true`
- `release_mode=true`

The draft step needs `contents: write`. It does not mark a release as a
prerelease or stable release. Complete the checks in
[`release-verification.md`](release-verification.md) before any publication.
Release mode labels its output as a release candidate only after the local
checks and signing configuration pass. Unsigned mode keeps the `unsigned test
build` label and cannot stage a draft.

## Required release configuration

Release mode fails before the build when any required value is absent. Store
these values as protected GitHub Actions secrets. Do not print them or commit
them.

| Secret or variable                                    | Use                                                  |
| ----------------------------------------------------- | ---------------------------------------------------- |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`     | macOS certificate import                             |
| `APPLE_SIGNING_IDENTITY`                              | Developer ID identity; `-` is not accepted           |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`         | macOS notarization credentials                       |
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Base64 PFX and password for Windows Authenticode     |
| `TAURI_SIGNING_PRIVATE_KEY`                           | Tauri updater artifact signing                       |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`                  | Optional password when the updater key is encrypted  |
| `TAURI_UPDATER_PUBLIC_KEY`                            | Public key to place in the app updater configuration |
| `TINYDASH_UPDATE_ENDPOINT`                            | HTTPS URL for the stable `latest.json` feed          |

The workflow checks that these values exist. The Windows certificate is a
base64-encoded PFX. The workflow imports it into the temporary runner
certificate store, uses its thumbprint for Tauri signing, and removes it after
the build. `TINYDASH_UPDATE_ENDPOINT` is a repository variable. The endpoint should be the stable feed, for example
`https://github.com/jewei/tinydash/releases/latest/download/latest.json`.
The endpoint and public key are compiled into release builds. Missing values
make release mode fail. A value in a secret does not prove that a package was
signed or notarized; record those results in the release verification report.
If the updater private key is encrypted, store its password in
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Leave this optional secret empty for an
unencrypted key. Never print either value.

## Updater contract

Tauri requires a public key in the application updater configuration and a
private key during the build. The private key must stay outside the repository
and have an encrypted offline backup. Release builds must enable
`bundle.createUpdaterArtifacts`. The build must fail if the updater key or
endpoint is missing. The workflow enables this option only in `release_mode`;
unsigned tag checks do not create updater files.

The workflow checks publisher signatures and notarization tickets on the macOS
app and DMG. It also runs a Gatekeeper assessment on the app. It checks the bundled Windows executable
and NSIS installer for a valid Authenticode signature, the expected
certificate thumbprint, and a signing timestamp. It extracts the bundled
`tinydash.exe` from the NSIS installer and checks that file too. It records
release metadata only after these checks. These CI checks do not replace
physical installation and launch checks on clean machines. Tauri notarizes and
staples the app during `tauri build` when the Apple credentials are present.
The workflow then submits the DMG explicitly to `notarytool`, staples it, and
validates both outputs.

In Tauri CLI 2.11.4, the [DMG builder signs the disk image](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/macos/dmg/mod.rs#L179), while the [app builder runs notarization](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/macos/app.rs#L120). This requires the separate DMG step. The [Windows bundler restores the original executable after packaging](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs#L182). This is why verification extracts the executable from the installer.

The workflow creates `latest.json` from the updater artifacts. It contains a
SemVer `version` and a `platforms` object. Each platform entry contains a URL
and the literal signature text from its `.sig` file. The current feed contains
`darwin-aarch64` and `windows-x86_64`. Linux is excluded because v1 uses a
manual `.deb` update path. Do not add a Linux entry for a `.deb`: Tauri's Linux
updater expects an AppImage.

The expected Tauri v2 updater files are:

- macOS: an `.app.tar.gz` bundle and its `.sig` file;
- Windows: the NSIS setup executable and its `.sig` file;
- Linux: an AppImage and its `.sig` file.

The v1 direct-download packages remain a DMG, NSIS setup executable, and
Ubuntu 24.04 `.deb`. The Linux `.deb` is updated manually. Do not use it as a
Tauri updater asset. The release workflow refuses a draft unless the matching
macOS and Windows updater artifacts and signatures exist. A draft is still an
internal candidate. A matching `.sig` file proves only that the manifest
copied the build output. The installed application still verifies the updater
signature during download. Complete the external update check in the release
verification record before publication.

These updater details follow the [Tauri updater guide](https://v2.tauri.app/plugin/updater/).
