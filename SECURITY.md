# Security

## Supported versions

Only the latest release gets security fixes. On macOS and Windows, TinyDash updates itself; on Linux, download the latest release.

## Report a vulnerability

Do not open a public issue. Report it privately on GitHub: [Security > Report a vulnerability](https://github.com/jewei/tinydash/security/advisories/new).

Include what you did, what happened, and the TinyDash version and OS. A fix is released as a new version, and the advisory is published after it.

## What counts

- A way for a web page, a file, a clipboard entry, or another app to make TinyDash run, open, or send something the user did not choose.
- A way around the checks on update files or release downloads.
- Saved data, such as clipboard history, leaving the computer.

## How releases are protected

- Updates are signed, and TinyDash refuses an update whose signature does not match its own key.
- The macOS app is signed with a Developer ID and notarized by Apple.
- Each release file has a build provenance attestation. Check one with `gh attestation verify FILE --repo jewei/tinydash`.
- CodeQL scans the code on each pull request, and `cargo audit` checks the Rust dependencies every week.
