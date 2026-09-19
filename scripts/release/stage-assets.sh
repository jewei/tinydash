#!/usr/bin/env bash
set -euo pipefail

output=${1:?Usage: stage-assets.sh <output> <artifact-dir>...}
shift
test "$#" -eq 3
rm -rf "$output"
mkdir -p "$output"
declare -A seen
for directory in "$@"; do
  test -d "$directory"
  label=$(basename "$directory")
  case "$label" in
    macos)
      platform=darwin
      architecture=ARM64
      updater_artifacts=true
      ;;
    windows)
      platform=win32
      architecture=X64
      updater_artifacts=true
      ;;
    linux)
      platform=linux
      architecture=X64
      updater_artifacts=false
      ;;
    *)
      echo "Unexpected release artifact directory: $directory" >&2
      exit 1
      ;;
  esac
  if [[ "${seen[$label]:-0}" -eq 1 ]]; then
    echo "Duplicate release artifact directory: $directory" >&2
    exit 1
  fi
  seen[$label]=1
  bun scripts/ci/artifacts.ts verify "$directory"
  RELEASE_MODE=true UPDATER_ARTIFACTS="$updater_artifacts" \
    bun scripts/release/verify-artifacts.ts "$directory" "$platform" "$architecture"
  if grep -Fxq 'Distribution: unsigned test build' "$directory/build.txt"; then
    echo "Refusing development build in a staged release: $directory" >&2
    exit 1
  fi
  while IFS= read -r -d '' file; do
    name=$(basename "$file")
    case "$name" in
      build.txt)
        cp "$file" "$output/build-$label.txt"
        continue
        ;;
      install.md|desktop-checks.md)
        if [[ ! -e "$output/$name" ]]; then
          cp "$file" "$output/$name"
        fi
        continue
        ;;
      SHA256SUMS) continue ;;
    esac
    test ! -e "$output/$name" || { echo "Duplicate release asset: $name" >&2; exit 1; }
    cp "$file" "$output/$name"
  done < <(find "$directory" -maxdepth 1 -type f -print0)
done
for label in macos windows linux; do
  test "${seen[$label]:-0}" -eq 1 || {
    echo "Missing $label release artifact directory" >&2
    exit 1
  }
done
test -f "$output/install.md" || { echo "Missing install.md in staged release" >&2; exit 1; }
test -f "$output/desktop-checks.md" || { echo "Missing desktop-checks.md in staged release" >&2; exit 1; }
{
  echo 'TinyDash release package set'
  for file in "$output"/build-*.txt; do
    echo
    echo "[$(basename "$file")]"
    cat "$file"
  done
} > "$output/build.txt"
package_count=$(find "$output" -maxdepth 1 -type f \( \
  -name '*.dmg' \
  -o -name '*-setup.exe' \
  -o -name '*.deb' \
\) | wc -l | tr -d ' ')
test "$package_count" -eq 3 || { echo "Expected three direct-download package files, found $package_count" >&2; exit 1; }
bun scripts/ci/artifacts.ts checksums "$output"
bun scripts/ci/artifacts.ts verify "$output"
echo "Staged $package_count direct-download packages and updater assets in $output"
