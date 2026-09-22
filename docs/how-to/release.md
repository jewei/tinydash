# Prepare a release

Use this procedure after source and desktop checks pass. The release workflow can stage a draft. Publication is a separate step.

The source uses the [MIT license](../../LICENSE). The first release is free to
download and use. Keep the third-party font and word-list notices with their
material. Include the project license with a published source archive.

## Configure signing

| System  | Publisher signing                                           | Update verification        |
| ------- | ----------------------------------------------------------- | -------------------------- |
| macOS   | Developer ID, notarization, and stapled app and DMG tickets | Tauri signature            |
| Windows | Unsigned preview                                            | Tauri signature            |
| Ubuntu  | No publisher signature; provide SHA-256 checksums           | Manual package replacement |

An updater signature does not establish a trusted Windows publisher. Checksums detect changed bytes; they do not authenticate the publisher.

Configure these GitHub Actions secrets. Keep certificate exports and private keys outside the repository.

| Secret                               | Value                                                                   |
| ------------------------------------ | ----------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`                  | Base64 encoding of a password-protected Developer ID certificate export |
| `APPLE_CERTIFICATE_PASSWORD`         | Export password                                                         |
| `APPLE_SIGNING_IDENTITY`             | The full Developer ID Application identity                              |
| `APPLE_ID`                           | Apple account used for notarization                                     |
| `APPLE_PASSWORD`                     | App-specific password for that account                                  |
| `APPLE_TEAM_ID`                      | Apple developer team identifier                                         |
| `TAURI_SIGNING_PRIVATE_KEY`          | Tauri updater private key                                               |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Updater key password                                                    |
| `TAURI_UPDATER_PUBLIC_KEY`           | Matching updater public key                                             |

Set the repository variable `TINYDASH_UPDATE_ENDPOINT` to the HTTPS update feed. The stable GitHub feed is `https://github.com/jewei/tinydash/releases/latest/download/latest.json`. It requires a stable release containing that file.

Use the same updater key for Mac and Windows. Restore the existing key when one exists. A replacement key does not verify updates for installed clients that trust the previous key. Keep an encrypted backup and store its password separately. Cloud storage is suitable for the encrypted file. Record certificate expiry and key recovery instructions in private maintainer notes.

Follow the [Tauri Mac signing guide](https://v2.tauri.app/distribute/sign/macos/) and [updater signing guide](https://v2.tauri.app/plugin/updater/#signing-updates) for key setup. A Mac certificate and the updater key have different purposes. Renewing the Mac certificate must not replace the updater key.

### Back up and test the updater key

The backup must contain the private key, its password, and the matching public
key. An encrypted key file alone is not sufficient when its password is lost.
Use a separate password for the backup container. Store that password in a
password manager.

On a Mac, an [encrypted disk image](https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac)
can hold these files. Before copying it to cloud storage, open the image with
the saved backup password. Restore its contents into a temporary private
folder, sign a new test file, and verify that signature with the public key
used by the release candidate. Confirm that verification rejects a changed
test file. Then unmount the image and remove the temporary restored files.
Keep only the result and public-key fingerprint in the verification record.

If the only usable copy is in GitHub Actions secrets, use a temporary recovery
workflow to encrypt those secrets for a public recovery key generated on the
maintainer's computer. Download only the encrypted artifact and decrypt it on
that computer. Do not print secrets in workflow logs or put them in workflow
inputs. Test the recovered key, remove the recovery job and artifact, and make
the encrypted backup. Do not generate a replacement updater key to avoid this
recovery step.

After a cloud copy is complete, download it and compare its SHA-256 checksum
with the verified local file. Record the cloud location and recovery steps in
private notes. Keep private keys and passwords out of ordinary notes and
clipboard history.

## Build a candidate

1. Set the same version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. Update lockfiles as required.
2. Run [verification](verify.md), then create the matching `v<version>` tag on the tested commit.
3. Open the Release candidate workflow and select Run workflow with that tag.
4. Set `release_mode=true`. Set `stage_draft=true` to stage a draft, or `false` to retain only Actions artifacts.

A tag push alone creates development artifacts with no configured updater. It does not create a release candidate.

The Mac job signs and notarizes the app and DMG. Windows remains an unsigned preview with separate updater signatures. Linux needs neither Apple nor updater credentials. Missing required configuration fails the candidate build.

## Verify the exact packages

1. Check package checksums, `build.txt`, platform, version, source commit, and signing results.
2. Run Native app checks with the candidate build run ID and `release_artifacts=true`.
3. Complete [desktop checks](desktop-checks.md) on macOS, Windows, and the supported Linux sessions.
4. On a disposable installation of an older version, install the exact candidate through its update path. Confirm that invalid signatures and changed bytes are rejected. Use a separate HTTPS test feed for the older test build.
5. Check cancellation, network failure, data retention, recovery, reinstall, and removal. Keep the evidence in a private verification record.

For automated version-upgrade checks, run **Native app checks** again with the
same candidate build run ID and set `upgrade_tag` to its version tag. This
selects the release packages and runs a separate Windows and Linux upgrade
suite. Leave `upgrade_tag` empty for the ordinary smoke and reinstall checks.

The upgrade suite builds a disposable `0.0.0` app. Windows uses a loopback HTTPS
feed and the existing updater public key. The suite checks failed downloads,
invalid metadata, invalid signatures, changed bytes, and a valid update through
Settings. Linux uses APT to install the candidate over the older package. Both
checks compare the installed executable with the package and verify saved
settings, clipboard consent, clipboard text, pins, and usage history after
restart and removal. The fixture data and process records stay in the workflow
artifact. The temporary HTTPS certificate is removed from the Windows runner.

These hosted checks do not verify Windows 11 security prompts, macOS updates,
or a Wayland desktop. Complete those checks separately with the same packages.

Mac updates use `.app.tar.gz` with its `.sig` file. Windows updates use `-setup.exe` with its `.sig` file. The feed contains `darwin-aarch64` and `windows-x86_64`. Ubuntu uses manual `.deb` replacement and does not enter the feed.

## Publish the tested files

Check the final asset list and release notes. Draft releases can require repository access. Use a public prerelease for anonymous testing when needed. Publish the exact tested files without rebuilding them. Confirm that the stable feed and download links resolve after publication.
