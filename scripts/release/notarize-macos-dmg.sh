#!/usr/bin/env bash
set -euo pipefail

test -n "${APPLE_ID:-}" || { echo 'APPLE_ID is required.' >&2; exit 1; }
test -n "${APPLE_PASSWORD:-}" || { echo 'APPLE_PASSWORD is required.' >&2; exit 1; }
test -n "${APPLE_TEAM_ID:-}" || { echo 'APPLE_TEAM_ID is required.' >&2; exit 1; }
dmg_files=$(find src-tauri/target/release/bundle/dmg -maxdepth 1 -type f -name '*.dmg' -print)
dmg_count=$(printf '%s\n' "$dmg_files" | sed '/^$/d' | wc -l | tr -d ' ')
test "$dmg_count" -eq 1 || {
  echo "Expected one macOS DMG, found $dmg_count." >&2
  exit 1
}
xcrun notarytool submit "$dmg_files" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait
xcrun stapler staple "$dmg_files"
echo "Notarized and stapled $dmg_files"
