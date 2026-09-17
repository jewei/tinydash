#!/usr/bin/env bash
set -euo pipefail

# Mount read-only and copy to a temporary directory, never to /Applications.
version=$(bun -e 'import config from "./src-tauri/tauri.conf.json"; console.log(config.version)')
images=(src-tauri/target/release/bundle/dmg/*_"$version"_*.dmg)
test "${#images[@]}" -eq 1
source_app=src-tauri/target/release/bundle/macos/TinyDash.app
check_dir=$(mktemp -d "${TMPDIR:-/tmp}/tinydash-dmg.XXXXXX")
mounted=false
cleanup() {
  if "$mounted"; then hdiutil detach "$check_dir/mount"; fi
  rm -rf "$check_dir"
}
trap cleanup EXIT
mkdir "$check_dir/mount" "$check_dir/installed"
hdiutil verify "${images[0]}"
hdiutil attach -readonly -nobrowse -mountpoint "$check_dir/mount" "${images[0]}"
mounted=true
test "$(readlink "$check_dir/mount/Applications")" = /Applications
ditto "$check_dir/mount/TinyDash.app" "$check_dir/installed/TinyDash.app"
installed="$check_dir/installed/TinyDash.app/Contents"
test -x "$installed/MacOS/tinydash"
test "$(plutil -extract CFBundleIdentifier raw "$installed/Info.plist")" = dev.tinydash.launcher
cmp "$source_app/Contents/Info.plist" "$installed/Info.plist"
cmp "$source_app/Contents/MacOS/tinydash" "$installed/MacOS/tinydash"
codesign --verify --deep --strict "$check_dir/installed/TinyDash.app"
file "$installed/MacOS/tinydash"
echo 'PASS: disk image, Applications link, copied app, metadata, and code integrity'
