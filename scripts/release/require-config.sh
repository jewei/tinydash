#!/usr/bin/env bash
set -euo pipefail

if [[ "${RELEASE_MODE:-false}" != "true" ]]; then
  echo 'Release configuration checks are disabled for a development build.'
  exit 0
fi

required=()
case "${RUNNER_OS:-}" in
  macOS)
    required=(APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY
      APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID)
    ;;
  Windows) ;; # The Windows preview has no Authenticode certificate.
  Linux)
    echo 'Linux uses SHA-256 checksums and manual .deb updates. No signing secrets are required.'
    exit 0
    ;;
  *) echo 'Release mode requires RUNNER_OS=macOS, Windows, or Linux.' >&2; exit 1 ;;
esac
required+=(TAURI_SIGNING_PRIVATE_KEY TAURI_UPDATER_PUBLIC_KEY TINYDASH_UPDATE_ENDPOINT)
missing=()
for name in "${required[@]}"; do
  value=${!name:-}
  if [[ -z "${value//[[:space:]]/}" ]]; then missing+=("$name"); fi
done
if [[ "$RUNNER_OS" == "macOS" && "${APPLE_SIGNING_IDENTITY:-}" != 'Developer ID Application: '* ]]; then
  missing+=("APPLE_SIGNING_IDENTITY (Developer ID Application identity required)")
fi
if [[ -n "${TINYDASH_UPDATE_ENDPOINT:-}" && "${TINYDASH_UPDATE_ENDPOINT}" != https://* ]]; then
  missing+=("TINYDASH_UPDATE_ENDPOINT (HTTPS URL required)")
fi
if ((${#missing[@]})); then
  printf 'Release mode needs valid configuration for %s. Missing or invalid:\n' "$RUNNER_OS" >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 1
fi
echo "Release configuration is present for $RUNNER_OS."
