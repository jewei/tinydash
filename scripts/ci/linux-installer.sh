#!/usr/bin/env bash
set -euo pipefail
if [ "${RUNNER_ENVIRONMENT:-}" != github-hosted ]; then
  echo 'Installer checks require a disposable GitHub-hosted runner.' >&2
  exit 1
fi

settings="$HOME/.config/dev.tinydash.launcher/settings.json"
marker="$HOME/.local/share/dev.tinydash.launcher/installer-check.txt"
desktop=/usr/share/applications/TinyDash.desktop
settings_text='{"currencyRatesEnabled":false,"clearQueryOnOpen":true}'
check_data() {
  test "$(cat "$settings")" = "$settings_text"
  test "$(cat "$marker")" = 'preserve user data'
}

case "${1:-}" in
  install)
    test ! -e /usr/bin/tinydash
    test ! -e "$settings"
    packages=(native-build/*.deb)
    test "${#packages[@]}" -eq 1
    package="$(realpath "${packages[0]}")"
    test "$(dpkg-deb -f "$package" Package)" = tiny-dash
    test "$(dpkg-deb -f "$package" Architecture)" = "$(dpkg --print-architecture)"
    dpkg-deb -I "$package"
    sudo apt-get install -y "$package"
    test -x /usr/bin/tinydash
    # Tauri patches its bundle-type marker when making the package. Compare
    # with the packaged executable, not the standalone build's different marker.
    expected="$RUNNER_TEMP/tinydash-package"
    dpkg-deb --extract "$package" "$expected"
    cmp "$expected/usr/bin/tinydash" /usr/bin/tinydash
    desktop-file-validate "$desktop"
    grep -Fx 'Exec=tinydash' "$desktop"
    grep -Fx 'Icon=tinydash' "$desktop"
    test -f /usr/share/icons/hicolor/128x128/apps/tinydash.png
    ldd /usr/bin/tinydash > test-results/native/installed-libraries.txt
    if grep -q 'not found' test-results/native/installed-libraries.txt; then
      cat test-results/native/installed-libraries.txt >&2
      exit 1
    fi
    mkdir -p "$(dirname "$settings")" "$(dirname "$marker")"
    printf '%s' "$settings_text" > "$settings"
    printf '%s' 'preserve user data' > "$marker"
    sudo apt-get install --reinstall -y "$package"
    check_data
    echo 'TINYDASH_NATIVE_BINARY=/usr/bin/tinydash' >> "$GITHUB_ENV"
    echo 'PASS: Debian installation, executable, desktop entry, libraries, and reinstall'
    ;;
  remove)
    sudo apt-get remove -y tiny-dash
    test ! -e /usr/bin/tinydash
    test ! -e "$desktop"
    check_data
    echo 'PASS: removal deleted the app and desktop entry and preserved user data'
    ;;
  *)
    echo 'Usage: bash scripts/ci/linux-installer.sh install|remove' >&2
    exit 1
    ;;
esac
