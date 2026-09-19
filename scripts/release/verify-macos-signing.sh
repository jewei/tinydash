#!/usr/bin/env bash
set -euo pipefail

app=${1:-src-tauri/target/release/bundle/macos/TinyDash.app}
test -d "$app" || { echo "Missing macOS app: $app" >&2; exit 1; }
test -n "${APPLE_SIGNING_IDENTITY:-}" || { echo 'APPLE_SIGNING_IDENTITY is required.' >&2; exit 1; }
codesign --verify --deep --strict --verbose=2 "$app"
authority=$(codesign --display --verbose=4 "$app" 2>&1 | sed -n 's/^Authority=//p' | head -n 1)
test "$authority" = "$APPLE_SIGNING_IDENTITY" || {
  echo "Unexpected macOS signing identity: ${authority:-missing}" >&2
  exit 1
}
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"
dmg_files=$(find src-tauri/target/release/bundle/dmg -maxdepth 1 -type f -name '*.dmg' -print)
dmg_count=$(printf '%s\n' "$dmg_files" | sed '/^$/d' | wc -l | tr -d ' ')
test "$dmg_count" -eq 1 || {
  echo "Expected one macOS DMG, found $dmg_count." >&2
  exit 1
}
codesign --verify --verbose=2 "$dmg_files"
dmg_authority=$(codesign --display --verbose=4 "$dmg_files" 2>&1 | sed -n 's/^Authority=//p' | head -n 1)
test "$dmg_authority" = "$APPLE_SIGNING_IDENTITY" || {
  echo "Unexpected macOS DMG signing identity: ${dmg_authority:-missing}" >&2
  exit 1
}
xcrun stapler validate "$dmg_files"
signatures=$(find src-tauri/target/release/bundle/macos -maxdepth 1 -type f -name '*.app.tar.gz.sig' -print)
signature_count=$(printf '%s\n' "$signatures" | sed '/^$/d' | wc -l | tr -d ' ')
test "$signature_count" -eq 1 || {
  echo "Expected one macOS updater signature, found $signature_count." >&2
  exit 1
}
archive=${signatures%.sig}
test -f "$archive" || { echo "Missing updater archive for $signatures" >&2; exit 1; }
echo 'PASS: macOS app and DMG publisher signature, notarization ticket, Gatekeeper assessment, and updater pair'
