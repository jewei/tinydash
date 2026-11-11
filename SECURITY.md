# Security

## Supported versions

Only the latest release gets security fixes. The Go app has no release yet; until it does, the latest release is the Rust app 0.9.0, which updates itself on macOS and Windows.

## Report a vulnerability

Do not open a public issue. Report it privately on GitHub: [Security > Report a vulnerability](https://github.com/jewei/tinydash/security/advisories/new).

Include what you did, what happened, and the TinyDash version and OS. A fix is released as a new version, and the advisory is published after it.

## What counts

- A way for a web page, a file, a clipboard entry, or another app to make TinyDash run, open, or send something the user did not choose.
- A way around the checks on release downloads.
- Saved data, such as clipboard history, leaving the computer.

## How the code is checked

- CodeQL scans the Go and workflow code on each pull request, on `main`, and on the first day of the month.
- `govulncheck` checks the Go dependencies on the first day of the month.
- The release process for the Go app is not set up yet. The earlier Rust releases are signed and notarized on macOS, and each file has a build provenance attestation: check one with `gh attestation verify FILE --repo jewei/tinydash`.
