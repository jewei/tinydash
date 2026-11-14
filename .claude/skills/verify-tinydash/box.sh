#!/bin/sh
# Runs inside the verification container; verify.sh calls it.
#   box.sh start    seed the home and data folders, start X and TinyDash, wait until ready
#   box.sh doctor   read-only checks of the app this box started
set -eu

data=/scratch/data
socket=/run/td/automation.sock
log=/scratch/app.log
pidfile=/scratch/app.pid

seed() {
  mkdir -p "$data" /root/Documents/projects /root/.local/share/applications
  # settings.json: index only the seeded Documents folder; every other key
  # keeps its default, so clipboard history starts off, as for a new user.
  cat >"$data/settings.json" <<'EOF'
{"fileSearchFolders": ["~/Documents"]}
EOF
  echo "Q3 numbers" >"/root/Documents/Quarterly Report.txt"
  echo "milk, eggs" >/root/Documents/grocery-list.md
  echo "plan" >/root/Documents/projects/roadmap-2027.md
  # An app to find and run: running it leaves a marker file.
  cat >/root/.local/share/applications/verify-notes.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Verify Notes
Comment=Seeded by the verify-tinydash skill
Exec=sh -c "echo launched > /scratch/launched-verify-notes"
Terminal=false
EOF
}

# prepare makes the launch options real before the app starts: a stub
# logs each call to /scratch/stub-calls and succeeds, and a hidden
# library moves out of the loader's path.
prepare() {
  mkdir -p /opt/stubs
  for name in ${TD_STUBS:-}; do
    printf '#!/bin/sh\necho "%s $*" >>/scratch/stub-calls\n' "$name" >"/opt/stubs/$name"
    chmod +x "/opt/stubs/$name"
  done
  for lib in ${TD_WITHOUT_LIBS:-}; do
    mkdir -p /root/without-libs
    found=0
    for file in /usr/lib/*/"$lib".so*; do
      [ -e "$file" ] || continue
      mv "$file" /root/without-libs/ && found=1
    done
    [ "$found" = 1 ] || { echo "--without-lib $lib: no /usr/lib/*/$lib.so* in the box" >&2; exit 1; }
  done
  ldconfig
}

start() {
  seed
  prepare
  mkdir -p /run/td && chmod 700 /run/td
  Xvfb :99 -screen 0 1440x900x24 -nolisten tcp >/scratch/xvfb.log 2>&1 &
  i=0
  until xdpyinfo >/dev/null 2>&1; do
    i=$((i + 1)); [ "$i" -lt 100 ] || { echo "Xvfb did not start" >&2; exit 1; }
    sleep 0.1
  done
  openbox >/scratch/wm.log 2>&1 &
  # A session bus for GTK and the tray icon; the app gets its pid file, so
  # cleanup and doctor name this process, never a process name.
  eval "$(dbus-launch --sh-syntax)"
  # shellcheck disable=SC2016 # $$ is the inner shell's, which exec makes the app
  (cd /scratch && exec sh -c 'echo $$ >"$1" && exec env MYGO_AUTOMATION="$2" /opt/td/tinydash --data-dir "$3"' sh "$pidfile" "$socket" "$data") >"$log" 2>&1 &
  ready
}

# ready: the socket lists the launcher and both indexes were built.
ready() {
  i=0
  while :; do
    if [ -S "$socket" ] && grep -q "Indexed applications" "$log" && grep -q "Indexed files" "$log" &&
      /opt/td/mygo-automation open --socket "$socket" 2>/dev/null | grep -q '"TinyDash"'; then
      break
    fi
    i=$((i + 1))
    if [ "$i" -ge 300 ]; then
      echo "TinyDash was not ready after 30 s; log:" >&2
      cat "$log" >&2
      exit 1
    fi
    sleep 0.1
  done
  grep "Indexed" "$log"
  /opt/td/mygo-automation windows
}

doctor() {
  fail=0
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  if [ -n "$pid" ] && [ "$(readlink "/proc/$pid/exe" 2>/dev/null)" = /opt/td/tinydash ]; then
    echo "ok   app pid $pid runs /opt/td/tinydash --data-dir $(tr '\0' ' ' <"/proc/$pid/cmdline" | sed 's/.*--data-dir //')"
    if sha256sum "/proc/$pid/exe" | cut -d' ' -f1 | grep -qx "$(sed -n 's/^sha256 //p' /opt/td/BUILD)"; then
      echo "ok   running binary is the one in BUILD"
    else
      echo "FAIL running binary differs from BUILD: the box runs an older build; cleanup and launch again"; fail=1
    fi
  else
    echo "FAIL app process is gone; tail of the log:"; tail -5 "$log"; fail=1
  fi
  if out="$(/opt/td/mygo-automation windows 2>&1)"; then
    echo "ok   automation socket answers: $out"
  else
    echo "FAIL automation socket: $out"; fail=1
  fi
  for line in "Indexed applications" "Indexed files"; do
    if grep -q "$line" "$log"; then echo "ok   log: $(grep -m1 "$line" "$log" | cut -d' ' -f3-)"; else echo "FAIL log lacks \"$line\""; fail=1; fi
  done
  for name in ${TD_STUBS:-}; do echo "ok   stub $name: $(command -v "$name")"; done
  for lib in ${TD_WITHOUT_LIBS:-}; do
    if ls /usr/lib/*/"$lib".so* >/dev/null 2>&1; then echo "FAIL $lib is loadable"; fail=1; else echo "ok   without $lib"; fi
  done
  if grep -E ' (WARN|ERROR) |panic' "$log" >/dev/null; then
    echo "WARN app logged:"; grep -E ' (WARN|ERROR) |panic' "$log"
  fi
  exit "$fail"
}

case "${1:-}" in
start) start ;;
doctor) doctor ;;
*) echo "usage: box.sh start|doctor" >&2; exit 2 ;;
esac
