# Public release plan

Status: resumed on 19 September 2026 with the maintainer's approval.
TinyDash is a solo open source project. Use the existing tools and free hosting.
No new paid service is required. Services above $10 are outside scope.
The [checkpoint](release-checkpoint.md) preserves the earlier work.

Release macOS, Windows, and Linux together. Use one version and source commit.
Users must be able to download, install, launch, update, and remove the app
without developer tools. Record the results in [release verification](release-verification.md).

## Signing and support

| System                 | First package | Signing and updates                                                                                   |
| ---------------------- | ------------- | ----------------------------------------------------------------------------------------------------- |
| macOS on Apple silicon | DMG           | Developer ID signature, notarization, and stapled tickets. Tauri-signed updates.                      |
| Windows 11 x64         | NSIS EXE      | Unsigned preview. Tauri-signed updates. Windows may warn or block installation.                       |
| Ubuntu 24.04 x64       | DEB           | SHA-256 checksums. Manual replacement with a new `.deb`. No publisher signature or automatic updater. |

Use the existing Mac identity, `Developer ID Application: Jewei Mak (4L4SS26L9J)`.
The local keychain lists this identity. Notarization credentials and a signed
candidate still need verification. See [release setup](release-setup.md).

Windows publisher signing is deferred. Do not buy a certificate, add a signing
service, create a private root certificate, or add Store packaging for v1.
The download page must label Windows as an unsigned preview. Systems that
block unsigned software are not supported by this preview. Do not ask users
to disable security controls. The [Windows guide](windows-signing.md) records
future free options.

Tauri update signatures are separate from OS publisher signatures. Keep them
for Mac and Windows. For Linux, publish `SHA256SUMS`; a checksum detects changed
bytes but does not authenticate a publisher. A GPG key and APT repository can
wait until users need them.

Test each advertised macOS version; the configured minimum is not test evidence.
Test Ubuntu GNOME under X11 and Wayland. Document the Wayland shortcut and
clipboard limits. Intel Macs, Windows ARM, and other Linux distributions are
outside v1 support.

Keep the Windows WebView2 download bootstrapper. Internet access is required
when the runtime is absent. An installed runtime must permit offline installation.
Test a failed runtime download and retry. Do not bundle the large offline runtime.

## Work to complete

1. Record the source license and price. MIT and a free first release remain
   recommendations until the maintainer selects them. Set one version in
   `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
   Keep the application identifier `dev.tinydash.launcher`.
2. Configure the Mac and updater secrets using [release setup](release-setup.md).
   Save an encrypted offline backup of the updater key and its password
   separately. Verify recovery before the first public update.
3. Build all three packages from one clean tag. The release workflow checks
   the signing policy and stages a draft. Include checksums, source metadata,
   install instructions, and update files. Check the actual final packages.
4. On each supported system, test installation, launch, shortcut use, one
   search, update, retained settings/data, and removal. Check clipboard choice,
   start at login, failed updates, and migration backup/recovery. Use disposable
   data for failure tests. Keep one report per system; no tester quota applies.
5. Publish all three packages as a GitHub prerelease for public testing after
   the required internal checks. Draft assets are not anonymous downloads.
   Confirm anonymous downloads and checksums. Label the prerelease clearly
   and keep it out of the stable update feed. Invite available testers.
6. After the checks pass, promote the same release to stable. Keep its tag,
   files, signatures, and hashes unchanged. Verify anonymous downloads again,
   then publish the static download page and announce the release.

A data-loss, launch, update-integrity, or removal failure on a supported system
blocks release. An expected Windows unknown-publisher warning is a documented
limitation. A security-policy block is not a successful installation. Record
its configuration and the support limit. Hold the combined release if a
supported system fails. Fix published defects with a new version; do not
replace published files or force users to downgrade their database.

After v1, test upgrades from the previous public version. For v1, use an older
test version with the same updater key. Use the final candidate as the update
target. Compile the stable feed into the candidate. Test data preservation
and keep one verified local backup before migration. See [data recovery](data-recovery.md).

## Distribution and marketing

Host installers and checksums on GitHub Releases. Use the existing static page
on GitHub Pages or Cloudflare Pages with a free domain. No R2 storage or custom
download service is needed.

Use the supplied Mac screenshot and describe its platform correctly. Extra
screenshots and a short demo can follow. Show each package's OS, processor,
version, size, checksum, and installation limits. Keep all three downloads
available. See the [site guide](download-site.md).

Explain local clipboard storage and the optional network features. Link to
installation, removal, release notes, source code, and issue reporting. Do not
claim support or performance that the candidate checks do not establish.

Start with one release post and the download page. Ask users about installation
problems and useful features. Homebrew, WinGet, app stores, broader Linux
packaging, and further marketing automation are later work.
