#!/usr/bin/env bash
set -euo pipefail

if [[ "${RELEASE_MODE:-false}" != "true" ]]; then
  echo 'Release signing checks are disabled for an unsigned build.'
  exit 0
fi

required=(
  APPLE_CERTIFICATE
  APPLE_CERTIFICATE_PASSWORD
  APPLE_SIGNING_IDENTITY
  APPLE_ID
  APPLE_PASSWORD
  APPLE_TEAM_ID
  WINDOWS_CERTIFICATE
  WINDOWS_CERTIFICATE_PASSWORD
  TAURI_SIGNING_PRIVATE_KEY
  TAURI_UPDATER_PUBLIC_KEY
  TINYDASH_UPDATE_ENDPOINT
)
missing=()
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then missing+=("$name"); fi
done
if [[ "${APPLE_SIGNING_IDENTITY:-}" == "-" ]]; then
  missing+=("APPLE_SIGNING_IDENTITY (Developer ID identity required; '-' is ad-hoc)")
fi
if [[ -n "${TINYDASH_UPDATE_ENDPOINT:-}" && "${TINYDASH_UPDATE_ENDPOINT}" != https://* ]]; then
  missing+=("TINYDASH_UPDATE_ENDPOINT (HTTPS URL required)")
fi
if ((${#missing[@]})); then
  printf 'Release mode needs signing and updater configuration. Missing:\n' >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 1
fi
echo 'Release signing and updater configuration is present.'
