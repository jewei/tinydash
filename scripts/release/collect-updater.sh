#!/usr/bin/env bash
set -euo pipefail

output=${1:?Usage: collect-updater.sh <artifact-dir>}
release_dir=${2:-src-tauri/target/release}
mkdir -p "$output"

case "${RUNNER_OS:-$(uname -s)}" in
  macOS|Darwin)
    pattern='*.app.tar.gz.sig'
    ;;
  Windows_NT|Windows)
    pattern='*-setup.exe.sig'
    ;;
  Linux)
    echo 'Linux uses manual .deb updates; no Tauri updater artifact is collected.'
    exit 0
    ;;
  *)
    echo "Unsupported runner OS: ${RUNNER_OS:-unknown}" >&2
    exit 1
    ;;
esac

signatures=$(find "$release_dir/bundle" -type f -name "$pattern" -print)
signature_count=$(printf '%s\n' "$signatures" | sed '/^$/d' | wc -l | tr -d ' ')
test "$signature_count" -eq 1 || {
  echo "Expected one updater signature matching $pattern, found $signature_count." >&2
  exit 1
}

signature=$signatures
artifact=${signature%.sig}
test -f "$artifact" || {
  echo "Updater artifact for $signature is missing." >&2
  exit 1
}

artifact_name=$(basename "$artifact")
signature_name=$(basename "$signature")
if [[ "$artifact" != "$output/$artifact_name" ]]; then
  cp "$artifact" "$output/$artifact_name"
fi
if [[ "$signature" != "$output/$signature_name" ]]; then
  cp "$signature" "$output/$signature_name"
fi
echo "Collected $artifact_name and $signature_name"
