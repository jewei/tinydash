# Release setup

Use the existing GitHub workflow and free Tauri signing tools. This setup needs
no Windows signing service. It does not publish a release.

| System  | Publisher signing                                       | Update verification             |
| ------- | ------------------------------------------------------- | ------------------------------- |
| macOS   | Developer ID, notarization, and stapled app/DMG tickets | Tauri key                       |
| Windows | None. Label the NSIS package as an unsigned preview.    | Tauri key                       |
| Ubuntu  | None. Publish SHA-256 checksums for the `.deb`.         | Manual download and replacement |

An updater signature does not establish a trusted Windows publisher. Linux
checksums do not authenticate a publisher. No self-signed certificate is needed.

## Configure Mac signing

The local keychain lists this identity:

```text
Developer ID Application: Jewei Mak (4L4SS26L9J)
```

Use this Developer ID certificate, not an Apple Development certificate.
In Keychain Access, open **My Certificates**, select this certificate with its
private key, and export it as a password-protected `.p12` file. Save it outside
the repository. Keep an encrypted backup.

The current certificate expires on 1 February 2027 at 22:12:15 UTC. Renew it
before that date. The certificate's public metadata supplied this date.

In GitHub, open **Settings**, then **Secrets and variables**, then **Actions**.
Add the repository secrets below. Do not put private values in chat, source
files, or command arguments.

All six Apple secret names are present in GitHub as of 19 September 2026.
The [internal release build](https://github.com/jewei/tinydash/actions/runs/35430862006)
used these credentials successfully for app and DMG signing and notarization.
The instructions below remain for future recovery.

| Secret                       | Value                                              |
| ---------------------------- | -------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Base64 contents of the exported `.p12` file        |
| `APPLE_CERTIFICATE_PASSWORD` | Password set during export                         |
| `APPLE_SIGNING_IDENTITY`     | `Developer ID Application: Jewei Mak (4L4SS26L9J)` |
| `APPLE_ID`                   | Apple account email used for notarization          |
| `APPLE_PASSWORD`             | An Apple app-specific password for that account    |
| `APPLE_TEAM_ID`              | `4L4SS26L9J`                                       |

The following command sends the encoded certificate directly to a GitHub secret.
Replace the example path with the exported file path. It does not print the key.

```sh
base64 < '/absolute/path/TinyDash-Developer-ID.p12' | gh secret set APPLE_CERTIFICATE --repo jewei/tinydash
```

For passwords, use the GitHub secret form or the interactive CLI prompt:

```sh
gh secret set APPLE_CERTIFICATE_PASSWORD --repo jewei/tinydash
gh secret set APPLE_PASSWORD --repo jewei/tinydash
```

Follow the [Tauri Mac signing guide](https://v2.tauri.app/distribute/sign/macos/)
for the notarization account. An installed certificate alone does not prove
that the account can notarize an app.

## Configure the free updater key

The maintainer added all three updater secrets on 19 September 2026. The
internal release build signed the Mac and Windows update files. Both downloaded
files verify with the local public key; changed files are rejected. Installed
version upgrades and offline key recovery still need checks before publication.

Use one updater key for Mac and Windows. If a key already exists, restore that
key. Do not generate a replacement for an existing public release.

For the first key, run this command in your own terminal. Choose a password
when prompted. Keep its output private. Tauri may print key material.

```sh
mkdir -p "$HOME/.config/tinydash-signing"
chmod 700 "$HOME/.config/tinydash-signing"
umask 077
bun run tauri signer generate -w "$HOME/.config/tinydash-signing/updater.key"
```

Upload the files directly and set the password through the interactive prompt:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo jewei/tinydash < "$HOME/.config/tinydash-signing/updater.key"
gh secret set TAURI_UPDATER_PUBLIC_KEY --repo jewei/tinydash < "$HOME/.config/tinydash-signing/updater.key.pub"
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo jewei/tinydash
```

`TINYDASH_UPDATE_ENDPOINT` is already set as a repository **variable**:

```text
https://github.com/jewei/tinydash/releases/latest/download/latest.json
```

This URL becomes available when a stable release contains `latest.json`.
A draft or prerelease does not supply the stable feed. Do not publish a stable
release just to test the feed. Use a separate HTTPS test feed compiled into
an older disposable test version. Keep the candidate's endpoint set to stable.
Update from that older version to the exact candidate bytes.

Jewei is the key custodian. Keep an encrypted offline copy of the private key
and public key outside CI. Store its password separately. Record the public
key's SHA-256 fingerprint and the backup location in a private maintainer note.
Restore it into a temporary private directory and verify a signed test update
with the public key in a test installation. Then remove the restored copy.
Keep only public fingerprints and pass/fail evidence in release records.

If the key is lost, restore the backup. Without it, existing clients need a
manual reinstall with a new key. If compromise is suspected, stop update
publication, remove compromised access, replace the affected key, and publish
manual recovery instructions through an account under restored control. Do not
use only the compromised key to establish trust in its replacement.

Record the Mac certificate expiry date and renew it before expiry. Mac
certificate renewal must keep the updater key unchanged. Verify the next app
and DMG with the renewed certificate. These are different keys with different
purposes. See [Tauri update signing](https://v2.tauri.app/plugin/updater/#signing-updates).

## Run the workflow

A `v<version>` tag push creates development artifacts only. These builds have
no configured updater and cannot become a draft release.

For a release candidate, use **Run workflow** with an existing version tag:

- `release_mode=true` applies the signing policy in the table above.
- `stage_draft=true` stages a draft after all three builds pass.
- `stage_draft=false` keeps packages in Actions artifacts for internal checks.

The Mac job requires Apple and updater secrets. The Windows job requires only
updater secrets. Linux requires neither. Apple credentials go only to the Mac
runner; updater credentials go only to Mac and Windows. Missing required
configuration fails the build. No Windows PFX, certificate password, or paid
service is used.

Tauri signs, notarizes, and staples the Mac app. The workflow also notarizes
and staples the DMG. It checks Developer ID signatures, tickets, and Gatekeeper.
The Windows check requires `NotSigned` for the NSIS installer and its extracted
`tinydash.exe`. It checks that the matching updater signature file is present.
`build.txt` records publisher signing separately from updater signing.

After a successful build, run **Native app checks** with its run ID and
`release_artifacts=true`. This installs, tests, reinstalls, and removes the
exact Windows and Ubuntu packages from that run. The default value, `false`,
keeps the existing development-build tests. Keep the tested build and test-code
commits in the verification record. Complete Mac and Wayland checks separately.

The workflow stages these updater pairs:

- Mac: `.app.tar.gz` and `.app.tar.gz.sig`.
- Windows: `-setup.exe` and `-setup.exe.sig`.

It creates `latest.json` with `darwin-aarch64` and `windows-x86_64` entries.
Linux `.deb` files do not enter the feed. Tauri's Linux updater uses AppImage;
v1 uses manual `.deb` updates instead. See the [Tauri updater guide](https://v2.tauri.app/plugin/updater/).

A matching signature filename is not cryptographic verification. The installed
app must accept a valid update and reject changed bytes or an invalid signature.
Complete [release verification](release-verification.md) using the actual
packages. CI configuration alone does not prove installation or publisher trust.

Draft assets are for internal checks. Use a public prerelease for anonymous
tester downloads. After verification, promote the same assets without rebuilding.
